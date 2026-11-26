mod db;
mod feed;
pub(crate) use feed::public_ip;
mod probe;
mod relay;
pub(crate) mod vault;

use crate::component::CoreStatus;
use db::{DbResult, R};
use parking_lot::Mutex;
use rusqlite::Connection;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, State};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn retry_delay(base: u64, retry_after: u64, failures: u32) -> u64 {
    base.max(60)
        .saturating_mul(1 << failures.saturating_sub(1).min(6))
        .min(3600)
        .max(retry_after)
}

fn log_source(report: &db::SourceReport) {
    let status = report
        .http_status
        .map(|status| format!("HTTP {status}"))
        .unwrap_or_else(|| "HTTP-статус не получен".into());
    let route = if report.direct {
        "напрямую"
    } else {
        "настройки системы"
    };
    let msg = format!(
        "Каталог · {} · {} · {} мс · ссылок {} · валидных {} · маршрут: {}",
        report.name, status, report.duration_ms, report.links, report.valid, route
    );
    if let Some(reason) = &report.reason {
        crate::logs::warn(
            "tgws",
            format!(
                "{msg} · {reason} Попыток с ошибкой подряд: {}. Пауза {} с.",
                report.failures,
                report.next_at.saturating_sub(now())
            ),
        );
    } else {
        crate::logs::info("tgws", msg);
    }
}

