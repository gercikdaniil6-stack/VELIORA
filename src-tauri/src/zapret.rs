//! Zapret DPI bypass: runs winws.exe (bol-van/zapret) with the strategies of the
//! Flowseal/zapret-discord-youtube bundle. Strategies are parsed from the
//! bundle's general*.bat files and launched directly — no cmd.exe, no
//! service.bat — so VELIORA controls the game filter, IPSet mode and fakes.
use crate::component::{Component, Health};
use crate::config::ZapretConfig;
use crate::logs;
use crate::win;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const PLACEHOLDER_DOMAIN: &str = "domain.example.abc";
pub const PLACEHOLDER_IP: &str = "203.0.113.113/32";
const USER_FILES: [&str; 4] = ["list-general-user.txt", "list-exclude-user.txt", "ipset-exclude-user.txt", "ipset-all-user.txt"];

/// Telegram networks, added to the IPSet in "basic" mode.
const TELEGRAM_NETS: [&str; 10] = [
    "91.108.4.0/22",
    "91.108.8.0/22",
    "91.108.12.0/22",
    "91.108.16.0/22",
    "91.108.20.0/22",
    "91.108.56.0/22",
    "91.105.192.0/23",
    "95.161.64.0/20",
    "149.154.160.0/20",
    "185.76.151.0/24",
];

// ── locations ─────────────────────────────────────────────────────────────

pub fn bundled(resources: &Path) -> PathBuf {
    resources.join("components").join("zapret")
}

/// Working copy in %ProgramData%\VELIORA\zapret (admin-only ACL).
pub fn runtime() -> PathBuf {
    win::program_data().join("zapret")
}

