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
    pub installed: bool,
    pub latest: bool,
}

fn trusted_host(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && url.username().is_empty() && url.password().is_none()
        && url.port().is_none() && matches!(url.host_str(),Some("github.com" | "api.github.com" | "release-assets.githubusercontent.com" | "objects.githubusercontent.com"))
}

fn validate_download(raw:&str, expected:&str) -> Result<reqwest::Url,String> {
    if expected.len()!=64 || !expected.bytes().all(|byte|byte.is_ascii_hexdigit()) {
        return Err("GitHub не предоставил корректную SHA-256 для архива.".into());
    }
    let url=reqwest::Url::parse(raw).map_err(|_|"Некорректный адрес загрузки.")?;
    if !trusted_host(&url) || url.host_str()!=Some("github.com")
        || !url.path().starts_with(&format!("/{ZAPRET_REPO}/releases/download/"))
        || url.query().is_some() || url.fragment().is_some() {
        return Err("Архив находится вне официального репозитория Zapret.".into());
    }
    Ok(url)
}

fn read_limited(input:impl Read, maximum:u64)->Result<Vec<u8>,String> {
    let mut bytes=Vec::new();
    input.take(maximum+1).read_to_end(&mut bytes).map_err(|e|format!("Загрузка прервалась: {e}"))?;
    if bytes.len() as u64 > maximum {return Err("Ответ превысил допустимый размер.".into());}
    Ok(bytes)
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("VELIORA-App-Updater")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !trusted_host(attempt.url()) {
                attempt.error("Недоверенный адрес перенаправления обновления")
            } else { attempt.follow() }
        }))
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
    let response = client()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(net_err)?;
    serde_json::from_slice(&read_limited(response,2*1024*1024)?).map_err(|_|"Получены некорректные метаданные релиза.".into())
}

pub fn zapret_releases() -> Result<Vec<Release>, String> {
    let v = get_json(&format!("https://api.github.com/repos/{ZAPRET_REPO}/releases?per_page=15"))?;
    let arr = v.as_array().ok_or("неверный ответ GitHub")?;
    let mut releases: Vec<Release> = arr
        .iter()
        .filter(|r| r["draft"].as_bool() == Some(false) && r["prerelease"].as_bool() == Some(false))
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
                installed: false,
                latest: false,
            }
        })
        .filter(|r| safe_tag(&r.tag))
        .collect();
    releases.sort_by(|a, b| if newer(&a.tag, &b.tag) { std::cmp::Ordering::Less }
        else if newer(&b.tag, &a.tag) { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal });
    if let Some(first) = releases.first_mut() { first.latest = true; }
    Ok(releases)
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
        let f = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(path) = f.enclosed_name() else { return Err("Небезопасный путь в архиве.".into()) };
        let Ok(rel) = path.strip_prefix(&root) else { continue };
        if rel.as_os_str().is_empty() || f.is_dir() {
            continue;
        }
        if f.is_symlink() {
            return Err("В архиве есть ссылка — такой архив не устанавливается.".into());
        }
        if rel.components().any(|item| match item {
            std::path::Component::Normal(name) => {
                let name=name.to_string_lossy();
                name.contains([':', '\\']) || name.ends_with(['.', ' '])
            }
            _ => true,
        }) {
            return Err("Некорректный путь в архиве Zapret.".into());
        }
        total = total.saturating_add(f.size());
        if total > MAX_UNPACKED {
            return Err("Архив распаковывается в слишком большой объём.".into());
        }
        let out = staging.join(rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let expected=f.size();
        let mut output=std::fs::OpenOptions::new().write(true).create_new(true).open(&out).map_err(|e|e.to_string())?;
        let actual=std::io::copy(&mut f.take(expected+1),&mut output).map_err(|e|e.to_string())?;
        if actual!=expected {return Err("Размер файла в архиве не совпадает с метаданными.".into());}
    }
    Ok(())
}

fn safe_tag(tag: &str) -> bool {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    version.split('.').count() >= 2 && version.len() <= 32
        && version.as_bytes().first().is_some_and(u8::is_ascii_digit)
        && version.split('.').all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric()))
}

fn archive_cache() -> std::path::PathBuf {
    win::program_data().join("zapret-archives")
}

