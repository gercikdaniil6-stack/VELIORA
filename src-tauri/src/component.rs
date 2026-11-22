//! A supervised background program (TgWsProxy, winws): spawned hidden inside a
//! job object, its output streamed into the journal, its exit observed and
//! reported as a status the UI and tray can show.
use crate::logs;
use crate::win::Job;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CoreStatus {
    /// stopped | starting | running | stopping | error
    pub state: &'static str,
    pub pid: Option<u32>,
    pub started_at: Option<u64>,
    pub last_error: Option<String>,
    /// What is running (strategy name for Zapret, listen address for Telegram)
    pub detail: Option<String>,
}

impl Default for CoreStatus {
    fn default() -> Self {
        Self { state: "stopped", pid: None, started_at: None, last_error: None, detail: None }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Health {
    Unknown,
    Ready,
    Failed,
}

/// Classifies one output line: (journal level, health signal).
pub type LineHook = fn(&str) -> (&'static str, Option<Health>);

struct Inner {
    status: CoreStatus,
    job: Option<Arc<Job>>,
    generation: u64,
    stopping: bool,
    alive: bool,
    health: Health,
    tail: VecDeque<String>,
}

pub struct Component {
    pub source: &'static str,
    inner: Mutex<Inner>,
    /// Serialises start/stop/restart so concurrent requests can't race.
    pub op: Mutex<()>,
    app: OnceLock<AppHandle>,
}

impl Component {
    pub fn new(source: &'static str) -> Self {
        Self {
            source,
            inner: Mutex::new(Inner { status: CoreStatus::default(), job: None, generation: 0, stopping: false, alive: false, health: Health::Unknown, tail: VecDeque::new() }),
            op: Mutex::new(()),
            app: OnceLock::new(),
        }
    }

    pub fn attach(&self, app: &AppHandle) {
        let _ = self.app.set(app.clone());
    }

    pub fn status(&self) -> CoreStatus {
        self.inner.lock().status.clone()
    }

    pub fn set_status(&self, f: impl FnOnce(&mut CoreStatus)) {
        let snapshot = {
            let mut s = self.inner.lock();
            f(&mut s.status);
            s.status.clone()
        };
        self.emit(&snapshot);
    }

    fn emit(&self, st: &CoreStatus) {
        if let Some(app) = self.app.get() {
            let _ = app.emit("core://status", serde_json::json!({ "component": self.source, "status": st }));
            crate::refresh_tray(app);
        }
    }

    pub fn fail(&self, msg: impl Into<String>) -> String {
        let msg = msg.into();
        logs::error(self.source, &msg);
        let m = msg.clone();
        self.set_status(|s| {
            s.state = "error";
            s.pid = None;
            s.last_error = Some(m);
        });
        msg
    }

    pub fn mark_capture_ready(&self, pid: u32) -> bool {
        let status = {
            let mut state = self.inner.lock();
            if !state.alive || state.stopping || state.health != Health::Ready { return false; }
            state.status.state = "running";
            state.status.pid = Some(pid);
            state.status.clone()
        };
        self.emit(&status);
        true
    }

    pub fn current_crash(&self, generation: u64) -> bool {
        let state = self.inner.lock();
        state.generation == generation && !state.alive && !state.stopping && state.status.state == "error"
    }

    /// Last lines the program printed (for error messages).
    pub fn tail(&self) -> String {
        self.inner.lock().tail.iter().cloned().collect::<Vec<_>>().join("\n")
    }

    /// Spawns `cmd` (already configured) and starts supervising it.
    pub fn spawn(self: &Arc<Self>, mut cmd: Command, hook: LineHook) -> Result<u32, String> {
        if self.alive() { return Err("Предыдущий процесс ещё работает.".into()); }
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let job = Arc::new(Job::new().ok_or("Windows не создала группу процессов для безопасной остановки.")?);
        let mut child = cmd.spawn().map_err(|e| match e.raw_os_error() {
            Some(2) | Some(3) => "Файл программы не найден — возможно, его удалил антивирус.".to_string(),
            Some(5) => "Windows или антивирус запретил запуск программы.".to_string(),
            Some(740) => "Программе нужны права администратора — перезапустите VELIORA App с правами.".to_string(),
            Some(1260) => "Запуск программы заблокирован политикой Windows (Smart App Control или AppLocker).".to_string(),
            _ => format!("Не удалось запустить программу: {e}"),
        })?;
        if !job.assign(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Windows не позволила отслеживать дочерний процесс. Запуск отменён.".into());
        }
        let pid = child.id();
        let generation = {
            let mut s = self.inner.lock();
            s.generation += 1;
            s.job = Some(job);
            s.stopping = false;
            s.alive = true;
            s.health = Health::Unknown;
            s.tail.clear();
            s.generation
        };

        for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>)]
            .into_iter()
            .flatten()
        {
            let me = Arc::clone(self);
            std::thread::spawn(move || {
                let mut r = BufReader::new(stream);
                let mut buf = Vec::new();
                let mut discarding = false;
                loop {
                    buf.clear();
                    match r.by_ref().take(8192).read_until(b'\n', &mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            let chunked = buf.len() == 8192 && buf.last() != Some(&b'\n');
                            if discarding || chunked {
                                discarding = chunked;
                                continue;
                            }
                            let line = logs::redact(String::from_utf8_lossy(&buf).trim_end());
                            if line.is_empty() {
                                continue;
                            }
                            let (level, health) = hook(&line);
                            logs::log(me.source, level, &line);
                            let mut s = me.inner.lock();
                            if s.generation != generation {
                                break;
                            }
                            if s.tail.len() >= 12 {
                                s.tail.pop_front();
                            }
                            s.tail.push_back(line);
                            if let Some(h) = health {
                                if s.health != Health::Failed {
                                    s.health = h;
                                }
                            }
                        }
                    }
                }
            });
        }

        let me = Arc::clone(self);
        std::thread::spawn(move || {
            let code = child.wait().ok().and_then(|s| s.code());
            let (snapshot, message, recover) = {
                let mut s = me.inner.lock();
                if s.generation != generation {
                    return;
                }
                let recover = !s.stopping && s.status.state == "running";
                s.alive = false;
                s.job = None;
                s.status.pid = None;
                let message = if s.stopping {
                    s.status.state = "stopped";
                    s.status.last_error = None;
                    s.status.started_at = None;
                    None
                } else {
                    let mut message = match code {
                        Some(c) => format!("Процесс неожиданно завершился (код {c})."),
                        None => "Процесс неожиданно завершился.".into(),
                    };
                    if let Some(line) = s.tail.back() {
                        message.push_str(&format!(" Последнее сообщение: {line}"));
                    }
                    s.status.state = "error";
                    s.status.last_error = Some(message.clone());
                    Some(message)
                };
                (s.status.clone(), message, recover)
            };
            match message {
                Some(message) => logs::error(me.source, message),
                None => logs::info(me.source, "процесс остановлен"),
            }
            me.emit(&snapshot);
            if recover && me.source == "zapret" {
                if let Some(app) = me.app.get() { crate::recover_zapret(app, generation); }
            }
        });
        Ok(pid)
    }

    pub fn health(&self) -> Health {
        self.inner.lock().health
    }

    pub fn alive(&self) -> bool {
        self.inner.lock().alive
    }

    /// Terminates the program (and everything it started) and waits for the exit.
    pub fn kill(&self) {
        let job = {
            let mut s = self.inner.lock();
            s.stopping = true;
            s.job.clone()
        };
        let Some(job) = job else { return; };
        job.terminate();
        let deadline = Instant::now() + Duration::from_millis(2500);
        while self.alive() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    /// Marks stopped once kill() has finished (covers a swallowed exit event).
    pub fn mark_stopped(&self) {
        {
            let mut s = self.inner.lock();
            if s.alive { return; }
            s.job = None;
        }
        self.set_status(|st| {
            st.state = "stopped";
            st.pid = None;
            st.last_error = None;
            st.started_at = None;
        });
    }
}