#[derive(Default)]
struct Activity {
    busy: String,
    feed_error: String,
    test_error: String,
    refresh: bool,
    remaining: u8,
    last_refresh: i64,
    last_scan: i64,
    diagnostic: Option<(i64, tokio::sync::oneshot::Sender<R<probe::Sample>>)>,
    handoff: Option<(i64, tokio::sync::oneshot::Sender<R<db::Target>>)>,
}
struct ActiveProxy {
    relay: relay::Relay,
    target: db::Target,
    started_at: u64,
}
pub struct Catalog {
    active: tokio::sync::Mutex<Option<ActiveProxy>>,
    status: Mutex<CoreStatus>,
    db: Mutex<R<Connection>>,
    run: Mutex<Activity>,
    stop: AtomicBool,
}
impl Catalog {
    fn with<T>(&self, f: impl FnOnce(&mut Connection) -> R<T>) -> R<T> {
        let mut locked = self.db.lock();
        f(locked.as_mut().map_err(|e| e.clone())?)
    }
    pub fn start(data: &Path, resources: &Path) -> Arc<Self> {
        let catalog = Arc::new(Self {
            db: Mutex::new(
                db::init(&data.join("telegram-catalog.sqlite3")).and_then(|db| {
                    db::new_verification(&db)?;
                    Ok(db)
                }),
            ),
            run: Default::default(),
            active: Default::default(),
            status: Default::default(),
            stop: AtomicBool::new(false),
        });
        let worker = Arc::clone(&catalog);
        let dll = resources.join("tdlib/tdjson.dll");
        thread::spawn(move || {
            if let Err(e) = worker.work(dll) {
                crate::logs::error("tgws", format!("Фоновая задача каталога остановлена: {e}"));
                let mut run = worker.run.lock();
                run.feed_error = format!("Каталог остановлен: {e} Перезапустите приложение и проверьте журнал.");
                run.test_error = e;
                run.busy.clear();
                run.remaining = 0;
            }
        });
        catalog
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
    fn probe(&self, tester: &mut probe::Tester, id: i64, through_local: bool) -> R<()> {
        let ep = self.with(|c| db::endpoint(c, id))?;
        self.run.lock().busy = format!("Проверяется {}", ep.host);
        let sample = tester.check(&ep, through_local);
        self.with(|c| db::record(c, id, &sample))
    }
    fn update_catalog(
        &self,
        http: &reqwest::blocking::Client,
        cursor: Option<(&str, i64)>,
        history: bool,
        sources: &[feed::Source],
        direct: bool,
        check_all: bool,
    ) -> R<(bool, Option<i64>)> {
        let mut attempted = false;
        let mut loaded = false;
        let mut next_retry: Option<i64> = None;
        for &source in sources {
            if self.stop.load(Ordering::Relaxed) {
                break;
            }
            let (allowed, previous) = self.with(|c| {
                Ok((
                    db::pref(c, source.backoff, 0)?,
                    db::source_report(c, source.backoff)?,
                ))
            })?;
            if allowed > now() {
                if !check_all
                    && source.telegram
                    && previous.as_ref().is_some_and(|last| last.failures == 0)
                {
                    return Ok((true, Some(allowed)));
                }
                next_retry = Some(next_retry.map_or(allowed, |old| old.min(allowed)));
                continue;
            }
            attempted = true;
            let start = std::time::Instant::now();
            let mut report = db::SourceReport {
                name: source.name.into(),
                attempted_at: now(),
                next_at: 0,
                direct,
                duration_ms: 0,
                http_status: None,
                links: 0,
                valid: 0,
                failures: 0,
                reason: None,
            };
            let result = feed::fetch(http, source, if source.telegram { cursor } else { None });
            report.duration_ms = start.elapsed().as_millis() as u64;
            match result {
                Ok(page) => {
                    report.http_status = Some(200);
                    report.links = page.links;
                    report.valid = page.valid();
                    // The archive and live view share a request budget for Telegram.
                    report.next_at = now() + if source.telegram { 60 } else { 300 };
                    self.with(|c| {
                        db::ingest(c, &page, history, source.telegram)?;
                        db::store_source_report(c, source.backoff, &report)
                    })?;
                    log_source(&report);
                    loaded = true;
                    if !check_all {
                        break;
                    }
                }
                Err(error) => {
                    report.failures = previous.map_or(1, |last| last.failures.saturating_add(1));
                    let pause = retry_delay(error.delay, error.retry_after, report.failures);
                    report.next_at = now().saturating_add(i64::try_from(pause).unwrap_or(i64::MAX));
                    report.http_status = error.status;
                    report.links = error.links;
                    report.reason = Some(error.message.into());
                    self.with(|c| db::store_source_report(c, source.backoff, &report))?;
                    log_source(&report);
                    next_retry =
                        Some(next_retry.map_or(report.next_at, |old| old.min(report.next_at)));
                }
            }
        }
        if !history && loaded {
            self.run.lock().feed_error.clear();
        }
        if !history && attempted && !loaded {
            let cached = self.with(|c| db::pref(c, "sync", 0))? > 0;
            self.run.lock().feed_error=if cached {
                "Свежий список пока не получен. Серверы сохранены. Подробности по каждому источнику указаны ниже."
            } else {
                "Список пока не получен. Подробности по каждому источнику указаны ниже."
            }.into();
        }
        Ok((loaded, next_retry))
    }

    fn work(&self, dll: PathBuf) -> R<()> {
        let mut tester = probe::Tester::load(&dll)
            .map_err(|e| {
                self.run.lock().test_error = e;
            })
            .ok();
        let mut next_latest = 0;
        let mut next_history = now() + 60;
        let mut next_test = 0;
        let mut turn = 0; // Rotate between recent posts, known servers and the oldest due check.
        let mut last_fetch = 0;
        let mut client_failures: u32 = 0;
        while !self.stop.load(Ordering::Relaxed) {
            let t = now();
            let (before, interval, backoff, direct) = self.with(|c| {
                Ok((
                    db::pref(c, "before", 0)?,
                    db::pref(c, "interval", 15)?,
                    db::pref(c, "backoff", 0)?,
                    db::pref(c, "feed_direct", 0)? != 0,
                ))
            })?;
            let request = self.run.lock().diagnostic.take();
            if let Some((id, reply)) = request {
                if !reply.is_closed() {
                    let sample = match tester.as_mut() {
                        Some(tester) => self.with(|db| db::endpoint(db, id))
                            .map(|endpoint| tester.check(&endpoint, false)),
                        None => Err("Проверка MTProto недоступна.".into()),
                    };
                    let _ = reply.send(sample);
                }
            }
            let manual = self.run.lock().refresh;
            let latest_due = t >= next_latest || manual;
            let archive_due = before > 0 && t >= next_history && t >= backoff;
            if (latest_due || archive_due) && t - last_fetch >= 15 {
                let history = !latest_due;
                // A fresh head snapshot refreshes sightings even when there are no new posts.
                let cursor = history.then_some(("before", before));
                {
                    let mut activity = self.run.lock();
                    if !history {
                        activity.refresh = false;
                    }
                    activity.busy = if history {
                        "Пополнение каталога"
                    } else {
                        "Обновление каталога"
                    }
                    .into();
                }
                last_fetch = t;
                let sources = if history {
                    &feed::SOURCES[..1]
                } else {
                    &feed::SOURCES[..]
                };
                // Re-read system proxy configuration for each cycle instead of pinning it at app startup.
                match feed::client(direct) {
                    Err(error) => {
                        client_failures = client_failures.saturating_add(1);
                        let pause = retry_delay(120, 0, client_failures) as i64;
                        next_latest = now() + pause;
                        next_history = next_latest;
                        crate::logs::warn("tgws", format!("Каталог: {error} Повтор через {pause} с."));
                        self.run.lock().feed_error = format!(
                            "{error} Сохранённые записи доступны. Следующая автоматическая попытка через {} мин.",
                            pause / 60
                        );
                    }
                    Ok(http) => {
                        client_failures = 0;
                        let (loaded, retry) =
                            self.update_catalog(&http, cursor, history, sources, direct, manual)?;
                        if history {
                            next_history = now() + 90;
                        } else {
                            next_latest = match (loaded, retry) {
                                (true, None) => now() + interval * 60,
                                (true, Some(at)) => (now() + interval * 60).min(at).max(now() + 60),
                                (false, at) => at.unwrap_or_else(|| now() + 120).max(now() + 15),
                            };
                        }
                    }
                }
                self.run.lock().busy.clear();
            }
            let manual_scan = self.run.lock().remaining > 0;
            if let Some(tester) = tester.as_mut() {
                let request = self.run.lock().handoff.take();
                if let Some((id, reply)) = request {
                    if !reply.is_closed() {
                        let result = (|| {
                            let start = std::time::Instant::now();
                            for candidate in self.with(|c| db::candidates(c, id))? {
                                if reply.is_closed()
                                    || self.stop.load(Ordering::Relaxed)
                                    || start.elapsed() > Duration::from_secs(75)
                                {
                                    break;
                                }
                                self.probe(tester, candidate, true)?;
                                if let Ok(target) = self.with(|c| db::verified(c, candidate)) {
                                    return Ok(target);
                                }
                                if id != 0 {
                                    break;
                                }
                                thread::sleep(Duration::from_millis(700));
                            }
                            Err("Доступный сервер пока не найден. Обновите каталог или выберите другой сервер.".into())
                        })();
                        let _ = reply.send(result);
                    }
                    self.run.lock().busy.clear();
                    next_test = now() + 2;
                } else if t >= next_test {
                    let id = self.with(|c| db::choose(c, manual_scan, turn))?;
                    if let Some(id) = id {
                        self.probe(tester, id, false)?;
                        turn = (turn + 1) % 3;
                        let mut r = self.run.lock();
                        r.busy.clear();
                        r.remaining = r.remaining.saturating_sub(1);
                    } else {
                        self.run.lock().remaining = 0;
                    }
                    next_test = now() + if manual_scan { 2 } else { 8 };
                }
            }
            thread::sleep(Duration::from_millis(750));
        }
        Ok(())
    }
}
impl Catalog {
    pub async fn diagnose_selected(&self) -> R<Option<probe::Sample>> {
        let id = self.active.lock().await.as_ref().map(|active| active.target.id);
        let id = match id {
            Some(id) if id > 0 => id,
            _ => self.with(|db| db::pref(db, "selected", 0))?,
        };
        if id == 0 {
            return Ok(None);
        }
        let (send, receive) = tokio::sync::oneshot::channel();
        {
            let mut run = self.run.lock();
            if !run.test_error.is_empty() || self.stop.load(Ordering::Relaxed) {
                return Err("Проверка MTProto сейчас недоступна.".into());
            }
            if run.diagnostic.as_ref().is_some_and(|(_, pending)| !pending.is_closed()) {
                return Err("Проверка сервера уже в очереди.".into());
            }
            run.diagnostic = Some((id, send));
        }
        tokio::time::timeout(Duration::from_secs(20), receive).await
            .map_err(|_| "Проверка не завершилась за 20 секунд.")?
            .map_err(|_| "Проверку отменили.")?
            .map(Some)
    }