/// The bundle that is used: the prepared working copy, else the bundled one (read-only).
pub fn active_dir(resources: &Path) -> PathBuf {
    let rt = runtime();
    if rt.join("bin").join("winws.exe").exists() {
        rt
    } else {
        bundled(resources)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BundleState {
    pub version: String,
    /// bundled | update
    pub source: String,
}

pub fn bundle_state(dir: &Path) -> Option<BundleState> {
    serde_json::from_slice(&std::fs::read(dir.join("veliora.json")).ok()?).ok()
}

pub fn bundled_version(resources: &Path) -> String {
    std::fs::read(resources.join("components").join("components.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v["zapret"]["version"].as_str().map(String::from))
        .unwrap_or_else(|| "?".into())
}

pub fn installed_version(resources: &Path) -> String {
    bundle_state(&runtime()).map(|s| s.version).unwrap_or_else(|| bundled_version(resources))
}

pub(crate) fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let ty = e.file_type()?;
        if ty.is_symlink() {
            continue;
        }
        let to = dst.join(e.file_name());
        if ty.is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            std::fs::copy(e.path(), to)?;
        }
    }
    Ok(())
}

/// Turns an unpacked bundle in `staging` into the working copy: keeps pristine
/// list copies, carries the user's own lists over and swaps folders.
pub fn finalize_install(staging: &Path, version: &str, source: &str) -> Result<(), String> {
    let err = |e: std::io::Error| format!("Не удалось подготовить файлы Zapret: {e}");
    if !staging.join("bin").join("winws.exe").exists() || !staging.join("general.bat").exists() {
        return Err("Сборка неполная: нет bin\\winws.exe или general.bat.".into());
    }
    let originals = staging.join(".originals").join("lists");
    let _ = std::fs::remove_dir_all(&originals);
    copy_dir(&staging.join("lists"), &originals).map_err(err)?;

    let rt = runtime();
    let mut carried = 0;
    for f in USER_FILES {
        let from = rt.join("lists").join(f);
        match std::fs::copy(&from, staging.join("lists").join(f)) {
            Ok(_) => carried += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !from.exists() => {}
            Err(error) => return Err(format!("Не удалось сохранить список {f}: {error}")),
        }
    }
    let state = BundleState { version: version.to_string(), source: source.to_string() };
    std::fs::write(staging.join("veliora.json"), serde_json::to_vec_pretty(&state).unwrap_or_default()).map_err(err)?;
    ensure_user_files(staging);
    win::protected_tree(staging).map_err(err)?;

    let old = win::program_data().join("zapret.old");
    let _ = std::fs::remove_dir_all(&old);
    if rt.exists() {
        std::fs::rename(&rt, &old).map_err(|e| format!("Файлы Zapret заняты другой программой: {e}"))?;
    }
    if let Err(e) = std::fs::rename(staging, &rt) {
        let _ = std::fs::rename(&old, &rt);
        return Err(format!("Не удалось заменить файлы Zapret: {e}"));
    }
    logs::info("zapret", format!("установлена сборка {version} ({source}), перенесено ваших списков: {carried}"));
    Ok(())
}

pub fn rollback_install() -> Result<(), String> {
    let root=win::program_data();
    let old=root.join("zapret.old");
    let failed=root.join("zapret.failed");
    if !old.is_dir() {return Err("Предыдущая сборка недоступна.".into());}
    if failed.exists() {std::fs::remove_dir_all(&failed).map_err(|e| e.to_string())?;}
    let current=runtime();
    std::fs::rename(&current,&failed).map_err(|e|e.to_string())?;
    if let Err(error)=std::fs::rename(&old,&current) {
        let _=std::fs::rename(&failed,&current);
        return Err(error.to_string());
    }
    Ok(())
}

pub fn protect_runtime() -> Result<(), String> {
    if !win::is_admin() || !win::trusted_install() {
        return Err("Установите VELIORA в Program Files, чтобы использовать Zapret с повышенными правами.".into());
    }
    let root = win::program_data();
    win::create_protected_dir(&root).map_err(|e| format!("Недоступна защищённая папка Zapret: {e}"))?;
    win::protected_tree(&root).map_err(|e| format!("Zapret остановлен: {e}"))
}

/// Creates or refreshes the working copy from the bundled files when needed.
pub fn prepare(resources: &Path) -> Result<(), String> {
    if !win::is_admin() {
        return Err("Нужны права администратора".into());
    }
    protect_runtime()?;
    let rt = runtime();
    let bundled_ver = bundled_version(resources);
    let state = bundle_state(&rt);
    let present = rt.join("bin").join("winws.exe").exists();
    let outdated = state.as_ref().is_some_and(|s| s.source == "bundled" && s.version != bundled_ver);
    if present && state.is_some() && !outdated {
        ensure_user_files(&rt);
        return Ok(());
    }
    let staging = win::program_data().join("zapret.new");
    let _ = std::fs::remove_dir_all(&staging);
    copy_dir(&bundled(resources), &staging).map_err(|e| format!("Не удалось скопировать встроенную сборку Zapret: {e}"))?;
    finalize_install(&staging, &bundled_ver, "bundled")
}

pub fn ensure_user_files(dir: &Path) {
    let lists = dir.join("lists");
    let _ = std::fs::create_dir_all(&lists);
    let defaults = [
        ("list-general-user.txt", format!("# Never leave this file empty\r\n{PLACEHOLDER_DOMAIN}\r\n")),
        ("list-exclude-user.txt", format!("{PLACEHOLDER_DOMAIN}\r\n")),
        ("ipset-exclude-user.txt", format!("{PLACEHOLDER_IP}\r\n")),
        ("ipset-all-user.txt", "# VELIORA: your own IPs/CIDRs for the IPSet rules\r\n".to_string()),
    ];
    for (f, content) in defaults {
        let p = lists.join(f);
        if !p.exists() {
            let _ = std::fs::write(p, content);
        }
    }
}

// ── strategies ────────────────────────────────────────────────────────────

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Strategy {
    pub file: String,
    pub name: String,
    pub note: Option<String>,
    pub game_filter: bool,
    pub fake_discord: bool,
    pub fake_game: bool,
}

fn strategy_files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| {
                    let l = n.to_ascii_lowercase();
                    l.starts_with("general") && l.ends_with(".bat")
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| natural_key(a).cmp(&natural_key(b)));
    v
}

/// "general (ALT10).bat" sorts after "general (ALT9).bat".
fn natural_key(s: &str) -> (String, u32) {
    let base = s.trim_end_matches(".bat");
    let end = base.trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
    let split = end.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    (end[..split].to_ascii_lowercase(), end[split..].parse().unwrap_or(0))
}

/// Extracts the winws command line of a strategy as raw tokens (variables kept).
pub fn parse_tokens(text: &str) -> Option<Vec<String>> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| l.to_ascii_lowercase().contains("winws.exe\""))?;
    let mut joined = String::new();
    for l in &lines[start..] {
        let t = l.trim_end();
        if let Some(stripped) = t.strip_suffix('^') {
            joined.push_str(stripped);
            joined.push(' ');
        } else {
            joined.push_str(t);
            break;
        }
    }
    let lower = joined.to_ascii_lowercase();
    let at = lower.find("winws.exe\"")? + "winws.exe\"".len();
    Some(tokenize(&joined[at..]))
}