fn cached_archive(tag: &str) -> Option<(std::path::PathBuf, String)> {
    if !safe_tag(tag) { return None; }
    let prefix = format!("{tag}_");
    std::fs::read_dir(archive_cache()).ok()?.flatten().find_map(|entry| {
        let name = entry.file_name();
        let hash = name.to_str()?.strip_prefix(&prefix)?.strip_suffix(".zip")?;
        (hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) && entry.file_type().ok()?.is_file())
            .then(|| (entry.path(), hash.to_owned()))
    })
}

pub fn version_catalog(resources: &Path) -> (Vec<Release>, Option<String>) {
    let (mut releases, error) = match zapret_releases() {
        Ok(releases) => (releases, None),
        Err(error) => (Vec::new(), Some(error)),
    };
    let active = zapret::installed_version(resources);
    for release in &mut releases { release.latest &= newer(&release.tag, &active); }
    let mut local = vec![active, zapret::bundled_version(resources)];
    if let Ok(entries) = std::fs::read_dir(archive_cache()) {
        local.extend(entries.flatten().filter_map(|entry| {
            let name = entry.file_name();
            let (tag, _) = name.to_str()?.split_once('_')?;
            cached_archive(tag).map(|_| tag.to_string())
        }));
    }
    for tag in local.into_iter().filter(|tag| safe_tag(tag)) {
        if let Some(release) = releases.iter_mut().find(|r| r.tag == tag) {
            release.installed = true;
        } else {
            releases.push(Release {
                name: tag.clone(), tag, published_at: String::new(), notes: String::new(),
                asset_name: None, asset_url: None, size: 0, sha256: None, installed: true, latest: false,
            });
        }
    }
    (releases, error)
}

fn verify_archive(bytes: &[u8], expected: &str) -> Result<(), String> {
    let actual = hex::encode(Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err("Контрольная сумма архива Zapret не совпала. Установка отменена.".into());
    }
    Ok(())
}

fn download_archive(tag: &str) -> Result<(Vec<u8>, String), String> {
    let release = zapret_releases()?.into_iter().find(|r| r.tag == tag).ok_or("Версия не найдена на GitHub.")?;
    let url = release.asset_url.ok_or("В релизе нет .zip-архива.")?;
    let expected = release.sha256.ok_or("У архива нет SHA-256 от GitHub.")?;
    let url = validate_download(&url, &expected)?;
    if release.size > MAX_ARCHIVE { return Err("Архив слишком большой.".into()); }
    let response = client()?.get(url).send().and_then(|r| r.error_for_status()).map_err(net_err)?;
    let bytes = read_limited(response, MAX_ARCHIVE)?;
    if release.size != 0 && bytes.len() as u64 != release.size {
        return Err("Размер архива не совпадает с метаданными GitHub.".into());
    }
    verify_archive(&bytes, &expected)?;
    Ok((bytes, expected))
}

