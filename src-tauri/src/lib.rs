mod autorun;
mod component;
mod config;
mod diag;
mod logs;
mod proxy;
mod tester;
mod tgws;
mod updates;
mod win;
mod zapret;

use component::{Component, CoreStatus};
use config::Config;
use parking_lot::Mutex;
use serde::Serialize;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};

pub const APP_NAME: &str = "VELIORA App";
const TRAY_ID: &str = "veliora-tray";

static ELEVATION_DECLINED: AtomicBool = AtomicBool::new(false);
static ELEVATION_ERROR: AtomicU32 = AtomicU32::new(0);
static LAUNCHED_BY_AUTOSTART: AtomicBool = AtomicBool::new(false);

struct TrayItems {
    show: MenuItem<tauri::Wry>,
    tgws: MenuItem<tauri::Wry>,
    zapret: MenuItem<tauri::Wry>,
    proxy: MenuItem<tauri::Wry>,
}

/// Runtime state shared between the UI, the tray and shutdown.
struct AppState {
    /// System proxy settings that were in place before VELIORA connected.
    snapshot: Mutex<Option<proxy::Snapshot>>,
    proxy_label: Mutex<Option<String>>,
    config: Mutex<Config>,
    store: OnceLock<config::Store>,
    resources: OnceLock<PathBuf>,
    data_dir: OnceLock<PathBuf>,
    tgws: Arc<Component>,
    zapret: Arc<Component>,
    tester: Arc<tester::Tester>,
    tray: Mutex<Option<TrayItems>>,
    quitting: AtomicBool,
    shut_down: AtomicBool,
}

impl AppState {
    fn new() -> Self {
        Self {
            snapshot: Mutex::new(None),
            proxy_label: Mutex::new(None),
            config: Mutex::new(Config::default()),
            store: OnceLock::new(),
            resources: OnceLock::new(),
            data_dir: OnceLock::new(),
            tgws: Arc::new(Component::new("tgws")),
            zapret: Arc::new(Component::new("zapret")),
            tester: Arc::new(tester::Tester::default()),
            tray: Mutex::new(None),
            quitting: AtomicBool::new(false),
            shut_down: AtomicBool::new(false),
        }
    }
    fn resources(&self) -> PathBuf {
        self.resources.get().cloned().unwrap_or_default()
    }
    fn cfg(&self) -> Config {
        self.config.lock().clone()
    }
}

/// Runs blocking work (processes, network, files) off the async runtime.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.await.expect("worker thread stopped")
}

// ── app ───────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    admin: bool,
    elevation_declined: bool,
    elevation_error: u32,
    autostarted: bool,
    data_dir: String,
    zapret_bundled: String,
    zapret_installed: String,
    zapret_source: String,
    zapret_dir: String,
    tgws_version: String,
    autorun_registered: bool,
}