/// Splits a cmd.exe command line: double quotes group, `^` escapes the next
/// character outside quotes (e.g. `^!` → `!`), as cmd itself does.
fn tokenize(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    let mut escaped = false;
    for ch in s.chars() {
        if escaped {
            cur.push(ch);
            any = true;
            escaped = false;
            continue;
        }
        match ch {
            '^' if !quoted => escaped = true,
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(cur);
    }
    out
}

pub fn strategies(dir: &Path) -> Vec<Strategy> {
    strategy_files(dir)
        .into_iter()
        .filter_map(|file| {
            let text = String::from_utf8_lossy(&std::fs::read(dir.join(&file)).ok()?).into_owned();
            let tokens = parse_tokens(&text)?;
            let joined = tokens.join(" ");
            let note = text
                .lines()
                .take(8)
                .filter_map(|l| l.trim().strip_prefix("::").map(str::trim))
                .find(|c| !c.is_empty() && !c.contains("65001"))
                .map(String::from);
            Some(Strategy {
                name: file.trim_end_matches(".bat").to_string(),
                note,
                game_filter: joined.contains("%GameFilter"),
                fake_discord: joined.contains("ACTIVE_DISCORD_UDP.bin"),
                fake_game: joined.contains("ACTIVE_GAME_UDP.bin"),
                file,
            })
        })
        .collect()
}

fn game_ports(cfg: &ZapretConfig) -> (String, String) {
    let none = "12".to_string();
    match cfg.game_filter.as_str() {
        "all" => (cfg.game_tcp.clone(), cfg.game_udp.clone()),
        "tcp" => (cfg.game_tcp.clone(), none),
        "udp" => (none, cfg.game_udp.clone()),
        _ => (none.clone(), none),
    }
}

/// Resolves the strategy into the exact winws.exe arguments.
pub fn build_args(dir: &Path, file: &str, cfg: &ZapretConfig) -> Result<Vec<String>, String> {
    if file.contains(['/', '\\']) || file.contains("..") {
        return Err("Неверное имя стратегии.".into());
    }
    let path = dir.join(file);
    let text = String::from_utf8_lossy(&std::fs::read(&path).map_err(|_| format!("Стратегия «{file}» не найдена."))?).into_owned();
    let tokens = parse_tokens(&text).ok_or_else(|| format!("В файле «{file}» не найдена команда winws.exe."))?;
    let bin = format!("{}\\", dir.join("bin").display());
    let lists = format!("{}\\", dir.join("lists").display());
    let root = format!("{}\\", dir.display());
    let (tcp, udp) = game_ports(cfg);
    let game = if tcp != "12" { tcp.clone() } else { udp.clone() };
    let fake = |name: &Option<String>| {
        name.as_ref().filter(|f| !f.contains(['/', '\\']) && f.ends_with(".bin") && dir.join("bin").join(f).exists()).cloned()
    };
    let fake_discord = fake(&cfg.fake_discord);
    let fake_game = fake(&cfg.fake_game);
    let mut out = Vec::with_capacity(tokens.len());
    for t in tokens {
        let mut a = t
            .replace("%BIN%", &bin)
            .replace("%LISTS%", &lists)
            .replace("%~dp0", &root)
            .replace("%GameFilterTCP%", &tcp)
            .replace("%GameFilterUDP%", &udp)
            .replace("%GameFilter%", &game);
        if let Some(f) = &fake_discord {
            a = a.replace("ACTIVE_DISCORD_UDP.bin", f);
        }
        if let Some(f) = &fake_game {
            a = a.replace("ACTIVE_GAME_UDP.bin", f);
        }
        if a.contains('%') {
            return Err(format!("Стратегия «{file}» использует неизвестную переменную: {a}"));
        }
        out.push(a);
    }
    if !crate::managed_sites::domains(&cfg.managed_sites, &cfg.managed_services, file).is_empty() {
        let existing = format!("--hostlist={lists}list-general-user.txt");
        let managed = format!("--hostlist={lists}{}", crate::managed_sites::HOST_FILE);
        if !out.iter().any(|arg| arg == &existing) {
            return Err("Эта стратегия не поддерживает каталог и пользовательские сайты VELIORA.".into());
        }
        out = out.into_iter().flat_map(|arg| {
            let user_list = arg == existing;
            std::iter::once(arg).chain(user_list.then(|| managed.clone()))
        }).collect();
    }
    crate::discord::apply_udp_profile(out, cfg.discord_udp)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FakeFile {
    pub file: String,
    pub label: String,
}

pub fn fakes(dir: &Path) -> Vec<FakeFile> {
    let mut v: Vec<FakeFile> = std::fs::read_dir(dir.join("bin"))
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(".bin") && !n.starts_with("ACTIVE_"))
                .map(|file| {
                    let base = file.trim_end_matches(".bin");
                    let label = if let Some(h) = base.strip_prefix("quic_initial_") {
                        format!("QUIC · {}", h.replace('_', "."))
                    } else if let Some(h) = base.strip_prefix("tls_clienthello_") {
                        format!("TLS · {}", h.replace('_', "."))
                    } else {
                        base.to_uppercase()
                    };
                    FakeFile { file, label }
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| a.label.cmp(&b.label));
    v
}

// ── IPSet ─────────────────────────────────────────────────────────────────

pub fn write_ipset(dir: &Path, mode: &str) -> Result<usize, String> {
    let lists = dir.join("lists");
    let user = read_entries(&lists.join("ipset-all-user.txt"));
    let mut entries: Vec<String> = match mode {
        "any" => Vec::new(),
        "full" => {
            let full = std::fs::read_to_string(lists.join("ipset-all.txt.backup")).map_err(|_| "Нет полного списка IPSet в сборке (ipset-all.txt.backup).".to_string())?;
            full.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(String::from).chain(user).collect()
        }
        _ => std::iter::once(PLACEHOLDER_IP.to_string()).chain(TELEGRAM_NETS.iter().map(|s| s.to_string())).chain(user).collect(),
    };
    dedup(&mut entries);
    let body = if entries.is_empty() { String::new() } else { format!("{}\r\n", entries.join("\r\n")) };
    std::fs::write(lists.join("ipset-all.txt"), body).map_err(|e| format!("Не удалось записать ipset-all.txt: {e}"))?;
    Ok(entries.len())
}

pub fn ipset_full_count(dir: &Path) -> usize {
    std::fs::read_to_string(dir.join("lists").join("ipset-all.txt.backup")).map(|s| s.lines().filter(|l| !l.trim().is_empty()).count()).unwrap_or(0)
}

// ── run ───────────────────────────────────────────────────────────────────

fn classify(line: &str) -> (&'static str, Option<Health>) {
    let l = line.to_ascii_lowercase();
    if l.contains("windivert initialized") || l.contains("capture is started") {
        return ("info", Some(Health::Ready));
    }
    if (l.contains("windivert") && (l.contains("fail") || l.contains("error")))
        || l.contains("access is denied")
        || l.contains("could not open")
        || l.contains("requires admin")
    {
        return ("error", Some(Health::Failed));
    }
    if l.contains("error") || l.contains("fail") {
        return ("warn", None);
    }
    ("info", None)
}

pub fn validate_install(dir: &Path, cfg: &ZapretConfig) -> Result<(), String> {
    let bin = dir.join("bin");
    if !["winws.exe", "WinDivert.dll", "WinDivert64.sys", "cygwin1.dll"].iter().all(|name| bin.join(name).is_file()) {
        return Err("В архиве отсутствуют компоненты winws или WinDivert.".into());
    }
    if !dir.join("general.bat").is_file() || !dir.join("lists").is_dir() {
        return Err("В архиве отсутствуют стратегия или списки Zapret.".into());
    }
    let Some(strategy) = &cfg.strategy else { return Ok(()); };
    ensure_user_files(dir);
    let domains = crate::managed_sites::domains(&cfg.managed_sites, &cfg.managed_services, strategy);
    if !domains.is_empty() { crate::managed_sites::write(dir, &domains)?; }
    let args = build_args(dir, strategy, cfg)?;
    let mut child = win::hidden(bin.join("winws.exe")).arg("--dry-run").args(args).current_dir(&bin)
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null()).spawn()
        .map_err(|e| format!("Не удалось проверить стратегию в выбранной версии: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("Выбранная версия отклонила стратегию «{strategy}» ({status}).")),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(80)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Проверка совместимости Zapret не завершилась.".into());
            }
        }
    }
}

