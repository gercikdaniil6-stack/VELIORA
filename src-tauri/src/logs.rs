//! Event journal: an in-memory ring (what the Logs page shows), a live event
//! stream to the UI and a daily file under the app data folder.
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};

const CAPACITY: usize = 3000;
const KEEP_DAYS: u64 = 7;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: u64,
    pub time: u64,
    /// app | tgws | zapret | proxy | test
    pub source: &'static str,
    /// info | warn | error
    pub level: &'static str,
    pub text: String,
}

struct Logger {
    ring: Mutex<VecDeque<LogEntry>>,
    next: AtomicU64,
    app: OnceLock<AppHandle>,
    dir: OnceLock<PathBuf>,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

fn logger() -> &'static Logger {
    LOGGER.get_or_init(|| Logger { ring: Mutex::new(VecDeque::with_capacity(CAPACITY)), next: AtomicU64::new(1), app: OnceLock::new(), dir: OnceLock::new() })
}

pub fn init(app: &AppHandle, dir: PathBuf) {
    let l = logger();
    let _ = std::fs::create_dir_all(&dir);
    prune(&dir);
    let _ = l.dir.set(dir);
    let _ = l.app.set(app.clone());
}

fn prune(dir: &PathBuf) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let limit = std::time::Duration::from_secs(KEEP_DAYS * 86_400);
    for e in rd.flatten() {
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > limit);
        if old && e.path().extension().is_some_and(|x| x == "log") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

pub fn log(source: &'static str, level: &'static str, text: impl Into<String>) {
    let text: String = text.into();
    let text = text.trim_end().to_string();
    if text.is_empty() {
        return;
    }
    let l = logger();
    let entry = LogEntry { id: l.next.fetch_add(1, Ordering::Relaxed), time: crate::win::now_ms(), source, level, text };
    {
        let mut ring = l.ring.lock();
        if ring.len() >= CAPACITY {
            ring.pop_front();
        }
        ring.push_back(entry.clone());
    }
    if let Some(dir) = l.dir.get() {
        let (y, m, d) = crate::win::local_date();
        let path = dir.join(format!("{y:04}-{m:02}-{d:02}.log"));
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{} [{}] {}: {}", entry.time, source, level, entry.text);
        }
    }
    if let Some(app) = l.app.get() {
        let _ = app.emit("log://entry", &entry);
    }
}

pub fn info(source: &'static str, text: impl Into<String>) {
    log(source, "info", text)
}
pub fn warn(source: &'static str, text: impl Into<String>) {
    log(source, "warn", text)
}
pub fn error(source: &'static str, text: impl Into<String>) {
    log(source, "error", text)
}

pub fn history() -> Vec<LogEntry> {
    logger().ring.lock().iter().cloned().collect()
}

pub fn clear() {
    logger().ring.lock().clear();
}

pub fn export(path: &str) -> Result<usize, String> {
    let ring = logger().ring.lock().clone();
    let mut out = String::new();
    for e in &ring {
        out.push_str(&format!("{}\t{}\t{}\t{}\r\n", e.time, e.source, e.level, e.text));
    }
    std::fs::write(path, out).map_err(|e| format!("Не удалось сохранить файл: {e}"))?;
    Ok(ring.len())
}
