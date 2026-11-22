use crate::{logs, win, zapret};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{io::Read, path::{Path, PathBuf}};
use windows_sys::Win32::System::Services::{SERVICE_ACCEPT_STOP, SERVICE_RUNNING, SERVICE_WIN32_OWN_PROCESS};

const IMAGES: &[&str] = &["winws.exe", "winws2.exe", "winws64.exe", "goodbyedpi.exe", "zapret.exe"];
const DRIVERS: &[&str] = &["WinDivert", "WinDivert14", "WinDivert64"];
const MAX_APPROVALS: usize = 32;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub label: String,
    pub image: String,
    pub path: Option<PathBuf>,
    pub pid: u32,
    pub confirmed: bool,
    pub signature_verified: Option<bool>,
    pub token: Option<String>,
    #[serde(skip)]
    action: Option<StopAction>,
}

struct StopAction {
    service: win::ServiceInfo,
    identity: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub conflicts: Vec<Conflict>,
    pub drivers: Vec<win::ServiceInfo>,
}

fn service_image(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    let path = if let Some(quoted) = command.strip_prefix('"') {
        let (path, rest) = quoted.split_once('"')?;
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) { return None; }
        path
    } else {
        command.split_whitespace().next()?
    };
    let path = win::service_file(path);
    (path.is_absolute() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe"))).then_some(path)
}

fn fingerprint(svc: &win::ServiceInfo, path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if !(1..=64 * 1024 * 1024).contains(&len) { return None; }
    let mut hash = Sha256::new();
    for item in [svc.name.as_str(), svc.binary.as_str(), svc.account.as_str()] {
        hash.update(item.as_bytes());
        hash.update([0]);
    }
    let mut buf = [0; 16384];
    let mut limited = (&mut file).take(len + 1);
    let mut total = 0;
    loop {
        let n = limited.read(&mut buf).ok()?;
        if n == 0 { break; }
        total += n as u64;
        hash.update(&buf[..n]);
    }
    (total == len).then(|| hex::encode(hash.finalize()))
}

fn action_for(pid: u32, path: &Path, services: &[win::ServiceInfo]) -> Option<StopAction> {
    let only = services.iter().filter(|svc| svc.pid == pid).collect::<Vec<_>>();
    if only.len() != 1 { return None; }
    let svc = win::service(&only[0].name)?;
    if svc.pid != pid || svc.kind != SERVICE_WIN32_OWN_PROCESS || svc.state != SERVICE_RUNNING ||
        svc.accepted & SERVICE_ACCEPT_STOP == 0 || svc.account.is_empty() || svc.binary.contains("--dry-run") ||
        service_image(&svc.binary)?.canonicalize().ok()?.as_path() != path {
        return None;
    }
    Some(StopAction { identity: fingerprint(&svc, path)?, service: svc })
}

pub fn scan(own_pid: Option<u32>, detailed: bool) -> Vec<Conflict> {
    let processes = win::processes();
    let candidates: Vec<_> = processes.iter().filter(|p| Some(p.pid) != own_pid &&
        IMAGES.iter().any(|name| p.name.eq_ignore_ascii_case(name))).collect();
    if candidates.is_empty() { return Vec::new(); }
    let services = if detailed { win::services_all() } else { Vec::new() };
    candidates.into_iter().filter_map(|p| {
        let module = win::uses_windivert(p.pid);
        if module == Some(false) { return None; }
        let path = win::process_path(p.pid);
        let canonical = path.as_ref().and_then(|image| image.canonicalize().ok());
        let action = if detailed && module == Some(true) && win::is_admin() {
            canonical.as_deref().and_then(|image| action_for(p.pid, image, &services))
        } else { None };
        let owner = processes.iter().find(|process| process.pid == p.parent)
            .and_then(|parent| win::process_path(parent.pid))
            .and_then(|path| path.file_stem().map(|stem| stem.to_string_lossy().into_owned()))
            .filter(|name| !["services", "cmd", "explorer", "powershell", "pwsh"].iter().any(|host| name.eq_ignore_ascii_case(host)));
        let label = action.as_ref().map(|a| {
            if a.service.display.is_empty() { a.service.name.clone() } else { a.service.display.clone() }
        }).or(owner).unwrap_or_else(|| p.name.clone());
        let signature_verified = if detailed { path.as_deref().map(win::verified_signature) } else { None };
        let token = action.as_ref().map(|a| format!("{}:{}", p.pid, a.identity));
        Some(Conflict { label, image: p.name.clone(), path, pid: p.pid, confirmed: module == Some(true), signature_verified, token, action })
    }).collect()
}