pub fn launch(c: &Arc<Component>, dir: &Path, file: &str, cfg: &ZapretConfig, quiet: bool) -> Result<u32, String> {
    let args = build_args(dir, file, cfg)?;
    ensure_user_files(dir);
    let domains = crate::managed_sites::domains(&cfg.managed_sites, &cfg.managed_services, file);
    if !domains.is_empty() {
        crate::managed_sites::write(dir, &domains)?;
    }
    write_ipset(dir, &cfg.ipset_mode)?;
    let mut cmd = win::hidden(dir.join("bin").join("winws.exe"));
    cmd.args(&args).current_dir(dir.join("bin"));
    if !quiet {
        logs::info("zapret", format!("запуск стратегии «{}», Discord UDP: {:?}", file.trim_end_matches(".bat"), cfg.discord_udp));
    }
    c.spawn(cmd, if quiet { classify_quiet } else { classify })
}

fn classify_quiet(line: &str) -> (&'static str, Option<Health>) {
    let (_, h) = classify(line);
    ("info", h)
}

/// Waits until winws reports a live capture. Ok(true) = confirmed, Ok(false) = alive but silent.
pub fn wait_ready(c: &Arc<Component>, timeout: Duration) -> Result<bool, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match c.health() {
            Health::Ready => return Ok(true),
            Health::Failed => return Err(c.tail()),
            Health::Unknown => {}
        }
        if !c.alive() {
            return Err(c.tail());
        }
        if Instant::now() > deadline {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(80));
    }
}