fn component_version(res: &std::path::Path, key: &str) -> String {
    std::fs::read(res.join("components").join("components.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v[key]["version"].as_str().map(String::from))
        .unwrap_or_else(|| "?".into())
}

#[tauri::command]
async fn app_info(app: AppHandle) -> AppInfo {
    blocking(move || {
        let st = app.state::<AppState>();
        let res = st.resources();
        let state = zapret::bundle_state(&zapret::runtime());
        AppInfo {
            version: app.package_info().version.to_string(),
            admin: win::is_admin(),
            elevation_declined: ELEVATION_DECLINED.load(Ordering::Relaxed),
            elevation_error: ELEVATION_ERROR.load(Ordering::Relaxed),
            autostarted: LAUNCHED_BY_AUTOSTART.load(Ordering::Relaxed),
            data_dir: st.data_dir.get().map(|d| d.display().to_string()).unwrap_or_default(),
            zapret_bundled: zapret::bundled_version(&res),
            zapret_installed: zapret::installed_version(&res),
            zapret_source: state.map(|s| s.source).unwrap_or_else(|| "bundled".into()),
            zapret_dir: zapret::active_dir(&res).display().to_string(),
            tgws_version: component_version(&res, "tgws"),
            autorun_registered: autorun::is_enabled(),
        }
    })
    .await
}

fn relaunch(app: &AppHandle, elevated: bool) -> Result<(), String> {
    // The new instance waits for this one to exit (single-instance lock) and
    // never re-asks for elevation itself.
    let args = vec![format!("--wait-parent={}", std::process::id()), "--no-elevate".to_string()];
    let result = if elevated {
        win::relaunch_elevated(&args).map_err(|code| if code == 1223 { "Запрос прав отклонён.".to_string() } else { format!("Windows отклонила запрос прав (код {code}).") })
    } else {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        std::process::Command::new(exe).args(&args).spawn().map(|_| ()).map_err(|e| e.to_string())
    };
    result?;
    shutdown(app);
    app.exit(0);
    Ok(())
}

#[tauri::command]
async fn app_elevate(app: AppHandle) -> Result<(), String> {
    blocking(move || relaunch(&app, true)).await
}

#[tauri::command]
async fn app_restart(app: AppHandle) -> Result<(), String> {
    blocking(move || relaunch(&app, false)).await
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let ok = url.starts_with("tg://proxy?") || url.starts_with("https://");
    if !ok || url.contains(['"', '\n', '\r']) {
        return Err("Эта ссылка не открывается".into());
    }
    win::shell_execute(None, &url, None, None).map_err(|_| "Не удалось открыть ссылку. Скопируйте её и откройте вручную.".to_string())
}

// ── config ────────────────────────────────────────────────────────────────

#[tauri::command]
fn config_get(state: State<'_, AppState>) -> Config {
    state.cfg()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigResult {
    config: Config,
    /// Components whose running instance uses outdated settings.
    restart: Vec<&'static str>,
}

#[tauri::command]
async fn config_set(app: AppHandle, config: Config) -> Result<ConfigResult, String> {
    blocking(move || apply_config(&app, config)).await
}

fn apply_config(app: &AppHandle, mut next: Config) -> Result<ConfigResult, String> {
    let st = app.state::<AppState>();
    next.tgws.host = next.tgws.host.trim().to_string();
    next.tgws.worker_domain = next.tgws.worker_domain.trim().trim_start_matches("https://").trim_end_matches('/').to_string();
    next.validate()?;
    if !next.app.tray {
        next.app.silent_start = false;
        next.app.hide_taskbar = false;
    }
    let prev = st.cfg();
    if next.app.launch_at_login != prev.app.launch_at_login {
        let r = if next.app.launch_at_login { autorun::enable() } else { autorun::disable() };
        r?;
    }
    if let Some(store) = st.store.get() {
        store.save(&next)?;
    }
    *st.config.lock() = next.clone();

    if next.app.tray != prev.app.tray {
        if let Some(t) = app.tray_by_id(TRAY_ID) {
            let _ = t.set_visible(next.app.tray);
        }
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_skip_taskbar(next.app.tray && next.app.hide_taskbar);
    }
    let mut restart = Vec::new();
    let tg_changed = next.tgws.host != prev.tgws.host || next.tgws.port != prev.tgws.port || next.tgws.secret != prev.tgws.secret || next.tgws.worker_domain != prev.tgws.worker_domain || next.tgws.verbose != prev.tgws.verbose;
    if tg_changed && st.tgws.alive() {
        restart.push("tgws");
    }
    let z = (&next.zapret, &prev.zapret);
    let zap_changed = z.0.strategy != z.1.strategy || z.0.game_filter != z.1.game_filter || z.0.game_tcp != z.1.game_tcp || z.0.game_udp != z.1.game_udp || z.0.ipset_mode != z.1.ipset_mode || z.0.fake_discord != z.1.fake_discord || z.0.fake_game != z.1.fake_game;
    if zap_changed && st.zapret.alive() {
        restart.push("zapret");
    }
    let _ = app.emit("config://changed", &next);
    refresh_tray(app);
    Ok(ConfigResult { config: next, restart })
}

#[tauri::command]
async fn config_factory_reset(app: AppHandle) -> Result<Config, String> {
    blocking(move || {
        let st = app.state::<AppState>();
        tgws::stop(&st.tgws);
        zapret::stop(&st.zapret);
        st.tester.reset();
        let _ = autorun::disable();
        let mut fresh = Config::default();
        // Keep the look the user chose; everything else back to defaults.
        let prev = st.cfg();
        fresh.app.reduce_transparency = prev.app.reduce_transparency;
        fresh.appearance = prev.appearance.clone();
        apply_config(&app, fresh).map(|r| r.config)
    })
    .await
}

// ── logs ──────────────────────────────────────────────────────────────────

#[tauri::command]
fn logs_history() -> Vec<logs::LogEntry> {
    logs::history()
}
#[tauri::command]
fn logs_clear() {
    logs::clear()
}
#[tauri::command]
fn logs_export(path: String) -> Result<usize, String> {
    logs::export(&path)
}
#[tauri::command]
fn log_ui(level: String, text: String) {
    let lvl = match level.as_str() {
        "error" => "error",
        "warn" => "warn",
        _ => "info",
    };
    logs::log("app", lvl, text);
}

// ── Telegram ──────────────────────────────────────────────────────────────

#[tauri::command]
fn core_status(state: State<'_, AppState>) -> serde_json::Value {
    serde_json::json!({ "tgws": state.tgws.status(), "zapret": state.zapret.status() })
}

#[tauri::command]
async fn tgws_start(app: AppHandle) -> Result<CoreStatus, String> {
    blocking(move || {
        let st = app.state::<AppState>();
        let cfg = st.cfg().tgws;
        tgws::start(&st.tgws, &cfg, &tgws::binary(&st.resources()))?;
        Ok(st.tgws.status())
    })
    .await
}

#[tauri::command]
async fn tgws_stop(app: AppHandle) -> Result<CoreStatus, String> {
    blocking(move || {
        let st = app.state::<AppState>();
        tgws::stop(&st.tgws);
        Ok(st.tgws.status())
    })
    .await
}

#[tauri::command]
async fn tgws_restart(app: AppHandle) -> Result<CoreStatus, String> {
    blocking(move || {
        let st = app.state::<AppState>();
        tgws::stop(&st.tgws);
        std::thread::sleep(Duration::from_millis(250));
        let cfg = st.cfg().tgws;
        tgws::start(&st.tgws, &cfg, &tgws::binary(&st.resources()))?;
        Ok(st.tgws.status())
    })
    .await
}

#[tauri::command]
fn tgws_link(state: State<'_, AppState>) -> String {
    tgws::link(&state.cfg().tgws)
}

#[tauri::command]
async fn tgws_regenerate_secret(app: AppHandle) -> Result<String, String> {
    blocking(move || {
        let st = app.state::<AppState>();
        let mut cfg = st.cfg();
        cfg.tgws.secret = win::random_hex(16);
        let r = apply_config(&app, cfg)?;
        if r.restart.contains(&"tgws") {
            tgws::stop(&st.tgws);
            tgws::start(&st.tgws, &r.config.tgws, &tgws::binary(&st.resources()))?;
        }
        logs::info("tgws", "ключ и ссылка подключения изменены");
        Ok(tgws::link(&r.config.tgws))
    })
    .await
}

// ── Zapret ────────────────────────────────────────────────────────────────

#[tauri::command]
async fn zapret_start(app: AppHandle) -> Result<CoreStatus, String> {
    blocking(move || {
        let st = app.state::<AppState>();
        if st.tester.is_running() {
            return Err("Идёт тестирование стратегий — дождитесь окончания.".into());
        }
        let cfg = st.cfg().zapret;
        zapret::start(&st.zapret, &st.resources(), &cfg)?;
        Ok(st.zapret.status())
    })
    .await
}

#[tauri::command]
async fn zapret_stop(app: AppHandle) -> Result<CoreStatus, String> {
    blocking(move || {
        let st = app.state::<AppState>();
        if st.tester.is_running() {
            return Err("Идёт тестирование стратегий — дождитесь окончания.".into());
        }
        zapret::stop(&st.zapret);
        Ok(st.zapret.status())
    })
    .await
}

#[tauri::command]
async fn zapret_restart(app: AppHandle) -> Result<CoreStatus, String> {
    blocking(move || {
        let st = app.state::<AppState>();
        if st.tester.is_running() {
            return Err("Идёт тестирование стратегий — дождитесь окончания.".into());
        }
        zapret::stop(&st.zapret);
        std::thread::sleep(Duration::from_millis(400));
        let cfg = st.cfg().zapret;
        zapret::start(&st.zapret, &st.resources(), &cfg)?;
        Ok(st.zapret.status())
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ZapretOverview {
    strategies: Vec<zapret::Strategy>,
    fakes: Vec<zapret::FakeFile>,
    ipset_full: usize,
    version: String,
    source: String,
    ready: bool,
    admin: bool,
}

#[tauri::command]
async fn zapret_overview(app: AppHandle) -> ZapretOverview {
    blocking(move || {
        let st = app.state::<AppState>();
        let res = st.resources();
        let dir = zapret::active_dir(&res);
        ZapretOverview {
            strategies: zapret::strategies(&dir),
            fakes: zapret::fakes(&dir),
            ipset_full: zapret::ipset_full_count(&dir),
            version: zapret::installed_version(&res),
            source: zapret::bundle_state(&zapret::runtime()).map(|s| s.source).unwrap_or_else(|| "bundled".into()),
            ready: dir == zapret::runtime(),
            admin: win::is_admin(),
        }
    })
    .await
}

fn writable_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let st = app.state::<AppState>();
    zapret::prepare(&st.resources()).map_err(|_| "Изменять списки Zapret можно только с правами администратора.".to_string())?;
    Ok(zapret::runtime())
}

#[tauri::command]
async fn lists_info(app: AppHandle) -> Vec<zapret::ListInfo> {
    blocking(move || zapret::lists_info(&zapret::active_dir(&app.state::<AppState>().resources()))).await
}

#[tauri::command]
async fn list_read(app: AppHandle, file: String) -> Result<Vec<String>, String> {
    blocking(move || zapret::list_read(&zapret::active_dir(&app.state::<AppState>().resources()), &file)).await
}

#[tauri::command]
async fn list_write(app: AppHandle, file: String, entries: Vec<String>) -> Result<zapret::WriteResult, String> {
    blocking(move || zapret::list_write(&writable_dir(&app)?, &file, entries)).await
}

#[tauri::command]
async fn list_import(app: AppHandle, file: String, path: String) -> Result<zapret::WriteResult, String> {
    blocking(move || {
        let meta = std::fs::metadata(&path).map_err(|_| "Файл не найден.".to_string())?;
        if meta.len() > 8 * 1024 * 1024 {
            return Err("Файл слишком большой (больше 8 МБ).".into());
        }
        let text = String::from_utf8_lossy(&std::fs::read(&path).map_err(|e| e.to_string())?).into_owned();
        let entries: Vec<String> = text.lines().map(str::to_string).collect();
        zapret::list_write(&writable_dir(&app)?, &file, entries)
    })
    .await
}

#[tauri::command]
async fn list_export(app: AppHandle, file: String, path: String) -> Result<usize, String> {
    blocking(move || {
        let entries = zapret::list_read(&zapret::active_dir(&app.state::<AppState>().resources()), &file)?;
        std::fs::write(&path, format!("{}\r\n", entries.join("\r\n"))).map_err(|e| format!("Не удалось сохранить файл: {e}"))?;
        Ok(entries.len())
    })
    .await
}

#[tauri::command]
async fn list_restore_backup(app: AppHandle, file: String) -> Result<(), String> {
    blocking(move || zapret::list_restore_backup(&writable_dir(&app)?, &file)).await
}

#[tauri::command]
async fn lists_factory_reset(app: AppHandle) -> Result<usize, String> {
    blocking(move || zapret::lists_factory_reset(&writable_dir(&app)?)).await
}

#[tauri::command]
async fn discord_clear_cache() -> zapret::DiscordCacheResult {
    blocking(zapret::clear_discord_cache).await
}

// ── tester ────────────────────────────────────────────────────────────────

#[tauri::command]
async fn tester_run(app: AppHandle) -> Result<(), String> {
    blocking(move || {
        let st = app.state::<AppState>();
        tester::run(app.clone(), st.tester.clone(), st.zapret.clone(), st.resources(), st.cfg().zapret)
    })
    .await
}
#[tauri::command]
fn tester_cancel(state: State<'_, AppState>) {
    state.tester.cancel();
}
#[tauri::command]
fn tester_state(state: State<'_, AppState>) -> serde_json::Value {
    serde_json::json!({ "running": state.tester.is_running(), "report": state.tester.report() })
}
#[tauri::command]
fn tester_reset(state: State<'_, AppState>) -> Result<(), String> {
    if state.tester.is_running() {
        return Err("Тест ещё идёт.".into());
    }
    state.tester.reset();
    Ok(())
}

// ── updates ───────────────────────────────────────────────────────────────

#[tauri::command]
async fn updates_zapret_releases() -> Result<Vec<updates::Release>, String> {
    blocking(updates::zapret_releases).await
}

#[tauri::command]
async fn updates_zapret_install(app: AppHandle, tag: String) -> Result<updates::InstallResult, String> {
    blocking(move || {
        let st = app.state::<AppState>();
        if st.tester.is_running() {
            return Err("Идёт тестирование стратегий — дождитесь окончания.".into());
        }
        let mut cfg = st.cfg();
        let r = updates::install_zapret(&tag, &st.resources(), &st.zapret, &cfg.zapret)?;
        // Forget a strategy that no longer exists in the new version.
        if cfg.zapret.strategy.as_ref().is_some_and(|s| !zapret::runtime().join(s).exists()) {
            cfg.zapret.strategy = None;
            let _ = apply_config(&app, cfg);
        }
        st.tester.reset();
        Ok(r)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCheck {
    zapret_installed: String,
    zapret_latest: Option<String>,
    zapret_update: bool,
    tgws_installed: String,
    tgws_latest: Option<String>,
    tgws_update: bool,
    errors: Vec<String>,
}

#[tauri::command]
async fn updates_check(app: AppHandle) -> UpdateCheck {
    blocking(move || {
        let st = app.state::<AppState>();
        let res = st.resources();
        let zi = zapret::installed_version(&res);
        let ti = component_version(&res, "tgws");
        let mut errors = Vec::new();
        let zl = match updates::zapret_releases() {
            Ok(list) => list.first().map(|r| r.tag.clone()),
            Err(e) => {
                errors.push(format!("Zapret: {e}"));
                None
            }
        };
        let tl = match updates::tgws_latest() {
            Ok(v) => Some(v),
            Err(e) => {
                errors.push(format!("TgWsProxy: {e}"));
                None
            }
        };
        UpdateCheck {
            zapret_update: zl.as_ref().is_some_and(|l| updates::newer(l, &zi)),
            tgws_update: tl.as_ref().is_some_and(|l| updates::newer(l, &ti)),
            zapret_installed: zi,
            zapret_latest: zl,
            tgws_installed: ti,
            tgws_latest: tl,
            errors,
        }
    })
    .await
}

// ── diagnostics ───────────────────────────────────────────────────────────

#[tauri::command]
async fn diag_run(app: AppHandle) -> diag::DiagReport {
    blocking(move || {
        let st = app.state::<AppState>();
        let res = st.resources();
        let cfg = st.cfg();
        let own_proxy = st.snapshot.lock().is_some();
        diag::run(&diag::Ctx { resources: &res, tgws: &cfg.tgws, tgws_running: st.tgws.alive(), own_proxy_active: own_proxy })
    })
    .await
}

#[tauri::command]
fn diag_open(kind: String) -> Result<(), String> {
    diag::open(&kind)
}

// ── system proxy (profiles) ───────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectResult {
    server: String,
}

#[tauri::command]
fn proxy_status() -> Result<proxy::ProxyState, String> {
    proxy::read()
}

#[tauri::command]
fn proxy_connect(app: AppHandle, state: State<'_, AppState>, kind: String, host: String, port: u16, bypass: String) -> Result<ConnectResult, String> {
    let server = proxy::server_string(&kind, &host, port)?;
    {
        let mut snap = state.snapshot.lock();
        if snap.is_none() {
            let s = proxy::snapshot()?;
            persist_snapshot(&app, Some(&s));
            *snap = Some(s);
        }
    }
    proxy::set(&server, &bypass)?;
    logs::info("proxy", format!("системный прокси включён: {server}"));
    Ok(ConnectResult { server })
}

#[tauri::command]
fn proxy_disconnect(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let snap = state.snapshot.lock().take();
    if let Some(snap) = snap {
        proxy::restore(&snap)?;
        persist_snapshot(&app, None);
        logs::info("proxy", "системный прокси выключен, прежние настройки восстановлены");
    }
    Ok(())
}

// Crash safety: the pre-connection snapshot is mirrored to disk, so if the
// app is killed while connected, the next launch restores the user's settings.
fn snapshot_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("proxy-snapshot.json"))
}

fn persist_snapshot(app: &AppHandle, snap: Option<&proxy::Snapshot>) {
    let Some(path) = snapshot_path(app) else { return };
    match snap {
        Some(s) => {
            let _ = std::fs::create_dir_all(path.parent().unwrap());
            let _ = std::fs::write(&path, serde_json::to_vec(s).unwrap_or_default());
        }
        None => {
            let _ = std::fs::remove_file(&path);
        }
    }
}

fn recover_after_crash(app: &AppHandle) {
    let Some(path) = snapshot_path(app) else { return };
    if let Ok(bytes) = std::fs::read(&path) {
        if let Ok(snap) = serde_json::from_slice::<proxy::Snapshot>(&bytes) {
            let _ = proxy::restore(&snap);
            logs::warn("proxy", "после аварийного завершения восстановлены прежние настройки прокси");
        }
        let _ = std::fs::remove_file(&path);
    }
}

/// TCP handshake time to host:port, in milliseconds.
#[tauri::command]
async fn probe_latency(host: String, port: u16) -> Result<u32, String> {
    blocking(move || {
        let addr = (host.trim(), port).to_socket_addrs().map_err(|_| "Не удалось найти адрес".to_string())?.next().ok_or_else(|| "Адрес не найден".to_string())?;
        let started = Instant::now();
        TcpStream::connect_timeout(&addr, Duration::from_millis(3000)).map_err(|_| "Сервер не отвечает".to_string())?;
        Ok(started.elapsed().as_millis().min(u32::MAX as u128) as u32)
    })
    .await
}

/// Keeps the tray in sync with the system-proxy connection shown in the UI.
#[tauri::command]
fn set_tray_state(app: AppHandle, state: State<'_, AppState>, connected: bool, label: Option<String>) {
    *state.proxy_label.lock() = if connected { Some(label.unwrap_or_default()) } else { None };
    refresh_tray(&app);
}

/// Stops everything, restores proxy settings, then exits.
#[tauri::command]
async fn quit_app(app: AppHandle) {
    blocking(move || {
        shutdown(&app);
        app.exit(0);
    })
    .await
}

// ── lifecycle ─────────────────────────────────────────────────────────────

fn shutdown(app: &AppHandle) {
    let st = app.state::<AppState>();
    if st.shut_down.swap(true, Ordering::SeqCst) {
        return;
    }
    st.quitting.store(true, Ordering::SeqCst);
    st.tester.cancel();
    tgws::stop(&st.tgws);
    zapret::stop(&st.zapret);
    zapret::unload_driver();
    let snap = st.snapshot.lock().take();
    if let Some(snap) = snap {
        let _ = proxy::restore(&snap);
        persist_snapshot(app, None);
    }
    logs::info("app", "VELIORA App завершает работу");
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    refresh_tray(app);
}

fn toggle_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false) {
            let _ = w.hide();
            refresh_tray(app);
        } else {
            show_main(app);
        }
    }
}

fn state_word(s: &str) -> &'static str {
    match s {
        "running" => "включён",
        "starting" => "запускается",
        "stopping" => "останавливается",
        "error" => "ошибка",
        _ => "выключен",
    }
}

/// Updates tray labels, tooltip and icon from the current state.
pub(crate) fn refresh_tray(app: &AppHandle) {
    let st = app.state::<AppState>();
    let tg = st.tgws.status();
    let zp = st.zapret.status();
    let proxy = st.proxy_label.lock().clone();
    if let Some(items) = st.tray.lock().as_ref() {
        let visible = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false);
        let _ = items.show.set_text(if visible { "Скрыть окно" } else { "Открыть VELIORA App" });
        let busy = |s: &str| matches!(s, "starting" | "stopping");
        let _ = items.tgws.set_text(format!("Telegram — {}: {}", state_word(tg.state), if tg.state == "running" { "остановить" } else { "запустить" }));
        let _ = items.tgws.set_enabled(!busy(tg.state));
        let _ = items.zapret.set_text(format!("Zapret — {}: {}", state_word(zp.state), if zp.state == "running" { "остановить" } else { "запустить" }));
        let _ = items.zapret.set_enabled(!busy(zp.state) && !st.tester.is_running());
        let _ = items.proxy.set_text(if proxy.is_some() { "Системный прокси: отключить" } else { "Системный прокси: подключить" });
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let mut tip = format!("{APP_NAME}\nTelegram: {}\nZapret: {}", state_word(tg.state), state_word(zp.state));
        if let Some(l) = &proxy {
            tip.push_str(&format!("\nПрокси: {}", if l.is_empty() { "подключён" } else { l }));
        }
        let _ = tray.set_tooltip(Some(tip));
        let on = tg.state == "running" || zp.state == "running" || proxy.is_some();
        let bytes: &[u8] = if on { include_bytes!("../icons/tray-icon@2x.png") } else { include_bytes!("../icons/tray-icon-off@2x.png") };
        if let Ok(img) = Image::from_bytes(bytes) {
            let _ = tray.set_icon(Some(img));
        }
    }
}