pub fn inspect(own_pid: Option<u32>) -> Inspection {
    Inspection { conflicts: scan(own_pid, true), drivers: drivers() }
}

pub fn drivers() -> Vec<win::ServiceInfo> {
    DRIVERS.iter().filter_map(|name| win::service(name)).collect()
}

fn approval_path() -> PathBuf {
    win::program_data().join("zapret-service-approvals.json")
}

fn approvals() -> Vec<String> {
    std::fs::File::open(approval_path()).ok().and_then(|f| serde_json::from_reader(f.take(8192)).ok()).unwrap_or_default()
}

pub fn resolve(token: &str, remember: bool) -> Result<(), String> {
    zapret::protect_runtime()?;
    let item = scan(None, true).into_iter().find(|item| item.token.as_deref() == Some(token))
        .ok_or("Состояние программы изменилось. Проверьте конфликт ещё раз.")?;
    let action = item.action.ok_or("Не удалось подтвердить владельца службы.")?;
    win::stop_verified_service(&action.service)?;
    if remember {
        let mut allowed = approvals();
        if !allowed.contains(&action.identity) {
            allowed.push(action.identity);
            if allowed.len() > MAX_APPROVALS { allowed.remove(0); }
            let path = approval_path();
            let temp = path.with_extension("new");
            let bytes = serde_json::to_vec(&allowed).map_err(|e| e.to_string())?;
            if let Err(error) = std::fs::write(&temp, bytes).and_then(|()| std::fs::rename(&temp, &path)) {
                logs::warn("zapret", format!("Служба остановлена, но разрешение не сохранено: {error}"));
            }
        }
    }
    logs::info("zapret", format!("Служба «{}» остановлена через Windows.", item.label));
    Ok(())
}

pub fn forget() -> Result<(), String> {
    zapret::protect_runtime()?;
    match std::fs::remove_file(approval_path()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

pub fn has_approvals() -> bool { approval_path().is_file() }

pub fn resolve_approved() -> Result<bool, String> {
    zapret::protect_runtime()?;
    let allowed = approvals();
    let items = scan(None, true);
    if !all_approved(&items, &allowed) {
        return Ok(false);
    }
    for item in items {
        let action = item.action.ok_or("Не удалось подтвердить владельца службы.")?;
        win::stop_verified_service(&action.service)?;
        logs::info("zapret", format!("Применено разрешение на остановку «{}».", item.label));
    }
    Ok(true)
}

fn all_approved(items: &[Conflict], allowed: &[String]) -> bool {
    !items.is_empty() && items.iter().all(|item| {
        item.action.as_ref().is_some_and(|action| allowed.contains(&action.identity))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remembered_permission_is_limited_to_verified_identity() {
        let mut item = Conflict {
            label: "Fixture".into(), image: "winws.exe".into(), path: None, pid: 4321,
            confirmed: true, signature_verified: None, token: None, action: None,
        };
        assert!(!all_approved(std::slice::from_ref(&item), &["known".into()]));
        item.action = Some(StopAction {
            identity: "known".into(),
            service: win::ServiceInfo {
                name: "Fixture".into(), display: "Fixture".into(), state: 4, start_type: 3,
                binary: "fixture".into(), account: "fixture".into(), pid: 4321, kind: 16, accepted: 1,
            },
        });
        assert!(!all_approved(std::slice::from_ref(&item), &[]));
        assert!(!all_approved(std::slice::from_ref(&item), &["changed".into()]));
        assert!(all_approved(std::slice::from_ref(&item), &["known".into()]));
    }

    #[test]
    fn only_unambiguous_absolute_service_executables_are_accepted() {
        assert!(service_image(r#""C:\\Program Files\\Zapret\\winws.exe" --wf-tcp=443"#).is_some());
        assert!(service_image(r"C:\\Program Files\\Zapret\\winws.exe --wf-tcp=443").is_none());
        assert!(service_image(r"winws.exe --wf-tcp=443").is_none());
        assert!(service_image(r#""C:\\zapret\\WinDivert64.sys""#).is_none());
    }
}
