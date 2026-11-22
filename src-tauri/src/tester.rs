//! Strategy tester: measures which services are blocked on this network without
//! any bypass, then runs every strategy and checks what it opens, what it
//! breaks and how fast it is; the leaders are re-measured before ranking.
use crate::component::Component;
use crate::config::ZapretConfig;
use crate::logs;
use crate::{win, zapret};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const TARGETS: [(&str, &str); 13] = [
    ("YouTube", "https://www.youtube.com"),
    ("YouTube", "https://i.ytimg.com"),
    ("YouTube", "https://redirector.googlevideo.com"),
    ("YouTube", "https://youtu.be"),
    ("Discord", "https://discord.com"),
    ("Discord", "https://gateway.discord.gg"),
    ("Discord", "https://cdn.discordapp.com"),
    ("Discord", "https://updates.discord.com"),
    ("Google", "https://www.google.com"),
    ("Google", "https://www.gstatic.com"),
    ("Cloudflare", "https://www.cloudflare.com"),
    ("Cloudflare", "https://cdnjs.cloudflare.com"),
    ("Telegram", "https://web.telegram.org"),
];
const WARMUP: Duration = Duration::from_millis(2500);
const LEADERS_RECHECK: usize = 3;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    pub service: String,
    pub url: String,
    pub ok: bool,
    pub ms: u32,
    /// "", "dns", "cert", "timeout", "reset"
    pub fault: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct StrategyResult {
    /// passed | failed | not_started
    pub status: String,
    pub opened: usize,
    pub blocked_total: usize,
    pub broken: usize,
    pub latency_ms: Option<u32>,
    pub services: BTreeMap<String, (usize, usize)>,
    pub score: f64,
    pub rechecked: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub ran_at: u64,
    pub duration_ms: u64,
    pub bundle_version: String,
    pub cancelled: bool,
    pub vpn: Option<String>,
    pub system_proxy: bool,
    pub baseline: Vec<Probe>,
    /// Services blocked without bypass (DPI-like failures)
    pub blocked: Vec<String>,
    /// Services no strategy managed to open
    pub unfixable: Vec<String>,
    /// Services that failed on DNS or a substituted certificate
    pub dns_issues: Vec<String>,
    pub results: BTreeMap<String, StrategyResult>,
    pub best: Vec<String>,
    pub tested: usize,
    pub total: usize,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// baseline | testing | recheck | done | cancelled | error
    pub phase: &'static str,
    pub current: usize,
    pub total: usize,
    pub strategy: Option<String>,
    pub message: Option<String>,
}

pub struct Tester {
    running: AtomicBool,
    cancel: AtomicBool,
    report: Mutex<Option<Report>>,
    path: Mutex<Option<PathBuf>>,
}

impl Default for Tester {
    fn default() -> Self {
        Self { running: AtomicBool::new(false), cancel: AtomicBool::new(false), report: Mutex::new(None), path: Mutex::new(None) }
    }
}

impl Tester {
    pub fn init(&self, data_dir: &Path) {
        let p = data_dir.join("zapret-test-report.json");
        if let Some(r) = std::fs::read(&p).ok().and_then(|b| serde_json::from_slice::<Report>(&b).ok()) {
            *self.report.lock() = Some(r);
        }
        *self.path.lock() = Some(p);
    }
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
    pub fn report(&self) -> Option<Report> {
        self.report.lock().clone()
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
    pub fn reset(&self) {
        *self.report.lock() = None;
        if let Some(p) = self.path.lock().as_ref() {
            let _ = std::fs::remove_file(p);
        }
    }
    fn save(&self, r: &Report) {
        *self.report.lock() = Some(r.clone());
        if let Some(p) = self.path.lock().as_ref() {
            let _ = std::fs::write(p, serde_json::to_vec_pretty(r).unwrap_or_default());
        }
    }
}

fn classify_error(e: &reqwest::Error) -> &'static str {
    let s = format!("{e:?}").to_ascii_lowercase();
    if e.is_timeout() {
        "timeout"
    } else if s.contains("dns") || s.contains("no such host") || s.contains("11001") || s.contains("name or service") {
        "dns"
    } else if s.contains("certificate") || s.contains("cert") || s.contains("0x800b") {
        "cert"
    } else {
        "reset"
    }
}

/// One HTTPS request on a fresh connection (no pooling, no system proxy):
/// any HTTP answer means the TLS handshake got through.
pub(crate) fn probe_one(service: &str, url: &str) -> Probe {
    let t0 = Instant::now();
    let r = reqwest::blocking::Client::builder()
        .no_proxy()
        .pool_max_idle_per_host(0)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(7))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) VELIORA-Test")
        .build()
        .and_then(|c| c.head(url).send());
    let ms = t0.elapsed().as_millis().min(u32::MAX as u128) as u32;
    let (ok, fault) = match r {
        Ok(_) => (true, String::new()),
        Err(e) => (false, classify_error(&e).to_string()),
    };
    Probe { service: service.into(), url: url.into(), ok, ms, fault }
}

