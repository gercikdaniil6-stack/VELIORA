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
const DAILY_LIMIT: u64 = 2 * 1024 * 1024;

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
    disk: Mutex<()>,
    app: OnceLock<AppHandle>,
    dir: OnceLock<PathBuf>,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

fn logger() -> &'static Logger {
    LOGGER.get_or_init(|| Logger { ring: Mutex::new(VecDeque::with_capacity(CAPACITY)), next: AtomicU64::new(1), disk: Mutex::new(()), app: OnceLock::new(), dir: OnceLock::new() })
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

/// Sanitize before a line reaches the journal, disk, exports, or error tails.
pub(crate) fn redact(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut redacted = String::with_capacity(text.len().min(4096));
    let mut from = 0;
    while let Some((start, prefix)) = ["tg://proxy?", "https://t.me/proxy?", "secret=", "--secret ", "authorization:"]
        .iter().filter_map(|prefix| lower[from..].find(prefix).map(|at| (from + at, *prefix))).min_by_key(|(at, _)| *at)
    {
        redacted.push_str(&text[from..start]);
        let value = start + prefix.len();
        if prefix == "authorization:" || prefix == "--secret " {
            redacted.push_str(prefix);
            redacted.push_str("[скрыто]");
            // Quoted values or authorization schemes can contain spaces.
            return redacted.chars().take(4096).collect();
        }
        if prefix == "secret=" { redacted.push_str(prefix); }
        redacted.push_str("[скрыто]");
        from = text[value..].find(|c:char|c.is_whitespace()).map_or(text.len(),|i|value+i);
    }
    redacted.push_str(&text[from..]);
    redacted.chars().take(4096).collect()
}

pub fn log(source: &'static str, level: &'static str, text: impl Into<String>) {
    let text: String = text.into();
    let text = redact(text.trim_end());
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
        let _disk = l.disk.lock();
        let (y, m, d) = crate::win::local_date();
        let path = dir.join(format!("{y:04}-{m:02}-{d:02}.log"));
        let line = format!("{} [{}] {}: {}\n", entry.time, source, level, entry.text);
        if path.metadata().is_ok_and(|meta| meta.len().saturating_add(line.len() as u64) > DAILY_LIMIT) {
            let _ = std::fs::rename(&path, dir.join(format!("{y:04}-{m:02}-{d:02}.previous.log")));
        }
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            if file.metadata().is_ok_and(|meta| meta.len().saturating_add(line.len() as u64) <= DAILY_LIMIT) {
                let _ = file.write_all(line.as_bytes());
            }
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

// Only fixed categories are shared. Native errors can contain arbitrary
// paths, endpoints, credentials and fragments of configuration.
fn export_reason(text: &str) -> &'static str {
    let text = text.to_ascii_lowercase();
    let reasons = [
        ("0x8032000a", "WinDivert: фильтр используется (0x8032000A)"),
        ("object is referenced by other objects", "WinDivert: фильтр используется"),
        ("требует прав администратора", "Нужны права администратора"),
        ("нужны права администратора", "Нужны права администратора"),
        ("файл программы не найден", "Исполняемый файл отсутствует"),
        ("timeout", "Истекло время ожидания"),
        ("истекло время ожидания", "Истекло время ожидания"),
        ("не удалось запустить стратегию", "Ошибка запуска стратегии"),
        ("winws.exe завершился", "winws.exe завершился"),
        ("windivert", "Событие WinDivert"),
    ];
    reasons.iter().find_map(|(marker, label)| text.contains(marker).then_some(*label))
        .unwrap_or("Подробности доступны в локальном журнале")
}

pub fn export(path: &str) -> Result<usize, String> {
    let ring = logger().ring.lock().clone();
    let mut out = format!("VELIORA {} · диагностический отчёт\r\n", env!("CARGO_PKG_VERSION"));
    for e in &ring {
        out.push_str(&format!("{}\t{}\t{}\t{}\r\n", e.time, e.source, e.level, export_reason(&e.text)));
    }
    std::fs::write(path, out).map_err(|e| format!("Не удалось сохранить файл: {e}"))?;
    Ok(ring.len())
}


#[cfg(test)]
mod tests {
    #[test]
    fn support_export_omits_arbitrary_process_output() {
        let private = r"WinDivert error 0x8032000a C:\Users\PrivateUser\example secret=private";
        assert_eq!(super::export_reason(private), "WinDivert: фильтр используется (0x8032000A)");
        assert_eq!(super::export_reason("https://user:password@example.org"), "Подробности доступны в локальном журнале");
    }

    #[test]
    fn secrets_are_removed_from_logs_and_tails() {
        for line in ["ready tg://proxy?server=example.org&SECRET=value", "argument --secret \"value\"", "Authorization: Bearer value", "key Secret=value"] {
            assert!(!super::redact(line).contains("value"));
        }
        assert_eq!(super::redact("HTTP 603"), "HTTP 603");
    }
}