    pub fn status(&self) -> CoreStatus {
        self.status.lock().clone()
    }
    fn publish(&self, app: &AppHandle, status: CoreStatus) {
        *self.status.lock() = status.clone();
        let _ = app.emit("telegram://power", status);
        crate::refresh_tray(app);
    }
    async fn activate(
        &self,
        active: &mut Option<ActiveProxy>,
        id: i64,
        open: impl FnOnce(String) -> R<()>,
    ) -> R<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut r = self.run.lock();
            if !r.test_error.is_empty() {
                return Err(r.test_error.clone());
            }
            r.handoff = Some((id, tx));
        }
        let target = tokio::time::timeout(Duration::from_secs(120), rx)
            .await
            .map_err(|_| "Подбор занял слишком много времени.")?
            .map_err(|_| "Подбор прерван.")??;
        if self.stop.load(Ordering::Relaxed) {
            return Err("Приложение завершает работу.".into());
        }
        let (last_id, last_port) = self.with(|c| {
            Ok((
                db::pref(c, "relay_profile", 0)?,
                db::pref(c, "relay_port", 0)?,
            ))
        })?;
        let preferred = u16::try_from(last_port).unwrap_or(0);
        let new = if active.is_none() {
            Some(
                relay::Relay::start(preferred, target.address)
                    .await
                    .map_err(|_| "Не удалось открыть локальный порт.")?,
            )
        } else {
            None
        };
        let port = active
            .as_ref()
            .map(|a| a.relay.port)
            .or_else(|| new.as_ref().map(|r| r.port))
            .unwrap();
        let same_key = target.id == last_id
            || (last_id > 0
                && self
                    .with(|c| db::endpoint(c, last_id))
                    .is_ok_and(|ep| ep.secret == target.secret));
        if !same_key || i64::from(port) != last_port {
            open(target.local_link(port))?;
        }
        let id = target.id;
        if let Some(existing) = active {
            existing.relay.switch(target.address).await?;
            existing.target = target;
        } else if let Some(relay) = new {
            *active = Some(ActiveProxy {
                relay,
                target,
                started_at: now() as u64 * 1000,
            });
        }
        self.with(|c| {
            let tx = c.transaction().db()?;
            for (key, value) in [
                ("relay_profile", id),
                ("relay_port", port.into()),
                ("selected", id),
                ("opened", id),
                ("switched", now()),
            ] {
                db::set(&tx, key, value)?;
            }
            tx.commit().db()
        })
    }
    pub async fn with_relay_stopped<T>(&self, app: &AppHandle, op: impl FnOnce() -> T) -> R<T> {
        let mut active = self.active.try_lock().map_err(|_| "Подождите окончания переключения Telegram.")?;
        if let Some(proxy) = active.take() {
            self.publish(app, CoreStatus { state: "stopping", ..self.status() });
            proxy.relay.close().await;
        }
        self.publish(app, CoreStatus::default());
        Ok(op())
    }

    pub async fn power(&self, app: &AppHandle, on: bool, id: i64) -> R<()> {
        let mut active = self
            .active
            .try_lock()
            .map_err(|_| "Переключение уже выполняется.")?;
        self.publish(
            app,
            CoreStatus {
                state: if on { "starting" } else { "stopping" },
                ..self.status()
            },
        );
        let result = if on {
            self.activate(&mut active, id, crate::open_url).await
        } else {
            if let Some(a) = active.take() {
                a.relay.close().await;
            }
            Ok(())
        };
        let status = if let Some(a) = active.as_ref() {
            CoreStatus {
                state: "running",
                started_at: Some(a.started_at),
                detail: Some(format!("{}:{}", a.target.host, a.target.address.port())),
                ..Default::default()
            }
        } else if let Err(e) = &result {
            CoreStatus {
                state: "error",
                last_error: Some(e.clone()),
                ..Default::default()
            }
        } else {
            CoreStatus::default()
        };
        self.publish(app, status);
        result
    }
}
#[tauri::command]
pub fn telegram_power_status(state: State<'_, Arc<Catalog>>) -> CoreStatus {
    state.status()
}
#[tauri::command]
pub async fn telegram_power(
    app: AppHandle,
    state: State<'_, Arc<Catalog>>,
    on: bool,
    id: i64,
) -> R<()> {
    state.power(&app, on, id).await
}
#[tauri::command]
pub async fn telegram_show_link(state: State<'_, Arc<Catalog>>) -> R<()> {
    let guard = state
        .active
        .try_lock()
        .map_err(|_| "Подождите окончания переключения.")?;
    let a = guard.as_ref().ok_or("Сначала включите прокси.")?;
    crate::open_url(a.target.local_link(a.relay.port))
}
#[tauri::command]
pub async fn catalog_view(
    state: State<'_, Arc<Catalog>>,
    search: String,
    filter: String,
    sort: String,
    offset: u32,
) -> R<db::View> {
    if search.len() > 512 {
        return Err("Поиск слишком длинный.".into());
    }
    let (busy, error) = {
        let r = state.run.lock();
        let busy = if r.remaining > 0 {
            format!("Подбор · осталось до {} проверок. {}", r.remaining, r.busy)
        } else {
            r.busy.clone()
        };
        (
            busy,
            [r.feed_error.as_str(), r.test_error.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
        )
    };
    let catalog = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        catalog.with(|c| db::view(c, &search, &filter, &sort, offset, busy, error))
    })
    .await
    .map_err(|_| "Не удалось прочитать каталог.")?
}
#[tauri::command]
pub fn catalog_history(state: State<'_, Arc<Catalog>>, id: i64) -> R<Vec<probe::Sample>> {
    state.with(|c| db::history(c, id))
}
#[tauri::command]
pub async fn catalog_action(state: State<'_, Arc<Catalog>>, action: String, value: i64) -> R<()> {
    match action.as_str() {
        "refresh" => {
            let mut r = state.run.lock();
            if now() - r.last_refresh < 30 {
                return Err("Повторное обновление доступно через 30 секунд.".into());
            }
            r.last_refresh = now();
            r.refresh = true;
        }
        "scan" => {
            let mut r = state.run.lock();
            if !r.test_error.is_empty() {
                return Err(r.test_error.clone());
            }
            if r.remaining > 0 {
                return Ok(());
            }
            if now() - r.last_scan < 30 {
                return Err("Повторный подбор доступен через 30 секунд.".into());
            }
            r.last_scan = now();
            r.remaining = 10;
        }
        "interval" if [5, 15, 30, 60, 120].contains(&value) => {
            state.with(|c| db::set(c, "interval", value))?;
            state.run.lock().refresh = true;
        }
        "route" if value == 0 || value == 1 => {
            state.with(|c| db::set(c, "feed_direct", value))?;
            state.run.lock().refresh = true;
        }
        "auto" if value == 0 || value == 1 => {
            state.with(|c| {
                let tx = c.transaction().db()?;
                db::set(&tx, "auto", value)?;
                db::recommend(&tx, now())?;
                tx.commit().db()
            })?;
        }
        "favorite" => {
            state.with(|c| {
                c.execute("UPDATE endpoint SET fav=1-fav WHERE id=?", [value])
                    .db()?;
                Ok(())
            })?;
        }
        _ => return Err("Неизвестная команда каталога.".into()),
    }
    Ok(())
}