fn probe_targets(attempts: usize) -> Vec<Probe> {
    std::thread::scope(|s| {
        let handles: Vec<_> = TARGETS
            .iter()
            .map(|(service, url)| {
                s.spawn(move || {
                    let mut p = probe_one(service, url);
                    for _ in 1..attempts {
                        if p.ok {
                            break;
                        }
                        p = probe_one(service, url);
                    }
                    p
                })
            })
            .collect();
        handles
            .into_iter()
            .zip(TARGETS.iter())
            .map(|(h, (svc, url))| h.join().unwrap_or_else(|_| Probe { service: svc.to_string(), url: url.to_string(), ok: false, ms: 0, fault: "reset".into() }))
            .collect()
    })
}

fn probe_all() -> Vec<Probe> {
    probe_targets(1)
}

/// Baseline: two attempts per target so a one-off hiccup isn't counted as a block.
fn probe_baseline() -> Vec<Probe> {
    probe_targets(2)
}

fn evaluate(baseline: &[Probe], now: &[Probe]) -> StrategyResult {
    let mut r = StrategyResult::default();
    let mut lat = Vec::new();
    for (b, n) in baseline.iter().zip(now) {
        let e = r.services.entry(n.service.clone()).or_insert((0, 0));
        e.1 += 1;
        if n.ok {
            e.0 += 1;
            lat.push(n.ms);
        }
        let dpi_blocked = !b.ok && b.fault != "dns" && b.fault != "cert";
        if dpi_blocked {
            r.blocked_total += 1;
            if n.ok {
                r.opened += 1;
            }
        } else if b.ok && !n.ok {
            r.broken += 1;
        }
    }
    r.latency_ms = (!lat.is_empty()).then(|| (lat.iter().map(|&x| x as u64).sum::<u64>() / lat.len() as u64) as u32);
    let ratio = if r.blocked_total == 0 { 1.0 } else { r.opened as f64 / r.blocked_total as f64 };
    r.passed_flag(ratio);
    r.score = ratio - 0.3 * r.broken as f64;
    r
}

impl StrategyResult {
    fn passed_flag(&mut self, ratio: f64) {
        self.status = if self.broken == 0 && ratio >= 0.5 { "passed".into() } else { "failed".into() };
    }
}

fn better(a: &StrategyResult, b: &StrategyResult) -> std::cmp::Ordering {
    // higher score first, then lower latency
    b.score
        .partial_cmp(&a.score)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then(a.latency_ms.unwrap_or(u32::MAX).cmp(&b.latency_ms.unwrap_or(u32::MAX)))
}

pub fn detect_vpn() -> Option<String> {
    let re = ["wireguard", "wintun", "tap-windows", "openvpn", "vpn", "tun", "outline", "amnezia", "happ", "hiddify", "nekoray", "clash", "v2ray", "sing-box", "xray", "zerotier", "tailscale", "radmin"];
    win::adapters()
        .into_iter()
        .filter(|a| a.up && a.if_type != 24)
        .find(|a| {
            let n = format!("{} {}", a.name, a.description).to_ascii_lowercase();
            re.iter().any(|k| n.contains(k)) && !n.contains("radmin") // Radmin VPN is a LAN tool; it does not tunnel internet traffic
        })
        .map(|a| if a.description.is_empty() { a.name } else { format!("{} ({})", a.name, a.description) })
}