pub fn preflight(own_pid: Option<u32>) -> Result<(), String> {
    if !win::is_admin() {
        return Err("Zapret работает только с правами администратора. Нажмите «Запросить права» или перезапустите VELIORA App.".into());
    }
    let conflicts = crate::zapret_conflicts::scan(own_pid, false);
    if let Some(item) = conflicts.first() {
        let reason = if item.confirmed { "WinDivert используется другой программой" } else { "Не удалось проверить другой сетевой процесс" };
        return Err(format!("{reason}: {}. Откройте диагностику конфликта.", item.label));
    }
    if let Some(bfe) = win::service("BFE") {
        if !bfe.running() {
            return Err("Служба Windows «Базовый механизм фильтрации» (BFE) остановлена — без неё драйвер WinDivert не работает. Откройте диагностику на главной.".into());
        }
    }
    Ok(())
}

fn filter_in_use(tail: &str) -> bool {
    let text = tail.to_ascii_lowercase();
    text.contains("0x8032000a") || text.contains("fwp_e_in_use") ||
        (text.contains("windivert") && text.contains("object is referenced by other objects"))
}

pub fn start(c: &Arc<Component>, resources: &Path, cfg: &ZapretConfig) -> Result<(), String> {
    let _guard = c.op.lock();
    if c.alive() {
        return if c.status().state == "running" { Ok(()) } else { Err("Предыдущий процесс Zapret ещё завершается.".into()) };
    }
    let Some(file) = cfg.strategy.clone() else {
        return Err("Выберите стратегию на странице Zapret.".into());
    };
    c.set_status(|s| {
        s.state = "starting";
        s.started_at = Some(win::now_ms());
        s.last_error = None;
        s.detail = Some(file.trim_end_matches(".bat").to_string());
    });
    prepare(resources).map_err(|e| c.fail(e))?;
    let dir = runtime();
    preflight(None).map_err(|e| c.fail(e))?;
    for attempt in 0..2 {
        let pid = launch(c, &dir, &file, cfg, false).map_err(|e| c.fail(e))?;
        match wait_ready(c, Duration::from_secs(5)) {
            Ok(true) if c.mark_capture_ready(pid) => {
                logs::info("zapret", "Перехват WinDivert подтверждён.");
                return Ok(());
            }
            outcome => {
                let tail = match outcome { Err(tail) => tail, _ => c.tail() };
                c.kill();
                if c.alive() {
                    return Err(c.fail("Предыдущий процесс VELIORA ещё завершается. Повторите попытку позже."));
                }
                let filter_busy = filter_in_use(&tail);
                if filter_busy && attempt == 0 && cfg.auto_resolve_conflicts {
                    logs::warn("zapret", "Фильтр WinDivert занят. После освобождения ресурсов проверяем запуск ещё раз.");
                    std::thread::sleep(Duration::from_millis(800));
                    preflight(None).map_err(|e| c.fail(e))?;
                    continue;
                }
                let message = if filter_busy {
                    "Фильтр WinDivert всё ещё занят. Закройте другую программу обхода и повторите попытку. Если она закрыта, перезагрузите Windows."
                } else {
                    "Не удалось подтвердить запуск WinDivert. Проверьте подробности в журнале и откройте диагностику Zapret."
                };
                return Err(c.fail(message));
            }
        }
    }
    Err(c.fail("Не удалось запустить Zapret."))
}

pub fn stop(c: &Arc<Component>) {
    let _guard = c.op.lock();
    if c.alive() {
        c.set_status(|s| s.state = "stopping");
        c.kill();
    }
    c.mark_stopped();
}

// ── lists ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListInfo {
    pub file: &'static str,
    pub title: &'static str,
    pub desc: &'static str,
    /// domains | ips
    pub kind: &'static str,
    pub user: bool,
    pub count: usize,
    pub has_backup: bool,
}