/// Drops the verbatim `\\?\` prefix Windows APIs sometimes return: child
/// programs (cygwin's winws) and path comparisons expect a plain path.
fn plain_path(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy().to_string();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p
    }
}

fn spawn_toggle(app: &AppHandle, which: &'static str) {
    let app = app.clone();
    std::thread::spawn(move || {
        let st = app.state::<AppState>();
        let cfg = st.cfg();
        let r = match which {
            "tgws" if st.tgws.alive() => {
                tgws::stop(&st.tgws);
                Ok(())
            }
            "tgws" => tgws::start(&st.tgws, &cfg.tgws, &tgws::binary(&st.resources())),
            _ if st.zapret.alive() => {
                zapret::stop(&st.zapret);
                Ok(())
            }
            _ if cfg.zapret.strategy.is_none() => {
                show_main(&app);
                let _ = app.emit("nav://go", "zapret");
                Err("Выберите стратегию Zapret".to_string())
            }
            _ => zapret::start(&st.zapret, &st.resources(), &cfg.zapret),
        };
        if let Err(e) = r {
            logs::warn("app", format!("из трея: {e}"));
        }
    });
}

fn build_tray(app: &tauri::App, visible: bool) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Открыть VELIORA App", true, None::<&str>)?;
    let tg = MenuItem::with_id(app, "tgws", "Telegram — выключен: запустить", true, None::<&str>)?;
    let zp = MenuItem::with_id(app, "zapret", "Zapret — выключен: запустить", true, None::<&str>)?;
    let px = MenuItem::with_id(app, "proxy", "Системный прокси: подключить", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Выйти из VELIORA App", true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);
    let menu = Menu::with_items(app, &[&show, &sep()?, &tg, &zp, &px, &sep()?, &quit])?;
    *app.state::<AppState>().tray.lock() = Some(TrayItems { show, tgws: tg, zapret: zp, proxy: px });

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../icons/tray-icon-off@2x.png"))?)
        .tooltip(APP_NAME)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => toggle_main(app),
            "tgws" => spawn_toggle(app, "tgws"),
            "zapret" => spawn_toggle(app, "zapret"),
            "proxy" => {
                let _ = app.emit("tray://toggle", ());
            }
            "quit" => {
                let app = app.clone();
                std::thread::spawn(move || {
                    shutdown(&app);
                    app.exit(0);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    tray.set_visible(visible)?;
    Ok(())
}

fn autostart_components(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let st = app.state::<AppState>();
        let res = st.resources();
        if win::is_admin() {
            if let Err(e) = zapret::prepare(&res) {
                logs::error("zapret", format!("подготовка файлов: {e}"));
            }
        }
        let cfg = st.cfg();
        if cfg.tgws.auto_start {
            logs::info("app", "автозапуск Telegram-прокси");
            let _ = tgws::start(&st.tgws, &cfg.tgws, &tgws::binary(&res));
        }
        if cfg.zapret.auto_start {
            if cfg.zapret.strategy.is_some() {
                logs::info("app", "автозапуск Zapret");
                let _ = zapret::start(&st.zapret, &res, &cfg.zapret);
            } else {
                logs::warn("app", "автозапуск Zapret пропущен: стратегия не выбрана");
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(pid) = args.iter().find_map(|a| a.strip_prefix("--wait-parent=")).and_then(|p| p.parse::<u32>().ok()) {
        win::wait_pid(pid, 15_000);
    }
    LAUNCHED_BY_AUTOSTART.store(args.iter().any(|a| a == "--autostart"), Ordering::Relaxed);

    // Zapret needs administrator rights: in release builds VELIORA asks for them
    // once at start. If the user declines, it keeps running without Zapret.
    if !cfg!(debug_assertions) && !win::is_admin() && !args.iter().any(|a| a == "--no-elevate") {
        let mut pass: Vec<String> = args.iter().filter(|a| !a.starts_with("--wait-parent=")).cloned().collect();
        pass.push("--no-elevate".into());
        match win::relaunch_elevated(&pass) {
            Ok(()) => std::process::exit(0),
            Err(code) => {
                ELEVATION_DECLINED.store(true, Ordering::Relaxed);
                ELEVATION_ERROR.store(code, Ordering::Relaxed);
            }
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            app_info,
            app_elevate,
            app_restart,
            open_url,
            config_get,
            config_set,
            config_factory_reset,
            logs_history,
            logs_clear,
            logs_export,
            log_ui,
            core_status,
            tgws_start,
            tgws_stop,
            tgws_restart,
            tgws_link,
            tgws_regenerate_secret,
            zapret_start,
            zapret_stop,
            zapret_restart,
            zapret_overview,
            lists_info,
            list_read,
            list_write,
            list_import,
            list_export,
            list_restore_backup,
            lists_factory_reset,
            discord_clear_cache,
            tester_run,
            tester_cancel,
            tester_state,
            tester_reset,
            updates_zapret_releases,
            updates_zapret_install,
            updates_check,
            diag_run,
            diag_open,
            proxy_status,
            proxy_connect,
            proxy_disconnect,
            probe_latency,
            set_tray_state,
            quit_app
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let st = app.state::<AppState>();
            let data = app.path().app_local_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("VELIORA"));
            let _ = std::fs::create_dir_all(&data);
            logs::init(&handle, data.join("logs"));
            let store = config::Store::new(&data);
            let cfg = store.load();
            *st.config.lock() = cfg.clone();
            let _ = st.store.set(store);
            let _ = st.resources.set(plain_path(app.path().resource_dir().unwrap_or_default()));
            st.tester.init(&data);
            let _ = st.data_dir.set(data);
            st.tgws.attach(&handle);
            st.zapret.attach(&handle);
            logs::info(
                "app",
                format!("{APP_NAME} {} запущена{}", app.package_info().version, if win::is_admin() { " с правами администратора" } else { " без прав администратора" }),
            );
            if ELEVATION_DECLINED.load(Ordering::Relaxed) {
                logs::warn("app", "запрос прав администратора отклонён — Zapret недоступен, Telegram-прокси работает");
            }
            recover_after_crash(&handle);
            build_tray(app, cfg.app.tray)?;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_skip_taskbar(cfg.app.tray && cfg.app.hide_taskbar);
                let hidden = cfg.app.tray && cfg.app.silent_start;
                if !hidden {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            autostart_components(&handle);
            refresh_tray(&handle);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let st = app.state::<AppState>();
                if st.quitting.load(Ordering::SeqCst) {
                    return;
                }
                let prefs = st.cfg().app;
                if prefs.tray && prefs.close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                    refresh_tray(app);
                } else if prefs.tray {
                    api.prevent_close();
                    let _ = window.minimize();
                } else {
                    shutdown(app);
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build VELIORA App")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                shutdown(app);
            }
        });
}
