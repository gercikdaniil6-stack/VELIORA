use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

const RELEASE_PATH: &str = "/gercikdaniil6-stack/VELIORA/releases/download/";
const MAX_INSTALLER_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Default)]
pub struct AppUpdater {
    available: Mutex<Option<Update>>,
    busy: AtomicBool,
}

struct Operation<'a>(&'a AtomicBool);
impl Drop for Operation<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl AppUpdater {
    fn begin(&self) -> Result<Operation<'_>, String> {
        self.busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map(|_| Operation(&self.busy))
            .map_err(|_| "Операция обновления уже выполняется.".into())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    version: String,
    notes: String,
}

#[derive(Serialize)]
pub struct CheckResult {
    latest: Option<String>,
    offer: Option<Offer>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    downloaded: u64,
    total: Option<u64>,
    installing: bool,
}

fn release_asset(url: &reqwest::Url, version: &str) -> bool {
    let prefix = format!("{RELEASE_PATH}v{version}/");
    let expected = format!("VELIORA.App_{version}_x64-setup.exe");
    url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url.path().strip_prefix(&prefix).is_some_and(|name| {
            name == expected
                || name == expected.replace("VELIORA.App", "VELIORA_App")
                || name == expected.replace("VELIORA.App", "VELIORA%20App")
        })
}

fn download_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(12))
        .timeout(Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let url = attempt.url();
            if attempt.previous().len() >= 5
                || url.scheme() != "https"
                || !url.username().is_empty()
                || url.password().is_some()
                || url.port().is_some()
                || !matches!(
                    url.host_str(),
                    Some(
                        "github.com"
                            | "release-assets.githubusercontent.com"
                            | "objects.githubusercontent.com"
                    )
                )
            {
                attempt.error("Недоверенный адрес загрузки.")
            } else {
                attempt.follow()
            }
        }))
        .user_agent("VELIORA-App-Updater")
        .build()
        .map_err(|_| "Не удалось подготовить загрузку.".into())
}

#[tauri::command]
pub async fn app_update_check(
    app: AppHandle,
    state: State<'_, AppUpdater>,
) -> Result<CheckResult, String> {
    let _op = state.begin()?;
    let latest = Arc::new(Mutex::new(None));
    let version = latest.clone();
    let available = app
        .updater_builder()
        .timeout(Duration::from_secs(18))
        .version_comparator(move |installed, release| {
            if !release.version.pre.is_empty() {
                return false;
            }
            *version.lock() = Some(release.version.to_string());
            release.version > installed
        })
        .build()
        .map_err(|_| "Не удалось запустить проверку обновлений.")?
        .check()
        .await
        .map_err(|_| {
            "Не удалось получить данные обновления с GitHub. Проверьте сеть и попробуйте позже."
        })?;
    if let Some(update) = &available {
        if !release_asset(&update.download_url, &update.version) {
            return Err("В релизе указан недопустимый адрес установщика.".into());
        }
    }
    let offer = available.as_ref().map(|update| Offer {
        version: update.version.clone(),
        notes: update
            .body
            .as_deref()
            .unwrap_or_default()
            .chars()
            .take(8000)
            .collect(),
    });
    *state.available.lock() = available;
    let latest = latest.lock().clone();
    Ok(CheckResult { latest, offer })
}

#[tauri::command]
pub async fn app_update_install(
    app: AppHandle,
    state: State<'_, AppUpdater>,
    version: String,
    progress: Channel<Progress>,
) -> Result<(), String> {
    let _op = state.begin()?;
    let update = state
        .available
        .lock()
        .clone()
        .filter(|update| update.version == version)
        .ok_or("Сначала проверьте доступность этой версии.")?;
    if !release_asset(&update.download_url, &update.version) {
        return Err("В релизе указан недопустимый адрес установщика.".into());
    }
    let client = download_client()?;
    let mut response = client
        .get(update.download_url.clone())
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|_| "Загрузка с GitHub не началась. Проверьте подключение.")?;
    let total = response.content_length();
    if total.is_some_and(|size| size == 0 || size > MAX_INSTALLER_BYTES) {
        return Err("Размер установщика превышает допустимый предел или файл пуст.".into());
    }
    let _ = progress.send(Progress {
        downloaded: 0,
        total,
        installing: false,
    });
    let mut bytes = Vec::new();
    let mut transferred = 0;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Загрузка прервалась. Повторите попытку.")?
    {
        if bytes.len() as u64 + chunk.len() as u64 > MAX_INSTALLER_BYTES {
            return Err("Размер загруженного установщика превышает допустимый предел.".into());
        }
        bytes.extend_from_slice(&chunk);
        let downloaded = bytes.len() as u64;
        if downloaded - transferred >= 128 * 1024 {
            let _ = progress.send(Progress {
                downloaded,
                total,
                installing: false,
            });
            transferred = downloaded;
        }
    }
    if bytes.is_empty() || total.is_some_and(|size| size != bytes.len() as u64) {
        return Err("Получен неполный установщик. Повторите загрузку.".into());
    }
    let download_size = bytes.len() as u64;
    crate::blocking(move || {
        update.verify(&bytes).map_err(|_| "Подпись установщика недействительна. Установка отменена.".to_owned())?;
        let core = app.state::<crate::AppState>();
        let _maintenance = core.network_maintenance.try_lock()
            .ok_or("Дождитесь завершения обслуживания сетевых компонентов.")?;
        if core.tester.is_running() || core.quitting.load(Ordering::SeqCst) {
            return Err("Дождитесь завершения тестирования или перезапустите VELIORA.".into());
        }
        // Quiesce only after download and verification. Do not launch an installer
        // while VELIORA still owns the user's Windows proxy settings.
        core.quitting.store(true, Ordering::SeqCst);
        let restore = crate::restore_proxy(&app, &mut core.snapshot.lock());
        if let Err(error) = restore {
            core.quitting.store(false, Ordering::SeqCst);
            return Err(format!("Не удалось восстановить системный прокси: {error}"));
        }
        let _ = progress.send(Progress { downloaded: download_size, total: Some(download_size), installing: true });
        // Windows exits the process on successful handoff. Leave the catalog worker
        // available if ShellExecute fails or elevation is cancelled before launch.
        crate::tgws::stop(&core.tgws);
        crate::zapret::stop(&core.zapret);
        match update.install(bytes) {
            Ok(()) => Ok(()), // On Windows the plugin exits after launching NSIS.
            Err(error) => {
                crate::logs::warn("app", format!("Не удалось запустить установщик VELIORA: {error}"));
                core.quitting.store(false, Ordering::SeqCst);
                Err("Windows не запустила установщик. Проверьте, разрешили ли вы запрос Windows, и повторите попытку. Текущая версия остаётся установленной.".into())
            }
        }
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_official_versioned_installers() {
        let url = |s: &str| reqwest::Url::parse(s).unwrap();
        let correct = "https://github.com/gercikdaniil6-stack/VELIORA/releases/download/v2.3.0/VELIORA_App_2.3.0_x64-setup.exe";
        assert!(release_asset(&url(correct), "2.3.0"));
        assert!(!release_asset(&url(correct), "2.3.1"));
        for untrusted in [
            correct.replace("github.com", "github.com.example.org"),
            correct.replace("gercikdaniil6-stack", "another-owner"),
            correct.replace("https", "http"),
            correct.replace("x64-setup.exe", "x64-setup.exe?anything"),
        ] {
            assert!(!release_asset(&url(&untrusted), "2.3.0"));
        }
    }
}