const LISTS: [(&str, &str, &str, &str, bool); 8] = [
    ("list-general-user.txt", "Мои домены", "Сайты, к которым применяется обход. Поддомены включаются автоматически.", "domains", true),
    ("ipset-all-user.txt", "Мои IP-адреса", "Адреса и подсети для обхода по IP. Добавляются к выбранному режиму IPSet.", "ips", true),
    ("list-exclude-user.txt", "Мои исключения: домены", "Сайты, которые обход не трогает.", "domains", true),
    ("ipset-exclude-user.txt", "Мои исключения: IP", "Адреса, которые обход не трогает.", "ips", true),
    ("list-general.txt", "Домены сборки", "Основной список сборки Flowseal: Discord, YouTube и другие.", "domains", false),
    ("list-google.txt", "Домены Google", "YouTube и Google — для отдельного правила стратегии.", "domains", false),
    ("list-exclude.txt", "Исключения сборки", "Домены, которые сборка не обрабатывает.", "domains", false),
    ("ipset-exclude.txt", "IP-исключения сборки", "Локальные и служебные сети.", "ips", false),
];

fn list_meta(file: &str) -> Result<(&'static str, &'static str, bool), String> {
    LISTS.iter().find(|l| l.0 == file).map(|l| (l.0, l.3, l.4)).ok_or_else(|| "Неизвестный файл списка.".to_string())
}

pub fn read_entries(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && *l != PLACEHOLDER_DOMAIN && *l != PLACEHOLDER_IP)
        .map(String::from)
        .collect()
}

fn dedup(v: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    v.retain(|x| seen.insert(x.to_ascii_lowercase()));
}

pub fn valid_domain_entry(s: &str) -> bool {
    let d = s.strip_prefix('^').unwrap_or(s);
    d.contains('.') && d.parse::<std::net::IpAddr>().is_err() && crate::config::valid_host(d)
}

pub fn valid_ip_entry(s: &str) -> bool {
    let (ip, prefix) = match s.split_once('/') {
        Some((a, p)) => (a, Some(p)),
        None => (s, None),
    };
    match ip.parse::<std::net::IpAddr>() {
        Ok(addr) => match prefix {
            None => true,
            Some(p) => p.parse::<u8>().is_ok_and(|n| n <= if addr.is_ipv4() { 32 } else { 128 }),
        },
        Err(_) => false,
    }
}

pub fn lists_info(dir: &Path) -> Vec<ListInfo> {
    let lists = dir.join("lists");
    LISTS
        .iter()
        .map(|(file, title, desc, kind, user)| ListInfo {
            file,
            title,
            desc,
            kind,
            user: *user,
            count: read_entries(&lists.join(file)).len(),
            has_backup: lists.join(format!("{file}.velbak")).exists(),
        })
        .collect()
}

