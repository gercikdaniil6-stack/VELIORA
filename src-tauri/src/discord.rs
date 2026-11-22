use crate::{
    component::{CoreStatus, Health},
    config::{DiscordUdp, DiscordVerified},
    tester::{self, Probe},
    win, zapret, AppState,
};
use serde::Serialize;
use std::{
    net::{ToSocketAddrs, UdpSocket},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};

const PORTS: &str = "19294-19344,50000-50100";
const MEDIA_FILTER: &str = "--filter-udp=19294-19344,50000-50100";
// Match the 74-byte Discord IP discovery request, never the encrypted UDP stream.
const DISCOVERY_CAPTURE: &str = "--wf-raw-part=outbound and udp and udp.DstPort>=50101 and udp.PayloadLength=74 and udp.Payload32[0]=0x00010046 and udp.Payload32[2]=0 and udp.Payload32[17]=0";

pub fn apply_udp_profile(args: Vec<String>, mode: DiscordUdp) -> Result<Vec<String>, String> {
    if mode == DiscordUdp::Bundle {
        return Ok(args);
    }
    let mut out = Vec::with_capacity(args.len() + 8);
    let mut replaced = false;
    for profile in args.split(|arg| arg == "--new") {
        if !out.is_empty() {
            out.push("--new".into());
        }
        let media = profile.iter().any(|arg| arg == MEDIA_FILTER)
            && profile.iter().any(|arg| {
                arg.strip_prefix("--filter-l7=")
                    .is_some_and(|list| list.split(',').any(|protocol| protocol == "discord"))
            });
        if !media {
            out.extend_from_slice(profile);
            continue;
        }
        replaced = true;
        match mode {
            DiscordUdp::Bundle => unreachable!(),
            // The matched no-op also prevents the later generic game rule from modifying these ports.
            DiscordUdp::Direct => out.push(format!("--filter-udp={PORTS}")),
            DiscordUdp::Discovery => {
                let discovery: Vec<_> = profile
                    .iter()
                    .filter(|arg| {
                        ![
                            "--filter-udp=",
                            "--filter-l7=",
                            "--dpi-desync-any-protocol=",
                            "--dpi-desync-cutoff=",
                            "--dpi-desync-repeats=",
                            "--dpi-desync-fake-unknown-udp=",
                        ]
                        .iter()
                        .any(|prefix| arg.starts_with(prefix))
                    })
                    .cloned()
                    .collect();
                for (ports, protocols) in [(PORTS, "discord,stun"), ("50101-65535", "discord")] {
                    if ports != PORTS {
                        out.push("--new".into());
                    }
                    out.push(format!("--filter-udp={ports}"));
                    out.push(format!("--filter-l7={protocols}"));
                    out.extend(discovery.iter().cloned());
                    out.extend([
                        "--dpi-desync-cutoff=n3".into(),
                        "--dpi-desync-repeats=2".into(),
                    ]);
                }
            }
        }
    }
    if !replaced {
        return Err(
            "В выбранной стратегии нет совместимого правила Discord UDP. Выберите «Из стратегии»."
                .into(),
        );
    }
    if mode == DiscordUdp::Discovery {
        out.insert(0, DISCOVERY_CAPTURE.into());
    }
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UdpReport {
    sent: u8,
    received: u8,
    average_ms: Option<u64>,
    error: Option<&'static str>,
}

fn udp_test() -> UdpReport {
    let mut report = UdpReport {
        sent: 0,
        received: 0,
        average_ms: None,
        error: None,
    };
    let Ok(mut endpoints) = ("stun.cloudflare.com", 3478).to_socket_addrs() else {
        report.error = Some("DNS для тестового STUN недоступен");
        return report;
    };
    let Some(target) = endpoints.find(|addr| addr.is_ipv4()) else {
        report.error = Some("У тестового STUN нет адреса IPv4");
        return report;
    };
    let socket = UdpSocket::bind(("0.0.0.0", 0)).and_then(|s| {
        s.connect(target)?;
        s.set_read_timeout(Some(Duration::from_millis(900)))?;
        Ok(s)
    });
    let Ok(socket) = socket else {
        report.error = Some("Не удалось открыть UDP-сокет");
        return report;
    };
    let mut elapsed = 0;
    let mut buffer = [0; 2048];
    for _ in 0..3 {
        let mut request = [0u8; 20];
        request[1] = 1; // STUN Binding request (RFC 8489).
        request[4..8].copy_from_slice(&0x2112a442u32.to_be_bytes());
        if getrandom::getrandom(&mut request[8..]).is_err() {
            report.error = Some("Не удалось создать идентификатор STUN");
            break;
        }
        let start = Instant::now();
        if socket.send(&request).is_err() {
            report.error = Some("Ошибка отправки UDP");
            break;
        }
        report.sent += 1;
        if let Ok(n) = socket.recv(&mut buffer) {
            if n >= 20
                && buffer[..2] == [1, 1]
                && buffer[4..20] == request[4..20]
                && n >= 20 + u16::from_be_bytes([buffer[2], buffer[3]]) as usize
            {
                report.received += 1;
                elapsed += start.elapsed().as_millis() as u64;
            }
        }
    }
    if report.received > 0 {
        report.average_ms = Some(elapsed / u64::from(report.received));
    } else if report.error.is_none() {
        report.error = Some("Тестовый STUN не ответил по UDP");
    }
    report
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordDiagnostics {
    web: Vec<Probe>,
    udp: UdpReport,
    status: CoreStatus,
    capture_confirmed: bool,
    competing: usize,
    mtu: Vec<u32>,
    vpn: bool,
}

#[tauri::command]
pub async fn discord_diagnostics(app: AppHandle) -> DiscordDiagnostics {
    crate::blocking(move || {
        let state=app.state::<AppState>();
        let (web,udp)=std::thread::scope(|scope|{
            let web=scope.spawn(|| [
                ("Интерфейс","https://discord.com/api/v10/gateway"),
                ("Шлюз","https://gateway.discord.gg/"),
                ("Файлы","https://cdn.discordapp.com/"),
            ].iter().map(|(label,url)|tester::probe_one(label,url)).collect::<Vec<_>>());
            let udp=udp_test();
            (web.join().unwrap_or_default(),udp)
        });
        let status=state.zapret.status();
        let capture_confirmed=state.zapret.health()==Health::Ready;
        let competing=crate::zapret_conflicts::scan(status.pid, false).len();
        let mut mtu:Vec<_>=win::adapters().into_iter().filter(|adapter|adapter.up && adapter.if_type!=24).map(|a|a.mtu).collect();
        mtu.sort_unstable();mtu.dedup();
        let vpn=tester::detect_vpn().is_some();
        let web_ok=web.iter().filter(|probe|probe.ok).count();
        crate::logs::info("zapret",format!("Discord диагностика · HTTPS {web_ok}/{} · STUN UDP {}/{} · причина UDP: {} · Zapret {} · перехват подтверждён: {} · сторонних winws/winws2: {competing} · VPN: {vpn} · MTU {:?}",web.len(),udp.received,udp.sent,udp.error.unwrap_or("—"),status.state,capture_confirmed,mtu));
        DiscordDiagnostics{web,udp,status,capture_confirmed,competing,mtu,vpn}
    }).await
}

#[tauri::command]
pub async fn discord_confirm_media(
    app: AppHandle,
    started_at: u64,
) -> Result<DiscordVerified, String> {
    crate::blocking(move || {
        let state = app.state::<AppState>();
        let _operation = state
            .zapret
            .op
            .try_lock()
            .ok_or("Дождитесь переключения Zapret.")?;
        let status = state.zapret.status();
        if state.tester.is_running()
            || status.state != "running"
            || status.started_at != Some(started_at)
        {
            return Err("Стратегия изменилась. Повторите ручную проверку.".into());
        }
        let mut cfg = state.cfg();
        let strategy = cfg
            .zapret
            .strategy
            .clone()
            .ok_or("Запустите выбранную стратегию Zapret.")?;
        let verified = DiscordVerified {
            strategy,
            udp: cfg.zapret.discord_udp,
            version: zapret::installed_version(&state.resources()),
            at: win::now_ms(),
        };
        cfg.zapret.discord_verified = Some(verified.clone());
        crate::apply_config(&app, cfg)?;
        crate::logs::info(
            "zapret",
            format!(
                "Discord: пользователь подтвердил голос и демонстрацию · {} · {:?}",
                verified.strategy, verified.udp
            ),
        );
        Ok(verified)
    })
    .await
}
