//! Zapret bundle versions from the official Flowseal/zapret-discord-youtube
//! GitHub releases. A version is installed only when GitHub publishes a SHA-256
//! digest for its archive and the downloaded bytes match it.
use crate::component::Component;
use crate::config::ZapretConfig;
use crate::{logs, win, zapret};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const ZAPRET_REPO: &str = "Flowseal/zapret-discord-youtube";
const TGWS_REPO: &str = "Flowseal/tg-ws-proxy";
const MAX_ARCHIVE: u64 = 64 * 1024 * 1024;
const MAX_UNPACKED: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 2000;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub tag: String,
    pub name: String,
    pub published_at: String,
    pub notes: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub size: u64,
    pub sha256: Option<String>,
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("VELIORA-App-Updater")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())
}

fn net_err(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "превышено время ожидания".into()
    } else if e.is_connect() {
        "нет связи с сервером".into()
    } else if let Some(s) = e.status() {
        if s.as_u16() == 403 || s.as_u16() == 429 {
            format!("GitHub ограничил число запросов (HTTP {}), повторите позже", s.as_u16())
        } else {
            format!("сервер ответил ошибкой HTTP {}", s.as_u16())
        }
    } else {
        e.to_string()
    }
}

fn get_json(url: &str) -> Result<serde_json::Value, String> {
    client()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(net_err)?
        .json()
        .map_err(|e| e.to_string())
}

pub fn zapret_releases() -> Result<Vec<Release>, String> {
    let v = get_json(&format!("https://api.github.com/repos/{ZAPRET_REPO}/releases?per_page=15"))?;
    let arr = v.as_array().ok_or("неверный ответ GitHub")?;
    Ok(arr
        .iter()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false))
        .map(|r| {
            let asset = r["assets"].as_array().and_then(|a| a.iter().find(|x| x["name"].as_str().is_some_and(|n| n.to_ascii_lowercase().ends_with(".zip"))));
            Release {
                tag: r["tag_name"].as_str().unwrap_or_default().to_string(),
                name: r["name"].as_str().unwrap_or_default().to_string(),
                published_at: r["published_at"].as_str().unwrap_or_default().to_string(),
                notes: r["body"].as_str().unwrap_or_default().chars().take(6000).collect(),
                asset_name: asset.and_then(|a| a["name"].as_str()).map(String::from),
                asset_url: asset.and_then(|a| a["browser_download_url"].as_str()).map(String::from),
                size: asset.and_then(|a| a["size"].as_u64()).unwrap_or(0),
                sha256: asset.and_then(|a| a["digest"].as_str()).and_then(|d| d.strip_prefix("sha256:")).map(String::from),
            }
        })
        .collect())
}

pub fn tgws_latest() -> Result<String, String> {
    let v = get_json(&format!("https://api.github.com/repos/{TGWS_REPO}/releases/latest"))?;
    Ok(v["tag_name"].as_str().unwrap_or_default().trim_start_matches('v').to_string())
}

/// Compares dotted versions numerically ("1.10.4" > "1.9.8c").
pub fn newer(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<(u64, String)> {
        s.trim_start_matches('v')
            .split(['.', '-'])
            .map(|p| {
                let digits: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
                (digits.parse().unwrap_or(0), p[digits.len()..].to_string())
            })
            .collect()
    };
    parse(a) > parse(b)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub version: String,
    pub strategies: usize,
}

fn unpack(bytes: &[u8], staging: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("Архив повреждён: {e}"))?;
    if zip.len() > MAX_FILES {
        return Err("В архиве слишком много файлов.".into());
    }
    // The bundle may sit in a top-level folder; find the folder that holds general.bat.
    let mut root = None;
    for i in 0..zip.len() {
        let f = zip.by_index(i).map_err(|e| e.to_string())?;
        if let Some(p) = f.enclosed_name() {
            if p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("general.bat")) {
                root = Some(p.parent().map(|x| x.to_path_buf()).unwrap_or_default());
                break;
            }
        }
    }
    let root = root.ok_or("В архиве нет general.bat — это не сборка Zapret.")?;
    let _ = std::fs::remove_dir_all(staging);
    std::fs::create_dir_all(staging).map_err(|e| e.to_string())?;
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(path) = f.enclosed_name() else { return Err("Небезопасный путь в архиве.".into()) };
        let Ok(rel) = path.strip_prefix(&root) else { continue };
        if rel.as_os_str().is_empty() || f.is_dir() {
            continue;
        }
        if f.is_symlink() {
            return Err("В архиве есть ссылка — такой архив не устанавливается.".into());
        }
        total += f.size();
        if total > MAX_UNPACKED {
            return Err("Архив распаковывается в слишком большой объём.".into());
        }
        let out = staging.join(rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::with_capacity(f.size() as usize);
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::write(&out, buf).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn install_zapret(tag: &str, resources: &Path, zap: &Arc<Component>, cfg: &ZapretConfig) -> Result<InstallResult, String> {
    if !win::is_admin() {
        return Err("Установка сборки Zapret требует прав администратора.".into());
    }
    let rel = zapret_releases()?.into_iter().find(|r| r.tag == tag).ok_or("Версия не найдена на GitHub.")?;
    let url = rel.asset_url.clone().ok_or("В релизе нет .zip-архива.")?;
    let expected = rel.sha256.clone().ok_or("GitHub не публикует SHA-256 для этого архива — проверенная установка невозможна.")?;
    if rel.size > MAX_ARCHIVE {
        return Err("Архив слишком большой.".into());
    }
    logs::info("zapret", format!("загрузка сборки {tag}…"));
    let bytes = client()?.get(&url).send().and_then(|r| r.error_for_status()).and_then(|r| r.bytes()).map_err(net_err)?;
    let actual: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err(format!("Контрольная сумма не совпала (ожидалась {}…, получена {}…). Установка отменена.", &expected[..12], &actual[..12]));
    }
    logs::info("zapret", format!("SHA-256 подтверждена: {}…", &actual[..16]));
    let staging = win::program_data().join("zapret.new");
    std::fs::create_dir_all(win::program_data()).map_err(|e| e.to_string())?;
    unpack(&bytes, &staging)?;

    let was_running = zap.alive();
    if was_running {
        zapret::stop(zap);
        std::thread::sleep(Duration::from_millis(400));
    }
    zapret::unload_driver();
    let result = zapret::finalize_install(&staging, tag, "update");
    let _ = std::fs::remove_dir_all(&staging);
    result?;
    let dir = zapret::runtime();
    let count = zapret::strategies(&dir).len();
    if was_running && cfg.strategy.as_ref().is_some_and(|s| dir.join(s).exists()) {
        let _ = zapret::start(zap, resources, cfg);
    }
    Ok(InstallResult { version: tag.to_string(), strategies: count })
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_compare() {
        assert!(super::newer("1.10.4", "1.9.8c"));
        assert!(super::newer("v1.11.4", "1.11.3"));
        assert!(!super::newer("1.10.4", "1.10.4"));
        assert!(super::newer("1.9.8c", "1.9.8"));
    }
}