pub fn list_read(dir: &Path, file: &str) -> Result<Vec<String>, String> {
    let (f, _, _) = list_meta(file)?;
    Ok(read_entries(&dir.join("lists").join(f)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteResult {
    pub saved: usize,
    pub rejected: Vec<String>,
}

pub fn list_write(dir: &Path, file: &str, entries: Vec<String>) -> Result<WriteResult, String> {
    let (f, kind, _) = list_meta(file)?;
    let path = dir.join("lists").join(f);
    let mut good = Vec::new();
    let mut rejected = Vec::new();
    for raw in entries.iter().flat_map(|e| e.split([' ', '\t', ',', ';', '\n', '\r'])) {
        let e = raw.trim().trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/').to_string();
        if e.is_empty() || e.starts_with('#') {
            continue;
        }
        let ok = if kind == "ips" { valid_ip_entry(&e) } else { valid_domain_entry(&e) };
        if ok {
            good.push(if kind == "domains" { e.to_ascii_lowercase() } else { e });
        } else {
            rejected.push(e);
        }
    }
    dedup(&mut good);
    let backup = path.with_file_name(format!("{f}.velbak"));
    if path.exists() && !backup.exists() {
        let _ = std::fs::copy(&path, &backup);
    }
    let mut body = String::new();
    if good.is_empty() {
        match f {
            "list-general-user.txt" => body.push_str(&format!("# Never leave this file empty\r\n{PLACEHOLDER_DOMAIN}\r\n")),
            "list-exclude-user.txt" | "list-general.txt" | "list-google.txt" | "list-exclude.txt" => body.push_str(&format!("{PLACEHOLDER_DOMAIN}\r\n")),
            "ipset-exclude-user.txt" | "ipset-exclude.txt" => body.push_str(&format!("{PLACEHOLDER_IP}\r\n")),
            _ => {}
        }
    } else {
        body = format!("{}\r\n", good.join("\r\n"));
    }
    std::fs::write(&path, body).map_err(|e| format!("Не удалось сохранить список: {e}"))?;
    Ok(WriteResult { saved: good.len(), rejected })
}

pub fn list_restore_backup(dir: &Path, file: &str) -> Result<(), String> {
    let (f, _, _) = list_meta(file)?;
    let path = dir.join("lists").join(f);
    let backup = path.with_file_name(format!("{f}.velbak"));
    std::fs::copy(&backup, &path).map_err(|_| "Резервной копии этого списка нет.".to_string())?;
    let _ = std::fs::remove_file(backup);
    Ok(())
}

pub fn lists_factory_reset(dir: &Path) -> Result<usize, String> {
    let lists = dir.join("lists");
    let originals = dir.join(".originals").join("lists");
    let mut n = 0;
    let rd = std::fs::read_dir(&originals).map_err(|_| "Заводские копии списков не найдены — переустановите сборку Zapret.".to_string())?;
    for e in rd.flatten() {
        if std::fs::copy(e.path(), lists.join(e.file_name())).is_ok() {
            n += 1;
        }
    }
    for f in USER_FILES {
        let _ = std::fs::remove_file(lists.join(f));
    }
    if let Ok(rd) = std::fs::read_dir(&lists) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().ends_with(".velbak") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    ensure_user_files(dir);
    Ok(n)
}

// ── Discord cache ─────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordCacheResult {
    pub installed: bool,
    pub closed: Vec<String>,
    pub removed: usize,
    pub failed: Vec<String>,
}

pub fn clear_discord_cache() -> DiscordCacheResult {
    let mut closed = Vec::new();
    for name in ["Discord.exe", "DiscordPTB.exe", "DiscordCanary.exe"] {
        let procs = win::find_processes(name);
        if !procs.is_empty() {
            for p in procs {
                win::kill_pid(p.pid);
            }
            closed.push(name.trim_end_matches(".exe").to_string());
        }
    }
    if !closed.is_empty() {
        std::thread::sleep(Duration::from_millis(900));
    }
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default();
    let mut installed = false;
    let mut removed = 0;
    let mut failed = Vec::new();
    for app in ["discord", "discordptb", "discordcanary"] {
        let root = appdata.join(app);
        if !root.exists() {
            continue;
        }
        installed = true;
        for sub in ["Cache", "Code Cache", "GPUCache"] {
            let p = root.join(sub);
            if p.exists() {
                match std::fs::remove_dir_all(&p) {
                    Ok(_) => removed += 1,
                    Err(_) => failed.push(p.display().to_string()),
                }
            }
        }
    }
    logs::info("zapret", format!("кеш Discord: удалено папок {removed}, закрыто {:?}", closed));
    DiscordCacheResult { installed, closed, removed, failed }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAT: &str = "@echo off\r\nchcp 65001 > nul\r\n:: 65001 - UTF-8\r\n:: NOT RECOMMENDED\r\n\r\nset \"BIN=%~dp0bin\\\"\r\nstart \"zapret: %~n0\" /min \"%BIN%winws.exe\" --wf-tcp=80,443,%GameFilterTCP% --wf-udp=443,%GameFilterUDP% ^\r\n--filter-udp=443 --hostlist=\"%LISTS%list-general.txt\" --dpi-desync=fake --dpi-desync-fake-quic=\"%BIN%quic_initial_www_google_com.bin\" --new ^\r\n--filter-udp=50000-50100 --dpi-desync-fake-discord=\"%BIN%ACTIVE_DISCORD_UDP.bin\"\r\n";

    #[test]
    fn parses_strategy_line() {
        let t = parse_tokens(BAT).unwrap();
        assert_eq!(t[0], "--wf-tcp=80,443,%GameFilterTCP%");
        assert_eq!(t[3], "--hostlist=%LISTS%list-general.txt");
        assert_eq!(t.last().unwrap(), "--dpi-desync-fake-discord=%BIN%ACTIVE_DISCORD_UDP.bin");
        assert!(t.iter().all(|x| !x.contains('^')));
        assert_eq!(tokenize(r#"--a=^! --b="x^y" --c=^^"#), vec!["--a=!", "--b=x^y", "--c=^"]);
    }

    #[test]
    fn validates_entries() {
        assert!(valid_domain_entry("discord.com"));
        assert!(valid_domain_entry("^example.com"));
        assert!(!valid_domain_entry("1.2.3.4"));
        assert!(!valid_domain_entry("localhost"));
        assert!(valid_ip_entry("1.2.3.4"));
        assert!(valid_ip_entry("149.154.160.0/20"));
        assert!(valid_ip_entry("2001:db8::/32"));
        assert!(!valid_ip_entry("1.2.3.4/33"));
        assert!(!valid_ip_entry("example.com"));
    }

    #[test]
    fn managed_hostlist_extends_only_user_hostlist_sections() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("components").join("zapret");
        let mut cfg = ZapretConfig::default();
        let baseline = build_args(&dir, "general.bat", &cfg).unwrap();
        cfg.managed_services.push("github".into());
        cfg.managed_sites.push(crate::managed_sites::Site {
            name: None, domain: "example.org".into(), enabled: true, strategy: Some("general.bat".into()),
        });
        let updated = build_args(&dir, "general.bat", &cfg).unwrap();
        let mut restored = Vec::new();
        let mut previous = "";
        let mut inserted = 0;
        for arg in &updated {
            if arg.ends_with(crate::managed_sites::HOST_FILE) {
                assert!(previous.ends_with("list-general-user.txt"));
                inserted += 1;
            } else {
                restored.push(arg.clone());
            }
            previous = arg;
        }
        assert!(inserted > 0);
        assert_eq!(baseline, restored);
        assert!(baseline.iter().any(|arg| arg.ends_with("list-google.txt")));
        assert!(read_entries(&dir.join("lists/list-google.txt")).iter().any(|entry| entry.contains("youtube")));
    }

    #[test]
    fn real_bundle_resolves() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("components").join("zapret");
        if !dir.exists() {
            return;
        }
        let list = strategies(&dir);
        assert!(list.len() >= 10, "found {}", list.len());
        let mut cfg = ZapretConfig::default();
        for s in &list {
            let args = build_args(&dir, &s.file, &cfg).unwrap_or_else(|e| panic!("{}: {e}", s.file));
            assert!(args.iter().any(|a| a.starts_with("--wf-tcp=")), "{}", s.file);
        }
        cfg.game_filter = "all".into();
        cfg.fake_discord = Some("stun.bin".into());
        let args = build_args(&dir, "general.bat", &cfg).unwrap();
        assert!(args.iter().any(|a| a.contains("1024-65535")));
        assert!(!args.iter().any(|a| a.contains("ACTIVE_DISCORD_UDP")) || !list.iter().find(|s| s.file == "general.bat").unwrap().fake_discord);
    }
}

#[cfg(test)]
mod winws_args {
    use super::*;

    /// Runs the real winws.exe (unelevated) with every strategy's resolved
    /// arguments: it must get past argument parsing and stop only at the
    /// driver, which needs admin rights. `cargo test -- --ignored winws`
    #[test]
    #[ignore]
    fn winws_parses_every_strategy() {
        // Work on a copy prepared exactly like the runtime folder (user lists, ipset).
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("components").join("zapret");
        let dir = std::env::temp_dir().join(format!("veliora-winws-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy_dir(&src, &dir).unwrap();
        ensure_user_files(&dir);
        let mut cfg = ZapretConfig::default();
        for discord_udp in [
            crate::config::DiscordUdp::Bundle,
            crate::config::DiscordUdp::Direct,
            crate::config::DiscordUdp::Discovery,
        ] {
            cfg.discord_udp = discord_udp;
            for (mode, fake) in [("off", None), ("all", Some("stun.bin".to_string()))] {
                cfg.game_filter = mode.into();
                cfg.fake_discord = fake.clone();
                cfg.fake_game = fake;
                for ipset in ["basic", "full", "any"] {
                    cfg.ipset_mode = ipset.into();
                    write_ipset(&dir, ipset).unwrap();
                    for s in strategies(&dir) {
                        let args = build_args(&dir, &s.file, &cfg).unwrap();
                        // --dry-run: winws validates every option and exits. RunAsInvoker only skips the
                        // manifest's elevation request; the check itself needs no rights.
                        let out = std::process::Command::new(dir.join("bin").join("winws.exe"))
                            .arg("--dry-run")
                            .args(&args)
                            .env("__COMPAT_LAYER", "RunAsInvoker")
                            .current_dir(dir.join("bin"))
                            .output()
                            .unwrap();
                        let text = format!(
                            "{}{}",
                            String::from_utf8_lossy(&out.stdout),
                            String::from_utf8_lossy(&out.stderr)
                        )
                        .to_lowercase();
                        let parse_error = text.contains("invalid")
                            || text.contains("unknown option")
                            || text.contains("unrecognized")
                            || text.contains("bad value")
                            || text.contains("could not read");
                        let at_driver = out.status.success() || text.contains("windivert");
                        println!(
                            "{mode:>3} {ipset:<5} {:<34} exit={:?} {}",
                            s.name,
                            out.status.code(),
                            text.lines().last().unwrap_or("")
                        );
                        assert!(
                            !parse_error && at_driver,
                            "{} ({mode}/{ipset}): {text}",
                            s.name
                        );
                    }
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
