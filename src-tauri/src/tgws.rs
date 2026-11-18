//! Telegram WS proxy: a local MTProto proxy (Flowseal/tg-ws-proxy, rebuilt as a
//! headless CLI) that carries Telegram traffic over WebSocket to Telegram's own
//! web endpoints, with a Cloudflare fallback.
use crate::component::{Component, Health};
use crate::config::TgwsConfig;
use crate::logs;
use crate::win;
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const EXE: &str = "TgWsProxy.exe";

pub fn binary(resources: &std::path::Path) -> PathBuf {
    resources.join("components").join("tgws").join(EXE)
}

fn classify(line: &str) -> (&'static str, Option<Health>) {
    let level = if line.contains("ERROR") || line.contains("Traceback") {
        "error"
    } else if line.contains("WARNING") {
        "warn"
    } else {
        "info"
    };
    let health = if line.contains("tg://proxy") || line.contains("Listening") {
        Some(Health::Ready)
    } else if line.contains("Address already in use") || line.contains("error while attempting to bind") || line.contains("Errno 10048") {
        Some(Health::Failed)
    } else {
        None
    };
    (level, health)
}

pub fn link(cfg: &TgwsConfig) -> String {
    let host = if cfg.host == "0.0.0.0" || cfg.host == "::" { "127.0.0.1" } else { cfg.host.as_str() };
    format!("tg://proxy?server={host}&port={}&secret=dd{}", cfg.port, cfg.secret)
}

fn listen_addr(cfg: &TgwsConfig) -> Option<SocketAddr> {
    (cfg.host.as_str(), cfg.port).to_socket_addrs().ok()?.next()
}

fn port_free(addr: SocketAddr) -> bool {
    TcpListener::bind(addr).is_ok()
}

/// Kills leftovers of our own proxy (never other programs' processes).
fn kill_stale(bin: &std::path::Path) -> usize {
    let mut n = 0;
    for p in win::find_processes(EXE) {
        if win::process_path(p.pid).is_some_and(|path| path.eq(bin)) && win::kill_pid(p.pid) {
            n += 1;
        }
    }
    n
}

pub fn start(c: &Arc<Component>, cfg: &TgwsConfig, bin: &std::path::Path) -> Result<(), String> {
    let _guard = c.op.lock();
    if c.alive() {
        return Ok(());
    }
    if !bin.exists() {
        return Err(c.fail(format!("Не найден файл Telegram-прокси ({EXE}). Переустановите VELIORA App или добавьте папку в исключения антивируса.")));
    }
    let Some(addr) = listen_addr(cfg) else {
        return Err(c.fail(format!("Не удалось разобрать адрес {}:{}", cfg.host, cfg.port)));
    };
    c.set_status(|s| {
        s.state = "starting";
        s.started_at = Some(win::now_ms());
        s.last_error = None;
        s.detail = Some(format!("{}:{}", cfg.host, cfg.port));
    });
    logs::info("tgws", format!("запуск на {}:{}", cfg.host, cfg.port));

    if !port_free(addr) {
        let killed = kill_stale(bin);
        if killed > 0 {
            logs::warn("tgws", format!("остановлен зависший экземпляр прокси ({killed})"));
            std::thread::sleep(Duration::from_millis(500));
        }
        if !port_free(addr) {
            return Err(c.fail(format!(
                "Порт {} занят другой программой. Укажите другой порт в разделе Telegram или закройте программу, которая его использует.",
                cfg.port
            )));
        }
    }

    let mut cmd = win::hidden(bin);
    cmd.args(["--host", cfg.host.trim(), "--port", &cfg.port.to_string(), "--secret", &cfg.secret]);
    if !cfg.worker_domain.trim().is_empty() {
        cmd.args(["--cfproxy-worker-domain", cfg.worker_domain.trim()]);
    }
    if cfg.verbose {
        cmd.arg("-v");
    }
    cmd.env("PYTHONUNBUFFERED", "1").env("PYTHONIOENCODING", "utf-8");
    if let Some(dir) = bin.parent() {
        cmd.current_dir(dir);
    }
    let pid = c.spawn(cmd, classify).map_err(|e| c.fail(e))?;

    // Healthy once it accepts connections (onefile builds unpack first: ~1–3 s).
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if !c.alive() {
            let tail = c.tail();
            return Err(c.fail(if tail.is_empty() { "Прокси завершился сразу после запуска.".to_string() } else { format!("Прокси завершился сразу после запуска: {tail}") }));
        }
        if c.health() == Health::Failed {
            c.kill();
            return Err(c.fail(format!("Прокси не смог занять порт {}.", cfg.port)));
        }
        let probe = if addr.ip().is_unspecified() { SocketAddr::from(([127, 0, 0, 1], addr.port())) } else { addr };
        if TcpStream::connect_timeout(&probe, Duration::from_millis(250)).is_ok() {
            break;
        }
        if Instant::now() > deadline {
            c.kill();
            return Err(c.fail("Прокси не начал принимать подключения за 12 секунд."));
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    c.set_status(|s| {
        s.state = "running";
        s.pid = Some(pid);
    });
    logs::info("tgws", format!("работает (pid {pid}), ссылка подключения не меняется между перезапусками"));
    Ok(())
}

pub fn stop(c: &Arc<Component>) {
    let _guard = c.op.lock();
    if !c.alive() {
        c.mark_stopped();
        return;
    }
    c.set_status(|s| s.state = "stopping");
    c.kill();
    c.mark_stopped();
}