pub fn install_zapret(tag: &str, resources: &Path, zap: &Arc<Component>, cfg: &ZapretConfig) -> Result<InstallResult, String> {
    if !safe_tag(tag) { return Err("Недопустимый номер версии Zapret.".into()); }
    zapret::protect_runtime()?;
    let current = zapret::installed_version(resources);
    if current == tag { return Err("Эта версия уже активна.".into()); }
    static UPDATING: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
    let _update = UPDATING.try_lock().ok_or("Обновление Zapret уже выполняется.")?;
    logs::info("zapret", format!("Подготовка сборки {tag}."));
    let staging = win::program_data().join("zapret.new");
    let bundled = tag == zapret::bundled_version(resources);
    let mut new_archive = None;
    if bundled {
        if staging.exists() { std::fs::remove_dir_all(&staging).map_err(|e| e.to_string())?; }
        zapret::copy_dir(&zapret::bundled(resources), &staging).map_err(|e| e.to_string())?;
    } else {
        let cached = cached_archive(tag).and_then(|(path, hash)| {
            let checked = std::fs::File::open(&path).map_err(|e| e.to_string())
                .and_then(|f| read_limited(f, MAX_ARCHIVE)).and_then(|bytes| {
                    verify_archive(&bytes, &hash)?;
                    Ok(bytes)
                });
            match checked {
                Ok(bytes) => Some(bytes),
                Err(error) => {
                    logs::warn("zapret", format!("Локальный архив {tag} недоступен: {error}"));
                    let _ = std::fs::remove_file(path);
                    None
                }
            }
        });
        let bytes = if let Some(bytes) = cached { bytes } else {
            let (bytes, hash) = download_archive(tag)?;
            new_archive = Some(hash);
            bytes
        };
        if let Err(error) = unpack(&bytes, &staging) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
        if let Some(hash) = new_archive {
            let dir = archive_cache();
            let temp = dir.join("download.tmp");
            std::fs::create_dir_all(&dir)
                .and_then(|()| std::fs::write(&temp, &bytes))
                .and_then(|()| std::fs::rename(&temp, dir.join(format!("{tag}_{hash}.zip"))))
                .map_err(|e| format!("Не удалось сохранить проверенный архив: {e}"))?;
        }
    }
    let validation = zapret::validate_install(&staging, cfg);
    if let Err(error) = validation {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    let was_running = zap.alive();
    if was_running {
        zapret::stop(zap);
        std::thread::sleep(Duration::from_millis(400));
    }
    let result = zapret::finalize_install(&staging, tag, if bundled { "bundled" } else { "update" });
    let _ = std::fs::remove_dir_all(&staging);
    if let Err(error)=result {
        if was_running {
            zapret::start(zap,resources,cfg).map_err(|restart|format!("{error}. Не удалось запустить прежнюю сборку: {restart}"))?;
        }
        return Err(error);
    }
    let dir = zapret::runtime();
    let count = zapret::strategies(&dir).len();
    if was_running && cfg.strategy.as_ref().is_some_and(|s| dir.join(s).exists()) {
        if let Err(error)=zapret::start(zap,resources,cfg) {
            zapret::rollback_install().map_err(|rollback|format!("Запуск новой сборки: {error}. Откат не удался: {rollback}"))?;
            zapret::start(zap,resources,cfg).map_err(|restart|format!("Новая сборка не запустилась: {error}. Прежняя восстановлена, но её запуск не удался: {restart}"))?;
            return Err(format!("Новая сборка не запустилась. Прежняя восстановлена: {error}"));
        }
    }
    Ok(InstallResult { version: tag.to_string(), strategies: count })
}

#[cfg(test)]
mod tests {
    #[test]
    fn download_policy_and_size_limit() {
        let hash="A".repeat(64);
        assert!(super::validate_download("https://github.com/Flowseal/zapret-discord-youtube/releases/download/1.0/test.zip",&hash).is_ok());
        assert!(super::validate_download("https://example.org/test.zip",&hash).is_err());
        assert!(super::validate_download("https://github.com/Flowseal/zapret-discord-youtube/releases/download/1.0/test.zip","abc").is_err());
        assert_eq!(super::read_limited(&b"abc"[..],3).unwrap(),b"abc");
        assert!(super::read_limited(&b"abcd"[..],3).is_err());
    }
    #[test]
    fn unpack_checks_paths_and_duplicate_entries() {
        use std::io::{Cursor, Write};
        let archive = |names: &[&str]| {
            let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
            for name in names {
                zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                zip.write_all(b"test").unwrap();
            }
            zip.finish().unwrap().into_inner()
        };
        let staging = std::env::temp_dir().join(format!(
            "veliora-archive-test-{}", crate::win::random_hex(8)
        ));
        let good = archive(&["bundle/general.bat", "bundle/bin/winws.exe"]);
        super::unpack(&good, &staging).unwrap();
        assert_eq!(std::fs::read(staging.join("bin/winws.exe")).unwrap(), b"test");
        let traversal = archive(&["general.bat", "../veliora-escape.txt"]);
        assert!(super::unpack(&traversal, &staging).is_err());
        let duplicate = archive(&["general.bat", "bin/winws.exe", "bin/WINWS.exe"]);
        assert!(super::unpack(&duplicate, &staging).is_err());
        std::fs::remove_dir_all(staging).unwrap();
    }

    #[test]
    fn version_compare() {
        use sha2::Digest;
        assert!(super::safe_tag("1.9.9d"));
        assert!(!super::safe_tag("1.2/../../bad"));
        assert!(!super::safe_tag("1..2"));
        assert!(super::verify_archive(b"a", &hex::encode(super::Sha256::digest(b"a"))).is_ok());
        assert!(super::verify_archive(b"modified", &"0".repeat(64)).is_err());
        assert!(super::newer("1.10.4", "1.9.8c"));
        assert!(super::newer("v1.11.4", "1.11.3"));
        assert!(!super::newer("1.10.4", "1.10.4"));
        assert!(super::newer("1.9.8c", "1.9.8"));
    }
}