pub fn run(app: AppHandle, tester: Arc<Tester>, zap: Arc<Component>, resources: PathBuf, cfg: ZapretConfig) -> Result<(), String> {
    if tester.running.swap(true, Ordering::SeqCst) {
        return Err("Тестирование уже идёт.".into());
    }
    tester.cancel.store(false, Ordering::SeqCst);
    let emit = {
        let app = app.clone();
        move |p: Progress| {
            let _ = app.emit("tester://progress", &p);
        }
    };
    let finish = |tester: &Tester| tester.running.store(false, Ordering::SeqCst);

    if !win::is_admin() {
        finish(&tester);
        return Err("Тест стратегий требует прав администратора.".into());
    }
    if let Err(e) = zapret::prepare(&resources) {
        finish(&tester);
        return Err(e);
    }
    let dir = zapret::runtime();
    if let Err(e) = zapret::preflight(zap.status().pid) {
        finish(&tester);
        return Err(e);
    }
    let list = zapret::strategies(&dir);
    if list.is_empty() {
        finish(&tester);
        return Err("В сборке Zapret нет стратегий.".into());
    }

    std::thread::spawn(move || {
        let started = Instant::now();
        let was_running = zap.alive();
        if was_running {
            zapret::stop(&zap);
        }
        let total = list.len();
        logs::info("test", format!("тест стратегий: {total} шт."));
        emit(Progress { phase: "baseline", current: 0, total, strategy: None, message: None });
        let baseline = probe_baseline();
        let mut report = Report {
            ran_at: win::now_ms(),
            bundle_version: zapret::installed_version(&resources),
            vpn: detect_vpn(),
            system_proxy: crate::proxy::read().map(|p| p.enabled).unwrap_or(false),
            total,
            ..Default::default()
        };
        let mut blocked: Vec<String> = Vec::new();
        let mut dns: Vec<String> = Vec::new();
        for p in &baseline {
            if !p.ok {
                let list = if p.fault == "dns" || p.fault == "cert" { &mut dns } else { &mut blocked };
                if !list.contains(&p.service) {
                    list.push(p.service.clone());
                }
            }
        }
        report.blocked = blocked;
        report.dns_issues = dns;
        report.baseline = baseline.clone();
        logs::info("test", format!("без обхода не открываются: {}", if report.blocked.is_empty() { "ничего".to_string() } else { report.blocked.join(", ") }));

        let mut cancelled = false;
        for (i, s) in list.iter().enumerate() {
            if tester.cancel.load(Ordering::SeqCst) {
                cancelled = true;
                break;
            }
            emit(Progress { phase: "testing", current: i + 1, total, strategy: Some(s.name.clone()), message: None });
            let result = test_one(&zap, &dir, &s.file, &cfg, &baseline);
            logs::info("test", format!("{}: {} (открыто {}/{}, сломано {}, {} мс)", s.name, result.status, result.opened, result.blocked_total, result.broken, result.latency_ms.unwrap_or(0)));
            report.results.insert(s.file.clone(), result);
            report.tested = i + 1;
            report.duration_ms = started.elapsed().as_millis() as u64;
            tester.save(&report);
        }

        if !cancelled {
            let mut leaders: Vec<(String, StrategyResult)> = report.results.iter().filter(|(_, r)| r.status == "passed").map(|(k, v)| (k.clone(), v.clone())).collect();
            leaders.sort_by(|a, b| better(&a.1, &b.1));
            leaders.truncate(LEADERS_RECHECK);
            for (n, (file, first)) in leaders.iter().enumerate() {
                if tester.cancel.load(Ordering::SeqCst) {
                    cancelled = true;
                    break;
                }
                emit(Progress { phase: "recheck", current: n + 1, total: leaders.len(), strategy: Some(file.trim_end_matches(".bat").to_string()), message: None });
                let again = test_one(&zap, &dir, file, &cfg, &baseline);
                let merged = StrategyResult {
                    status: if again.status == "passed" && first.status == "passed" { "passed".into() } else { "failed".into() },
                    opened: first.opened.min(again.opened),
                    blocked_total: first.blocked_total,
                    broken: first.broken.max(again.broken),
                    latency_ms: match (first.latency_ms, again.latency_ms) {
                        (Some(a), Some(b)) => Some((a + b) / 2),
                        (a, b) => a.or(b),
                    },
                    services: again.services.clone(),
                    score: (first.score + again.score) / 2.0,
                    rechecked: true,
                };
                report.results.insert(file.clone(), merged);
            }
        }

        // Rank: best = top score; equals = same score and latency within 15 %.
        let mut passed: Vec<(&String, &StrategyResult)> = report.results.iter().filter(|(_, r)| r.status == "passed").collect();
        passed.sort_by(|a, b| better(a.1, b.1));
        if let Some((_, top)) = passed.first() {
            let top_lat = top.latency_ms.unwrap_or(0) as f64;
            report.best = passed
                .iter()
                .filter(|(_, r)| (r.score - top.score).abs() < 1e-9 && (r.latency_ms.unwrap_or(0) as f64) <= top_lat * 1.15 + 5.0)
                .map(|(k, _)| (*k).clone())
                .collect();
        }
        report.unfixable = report
            .blocked
            .iter()
            .filter(|svc| !report.results.values().any(|r| r.services.get(*svc).is_some_and(|(ok, _)| *ok > 0)))
            .cloned()
            .collect();
        report.cancelled = cancelled;
        report.duration_ms = started.elapsed().as_millis() as u64;
        tester.save(&report);

        if was_running {
            if let Err(e) = zapret::start(&zap, &resources, &cfg) {
                logs::warn("test", format!("не удалось вернуть Zapret после теста: {e}"));
            }
        }
        logs::info("test", format!("тест {} за {} с", if cancelled { "отменён" } else { "завершён" }, report.duration_ms / 1000));
        emit(Progress { phase: if cancelled { "cancelled" } else { "done" }, current: report.tested, total, strategy: None, message: None });
        finish(&tester);
    });
    Ok(())
}

fn test_one(zap: &Arc<Component>, dir: &Path, file: &str, cfg: &ZapretConfig, baseline: &[Probe]) -> StrategyResult {
    let _guard = zap.op.lock();
    let launched = zapret::launch(zap, dir, file, cfg, true);
    let ready = launched.is_ok() && zapret::wait_ready(zap, Duration::from_millis(2000)).is_ok() && zap.alive();
    let result = if ready {
        std::thread::sleep(WARMUP);
        evaluate(baseline, &probe_all())
    } else {
        StrategyResult { status: "not_started".into(), blocked_total: baseline.iter().filter(|b| !b.ok).count(), ..Default::default() }
    };
    zap.kill();
    zap.mark_stopped();
    std::thread::sleep(Duration::from_millis(350));
    result
}