#[cfg(test)]
mod live {
    use super::*;
    #[test]
    fn retry_delay_climbs_and_honors_provider_header() {
        assert_eq!(
            (
                retry_delay(120, 0, 1),
                retry_delay(120, 0, 2),
                retry_delay(120, 0, 3)
            ),
            (120, 240, 480)
        );
        assert_eq!(retry_delay(120, 0, 100), 3600);
        assert_eq!(retry_delay(120, 7200, 2), 7200);
    }

    #[test]
    fn catalog_uses_reserve_and_keeps_saved_endpoints_on_failure() {
        use std::io::{Read, Write};
        fn fixture(
            template: feed::Source,
            status: &str,
            body: &str,
        ) -> (feed::Source, thread::JoinHandle<()>) {
            let server = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let url =
                Box::leak(format!("http://{}", server.local_addr().unwrap()).into_boxed_str());
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let worker = thread::spawn(move || {
                let (mut socket, _) = server.accept().unwrap();
                let _ = socket.read(&mut [0; 4096]);
                socket.write_all(response.as_bytes()).unwrap();
            });
            (feed::Source { url, ..template }, worker)
        }
        let catalog = Catalog {
            db: Mutex::new(db::init(Path::new(":memory:"))),
            active: Default::default(),
            status: Default::default(),
            run: Default::default(),
            stop: AtomicBool::new(false),
        };
        let http = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(4))
            .build()
            .unwrap();
        let (primary, one) = fixture(feed::SOURCES[0], "403 Forbidden", "");
        let (backup, two) = fixture(
            feed::SOURCES[1],
            "200 OK",
            "tg://proxy?server=example.org&port=443&secret=00112233445566778899aabbccddeeff",
        );
        assert!(
            catalog
                .update_catalog(&http, None, false, &[primary, backup], false, false)
                .unwrap()
                .0
        );
        one.join().unwrap();
        two.join().unwrap();
        let confirmed = catalog.with(|c| db::pref(c, "sync", 0)).unwrap();
        assert!(confirmed > 0);
        assert!(
            !catalog
                .update_catalog(&http, None, false, &[primary, backup], false, false)
                .unwrap()
                .0
        );
        assert!(
            catalog.run.lock().feed_error.is_empty(),
            "cooldown after success is not a failure"
        );
        catalog
            .with(|c| {
                db::set(c, primary.backoff, 0)?;
                db::set(c, backup.backoff, 0)
            })
            .unwrap();
        let (primary, one) = fixture(primary, "429 Too Many Requests", "");
        let (backup, two) = fixture(backup, "503 Service Unavailable", "");
        assert!(
            !catalog
                .update_catalog(&http, None, false, &[primary, backup], false, false)
                .unwrap()
                .0
        );
        one.join().unwrap();
        two.join().unwrap();
        assert_eq!(
            catalog
                .with(|c| db::source_report(c, primary.backoff))
                .unwrap()
                .unwrap()
                .failures,
            2
        );
        let cached = catalog
            .with(|c| {
                c.execute("UPDATE endpoint SET seen=1", []).db()?;
                assert_eq!(db::pref(c, "sync", 0)?, confirmed);
                db::view(c, "", "all", "rank", 0, String::new(), String::new())
            })
            .unwrap();
        assert_eq!((cached.total, cached.available), (1, 0));
        assert!(cached.items[0].stale);
        assert!(catalog.run.lock().feed_error.contains("сохранены"));
    }

    #[test]
    fn button_selects_without_an_id_and_reuses_the_confirmed_profile() {
        tauri::async_runtime::block_on(async {
            let catalog = Catalog {
                db: Mutex::new(db::init(Path::new(":memory:"))),
                active: Default::default(),
                status: Default::default(),
                run: Default::default(),
                stop: AtomicBool::new(false),
            };
            let upstream = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                .await
                .unwrap();
            let target = upstream.local_addr().unwrap();
            let selection = async {
                loop {
                    let req = catalog.run.lock().handoff.take();
                    if let Some((id, reply)) = req {
                        assert_eq!(id, 0, "the power button must not require a chosen row");
                        let _ = reply.send(Ok(db::Target {
                            id: 1,
                            address: target,
                            host: "sample.example.org".into(),
                            secret: "01".repeat(16),
                        }));
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            };
            let mut active = None;
            let (result, ()) = tokio::join!(
                catalog.activate(&mut active, 0, |u| {
                    assert!(u.starts_with("tg://proxy?server=127.0.0.1&"));
                    Ok(())
                }),
                selection
            );
            result.unwrap();
            let port = active.as_ref().unwrap().relay.port;
            active.take().unwrap().relay.close().await;
            assert!(tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_err());
            let select_again = async {
                loop {
                    let req = catalog.run.lock().handoff.take();
                    if let Some((id, reply)) = req {
                        assert_eq!(id, 0);
                        let _ = reply.send(Ok(db::Target {
                            id: 1,
                            address: target,
                            host: "sample.example.org".into(),
                            secret: "01".repeat(16),
                        }));
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            };
            let (result, ()) = tokio::join!(
                catalog.activate(&mut active, 0, |_| panic!("already offered this profile")),
                select_again
            );
            result.unwrap();
            assert_eq!(active.as_ref().unwrap().relay.port, port);
            active.take().unwrap().relay.close().await;
        });
    }
    #[test]
    #[ignore = "starts an empty catalog, selects a real public server and checks it through the relay"]
    fn cold_start_button_to_telegram() {
        let catalog = Arc::new(Catalog {
            db: Mutex::new(db::init(Path::new(":memory:"))),
            active: Default::default(),
            status: Default::default(),
            run: Default::default(),
            stop: AtomicBool::new(false),
        });
        let dll = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../node_modules/@prebuilt-tdlib/win32-x64/tdjson.dll")
            .canonicalize()
            .unwrap();
        let worker = catalog.clone();
        let library = dll.clone();
        let thread = thread::spawn(move || worker.work(library));
        let mut active = None;
        let selection = tauri::async_runtime::block_on(catalog.activate(&mut active, 0, |url| {
            assert!(url.starts_with("tg://proxy?server=127.0.0.1&"));
            Ok(())
        }));
        catalog.stop();
        thread.join().unwrap().unwrap();
        selection.unwrap();
        let a = active.unwrap();
        let mut client = probe::Tester::load(&dll).unwrap();
        eprintln!("testing {} via port {}", a.target.host, a.relay.port);
        let response = client.handshake(([127, 0, 0, 1], a.relay.port).into(), &a.target.secret, 0);
        eprintln!(
            "Button from empty catalog: Telegram via relay = {}, elapsed {:?} ms",
            response.result, response.elapsed
        );
        let port = a.relay.port;
        tauri::async_runtime::block_on(a.relay.close());
        assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
        assert_eq!(response.result, "ok");
    }

    #[test]
    #[ignore = "uses the public channel and up to six advertised endpoints"]
    fn public_channel() {
        let http = feed::client(false).unwrap();
        let page = feed::fetch(&http, feed::SOURCES[0], None).unwrap();
        let dir = std::env::temp_dir().join(format!("veliora-catalog-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.sqlite3");
        let mut c = db::init(&path).unwrap();
        db::ingest(&mut c, &page, false, true).unwrap();
        let total = db::view(&c, "", "", "", 0, String::new(), String::new())
            .unwrap()
            .total;
        db::ingest(&mut c, &page, false, true).unwrap();
        assert!(total > 0);
        assert_eq!(
            total,
            db::view(&c, "", "", "", 0, String::new(), String::new())
                .unwrap()
                .total
        );
        let dll = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../node_modules/@prebuilt-tdlib/win32-x64/tdjson.dll")
            .canonicalize()
            .unwrap();
        let mut tester = probe::Tester::load(&dll).unwrap();
        let mut oks = 0;
        // Spread the live sample across the page so one batch of related posts does not dominate it.
        let ids = c
            .prepare("SELECT id FROM endpoint ORDER BY post,id")
            .unwrap()
            .query_map([], |r| r.get::<_, i64>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for id in ids.iter().step_by(ids.len().div_ceil(6)).take(6).copied() {
            let ep = db::endpoint(&c, id).unwrap();
            let sample = tester.check(&ep, false);
            eprintln!(
                "{} · {} · tcp {:?} ms · MTProto {:?} ms",
                ep.host, sample.result, sample.tcp, sample.elapsed
            );
            db::record(&mut c, id, &sample).unwrap();
            if sample.result == "ok" {
                oks += 1;
                let target = db::verified(&c, id).unwrap();
                let relay =
                    tauri::async_runtime::block_on(relay::Relay::start(0, target.address)).unwrap();
                assert!(target
                    .local_link(relay.port)
                    .starts_with("tg://proxy?server=127.0.0.1&"));
                let via_relay =
                    tester.handshake(([127, 0, 0, 1], relay.port).into(), &target.secret, 0);
                tauri::async_runtime::block_on(relay.close());
                assert_eq!(
                    via_relay.result, "ok",
                    "Telegram verification through the running local proxy"
                );
            }
            thread::sleep(Duration::from_secs(1));
        }
        drop(tester);
        drop(c);
        let c = db::init(&path).unwrap();
        let view = db::view(&c, "", "", "", 0, String::new(), String::new()).unwrap();
        eprintln!(
            "{} publications, {total} distinct endpoints, {oks} verified; reopened database",
            page.posts.len()
        );
        assert_eq!(view.total, total);
        assert_eq!(view.available, oks);
        assert_eq!(view.best.is_some(), oks > 0);
        drop(c);
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}
