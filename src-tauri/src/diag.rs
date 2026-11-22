//! "Why might it not work?" — checks the system for the usual reasons the
//! Telegram proxy or Zapret fail, each with a plain explanation and steps.
use crate::config::TgwsConfig;
use crate::{tester, win, zapret};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    /// elevate | services | security | taskmgr | proxy-settings | apps | network
    pub kind: &'static str,
    pub label: &'static str,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: &'static str,
    /// error | warn
    pub severity: &'static str,
    pub title: String,
    pub impact: String,
    pub detail: String,
    pub steps: Vec<String>,
    pub commands: Vec<String>,
    pub actions: Vec<Action>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagReport {
    pub issues: Vec<Issue>,
    pub passed: Vec<String>,
    pub admin: bool,
}

pub struct Ctx<'a> {
    pub resources: &'a Path,
    pub tgws: &'a TgwsConfig,
    pub tgws_running: bool,
    pub own_proxy_active: bool,
    pub zapret_pid: Option<u32>,
}

pub fn run(ctx: &Ctx) -> DiagReport {
    let mut issues = Vec::new();
    let mut passed = Vec::new();
    let admin = win::is_admin();

    // 1. Rights
    if admin {
        passed.push("Права администратора".into());
    } else {
        issues.push(Issue {
            id: "admin",
            severity: "error",
            title: "VELIORA App запущена без прав администратора".into(),
            impact: "Не будет работать: Zapret и тест стратегий. Telegram-прокси работает.".into(),
            detail: "Драйвер WinDivert, через который Zapret обрабатывает трафик, загружается только с правами администратора.".into(),
            steps: vec![
                "Нажмите «Запросить права» и подтвердите запрос Windows.".into(),
                "Чтобы не подтверждать каждый раз, включите в настройках «Запускать вместе с Windows» — задача запускается сразу с нужными правами.".into(),
            ],
            commands: vec![],
            actions: vec![Action { kind: "elevate", label: "Запросить права" }],
        });
    }

    // 2. Base Filtering Engine
    match win::service("BFE") {
        Some(s) if s.running() => passed.push("Служба «Базовый механизм фильтрации»".into()),
        other => {
            let what = match &other {
                None => "отсутствует в системе",
                Some(s) if s.start_type == 4 => "отключена",
                _ => "остановлена",
            };
            issues.push(Issue {
                id: "bfe",
                severity: "error",
                title: format!("Служба «Базовый механизм фильтрации» {what}"),
                impact: "Не будет работать: Zapret.".into(),
                detail: "WinDivert работает поверх службы Base Filtering Engine (BFE). Её часто отключают «оптимизаторы» и облегчённые сборки Windows.".into(),
                steps: vec![
                    "Нажмите «Открыть службы», найдите «Базовый механизм фильтрации» (Base Filtering Engine).".into(),
                    "Тип запуска — «Автоматически», нажмите «Применить», затем «Запустить».".into(),
                    "Если кнопки недоступны — выполните команды ниже в командной строке администратора.".into(),
                ],
                commands: vec!["sc config BFE start= auto".into(), "sc start BFE".into()],
                actions: vec![Action { kind: "services", label: "Открыть службы" }],
            });
        }
    }

    // 3. Component files
    let zdir = zapret::active_dir(ctx.resources);
    let missing: Vec<&str> = ["winws.exe", "WinDivert.dll", "WinDivert64.sys", "cygwin1.dll"].into_iter().filter(|f| !zdir.join("bin").join(f).exists()).collect();
    if missing.is_empty() {
        passed.push("Файлы Zapret".into());
    } else {
        issues.push(Issue {
            id: "zapret-files",
            severity: "error",
            title: "Файлы Zapret не найдены".into(),
            impact: "Не будет работать: Zapret.".into(),
            detail: format!("Нет файлов: {}. Чаще всего их помещает в карантин антивирус: драйвер перехвата трафика похож на подозрительную программу.", missing.join(", ")),
            steps: vec![
                "Откройте «Безопасность Windows» → «Защита от вирусов и угроз» → «Журнал защиты» и восстановите файлы VELIORA.".into(),
                format!("Добавьте в исключения папки «{}» и папку установки VELIORA App.", win::program_data().display()),
                "Переустановите VELIORA App, если файлы не восстановились.".into(),
            ],
            commands: vec![],
            actions: vec![Action { kind: "security", label: "Открыть «Безопасность Windows»" }],
        });
    }
    let tg_bin = crate::tgws::binary(ctx.resources);
    if tg_bin.exists() {
        passed.push("Файл Telegram-прокси".into());
    } else {
        issues.push(Issue {
            id: "tgws-file",
            severity: "error",
            title: "Файл Telegram-прокси не найден".into(),
            impact: "Не будет работать: Telegram-прокси.".into(),
            detail: format!("Нет файла {}. Антивирусы иногда ошибочно удаляют упакованные Python-программы.", tg_bin.display()),
            steps: vec!["Восстановите файл из карантина антивируса и добавьте папку установки в исключения.".into(), "Или переустановите VELIORA App.".into()],
            commands: vec![],
            actions: vec![Action { kind: "security", label: "Открыть «Безопасность Windows»" }],
        });
    }

    // 4. Telegram port
    if !ctx.tgws_running {
        let addr = format!("{}:{}", ctx.tgws.host, ctx.tgws.port);
        let free = std::net::TcpListener::bind(addr.as_str()).is_ok();
        if free {
            passed.push(format!("Порт Telegram-прокси {}", ctx.tgws.port));
        } else {
            issues.push(Issue {
                id: "tgws-port",
                severity: "error",
                title: format!("Порт {} занят другой программой", ctx.tgws.port),
                impact: "Не будет работать: Telegram-прокси.".into(),
                detail: format!("Прокси должен слушать {addr}, но порт уже используется или зарезервирован Windows."),
                steps: vec![
                    "Проще всего — указать другой порт на странице Telegram (например, 1444) и заново добавить ссылку в Telegram.".into(),
                    "Или найдите программу, которая заняла порт: первая команда покажет номер процесса (PID), вторая — его имя.".into(),
                ],
                commands: vec![format!("netstat -ano | findstr :{}", ctx.tgws.port), "tasklist /FI \"PID eq 1234\"".into()],
                actions: vec![],
            });
        }
    }

    let services = win::services_all();
    // 5. Other bypass tools
    let conflicts = crate::zapret_conflicts::scan(ctx.zapret_pid, false);
    if conflicts.is_empty() {
        passed.push("Другие программы обхода".into());
    } else {
        issues.push(Issue {
            id: "conflict", severity: "error",
            title: "Обнаружена другая сетевая программа".into(),
            impact: "Она может использовать тот же механизм фильтрации.".into(),
            detail: {
                let processes = conflicts.iter().map(|item| format!("{} (PID {})", item.label, item.pid)).collect::<Vec<_>>().join(", ");
                let drivers = crate::zapret_conflicts::drivers().into_iter().map(|driver|
                    format!("{} · {}", driver.name, match driver.state { 4 => "работает", 1 => "остановлена", _ => "переходное состояние" }))
                    .collect::<Vec<_>>().join(", ");
                if drivers.is_empty() { processes } else { format!("{processes}. Службы драйвера: {drivers}.") }
            },
            steps: vec!["При включении VELIORA предложит безопасную остановку, если Windows подтверждает владельца службы.".into(),
                "Если такой вариант недоступен, выключите обход в указанном приложении и повторите попытку.".into()],
            commands: vec![], actions: vec![],
        });
    }

    // 6. VPN tunnel
    match tester::detect_vpn() {
        None => passed.push("VPN-туннели".into()),
        Some(name) => issues.push(Issue {
            id: "vpn",
            severity: "warn",
            title: "Активен VPN-туннель".into(),
            impact: "Работают некорректно: Zapret и тест стратегий.".into(),
            detail: format!("Трафик идёт через виртуальный адаптер «{name}». Zapret обрабатывает пакеты до туннеля, а тест стратегий измеряет работу VPN, а не вашей сети."),
            steps: vec!["Выключите туннель (TUN-режим) в VPN-программе.".into(), "Если VPN нужен одновременно с Zapret — просто учитывайте, что результаты теста не показательны.".into()],
            commands: vec![],
            actions: vec![Action { kind: "network", label: "Сетевые подключения" }],
        }),
    }

    // 7. System proxy set by someone else
    match crate::proxy::read() {
        Ok(p) if p.enabled && !ctx.own_proxy_active => issues.push(Issue {
            id: "proxy",
            severity: "warn",
            title: "В Windows включён системный прокси".into(),
            impact: "Браузеры идут через прокси, а не напрямую — обход и тест могут вести себя иначе.".into(),
            detail: format!("Адрес прокси: {}. Его включила другая программа или вы вручную.", p.server.unwrap_or_default()),
            steps: vec!["Если вы не пользуетесь этим прокси — выключите «Использовать прокси-сервер» в параметрах Windows.".into()],
            commands: vec![],
            actions: vec![Action { kind: "proxy-settings", label: "Параметры прокси" }],
        }),
        _ => passed.push("Системный прокси".into()),
    }

    // 8. AdGuard
    if !win::find_processes("AdguardSvc.exe").is_empty() {
        issues.push(Issue {
            id: "adguard",
            severity: "warn",
            title: "Работает AdGuard".into(),
            impact: "Может мешать Discord и YouTube через Zapret.".into(),
            detail: "AdGuard перехватывает HTTPS-трафик своим фильтром, из-за чего часть стратегий Zapret перестаёт работать.".into(),
            steps: vec!["Отключите фильтрацию HTTPS в AdGuard или добавьте Discord и YouTube в исключения.".into()],
            commands: vec![],
            actions: vec![],
        });
    } else {
        passed.push("AdGuard".into());
    }

    // 9. Killer Networking
    let killer: Vec<String> = services.iter().filter(|s| s.name.to_ascii_lowercase().contains("killer") && s.running()).map(|s| s.display.clone()).collect();
    if !killer.is_empty() {
        issues.push(Issue {
            id: "killer",
            severity: "warn",
            title: "Найдены службы Killer Networking".into(),
            impact: "Известный конфликт с WinDivert.".into(),
            detail: format!("Службы: {}. Они встраиваются в сетевой стек и мешают перехвату пакетов.", killer.join(", ")),
            steps: vec!["Удалите Killer Control Center / Killer Intelligence Center в «Приложениях» или отключите эти службы.".into()],
            commands: vec![],
            actions: vec![Action { kind: "apps", label: "Установленные приложения" }],
        });
    }

    // 10. hosts entries for YouTube
    let hosts = std::path::PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join(r"System32\drivers\etc\hosts");
    let hosts_text = std::fs::read_to_string(&hosts).unwrap_or_default().to_ascii_lowercase();
    let bad = hosts_text.lines().any(|l| !l.trim_start().starts_with('#') && (l.contains("youtube.com") || l.contains("youtu.be") || l.contains("discord")));
    if bad {
        issues.push(Issue {
            id: "hosts",
            severity: "warn",
            title: "В файле hosts есть записи для YouTube или Discord".into(),
            impact: "Сайты могут открываться по устаревшим адресам.".into(),
            detail: format!("В {} указаны адреса для youtube.com, youtu.be или discord. Обычно их оставляют старые программы обхода.", hosts.display()),
            steps: vec!["Откройте hosts в Блокноте от имени администратора и удалите эти строки.".into()],
            commands: vec![format!("notepad {}", hosts.display())],
            actions: vec![],
        });
    } else {
        passed.push("Файл hosts".into());
    }

    issues.sort_by_key(|i| if i.severity == "error" { 0 } else { 1 });
    DiagReport { issues, passed, admin }
}

/// Opens the Windows place a diagnostic step refers to (fixed list only).
pub fn open(kind: &str) -> Result<(), String> {
    let (file, params): (&str, Option<&str>) = match kind {
        "services" => ("services.msc", None),
        "security" => ("windowsdefender:", None),
        "taskmgr" => ("taskmgr.exe", None),
        "proxy-settings" => ("ms-settings:network-proxy", None),
        "apps" => ("ms-settings:appsfeatures", None),
        "network" => ("ncpa.cpl", None),
        _ => return Err("Неизвестное действие".into()),
    };
    win::shell_execute(None, file, params, None).map_err(|e| format!("Windows не открыла окно (код {e})"))
}
