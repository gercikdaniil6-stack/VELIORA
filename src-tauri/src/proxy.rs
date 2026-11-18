//! Windows system proxy (WinINet, per-user) with snapshot/restore.
//!
//! `apply` remembers the user's previous proxy settings the first time it runs,
//! so `restore` always returns the machine to exactly how it was before
//! VELIORA connected — never to some hard-coded default.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyState {
    pub enabled: bool,
    pub server: Option<String>,
    pub bypass: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    enable: Option<u32>,
    server: Option<String>,
    bypass: Option<String>,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    const PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
    const INTERNET_OPTION_REFRESH: u32 = 37;
    const INTERNET_OPTION_SETTINGS_CHANGED: u32 = 39;

    fn key(write: bool) -> Result<RegKey, String> {
        let flags = if write { KEY_READ | KEY_WRITE } else { KEY_READ };
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(PATH, flags)
            .map_err(|e| format!("Не удалось открыть настройки Интернета: {e}"))
    }

    /// Tell WinINet consumers (browsers, apps) that settings changed.
    fn notify() {
        unsafe {
            use windows_sys::Win32::Networking::WinInet::InternetSetOptionW;
            InternetSetOptionW(std::ptr::null(), INTERNET_OPTION_SETTINGS_CHANGED, std::ptr::null(), 0);
            InternetSetOptionW(std::ptr::null(), INTERNET_OPTION_REFRESH, std::ptr::null(), 0);
        }
    }

    pub fn read() -> Result<ProxyState, String> {
        let k = key(false)?;
        Ok(ProxyState {
            enabled: k.get_value::<u32, _>("ProxyEnable").unwrap_or(0) == 1,
            server: k.get_value::<String, _>("ProxyServer").ok(),
            bypass: k.get_value::<String, _>("ProxyOverride").ok(),
        })
    }

    pub fn snapshot() -> Result<Snapshot, String> {
        let k = key(false)?;
        Ok(Snapshot {
            enable: k.get_value::<u32, _>("ProxyEnable").ok(),
            server: k.get_value::<String, _>("ProxyServer").ok(),
            bypass: k.get_value::<String, _>("ProxyOverride").ok(),
        })
    }

    pub fn set(server: &str, bypass: &str) -> Result<(), String> {
        let k = key(true)?;
        k.set_value("ProxyServer", &server.to_string()).map_err(|e| e.to_string())?;
        k.set_value("ProxyOverride", &bypass.to_string()).map_err(|e| e.to_string())?;
        k.set_value("ProxyEnable", &1u32).map_err(|e| e.to_string())?;
        notify();
        Ok(())
    }

    pub fn restore(s: &Snapshot) -> Result<(), String> {
        let k = key(true)?;
        match s.enable {
            Some(v) => k.set_value("ProxyEnable", &v).map_err(|e| e.to_string())?,
            None => k.set_value("ProxyEnable", &0u32).map_err(|e| e.to_string())?,
        }
        match &s.server {
            Some(v) => k.set_value("ProxyServer", v).map_err(|e| e.to_string())?,
            None => { let _ = k.delete_value("ProxyServer"); }
        }
        match &s.bypass {
            Some(v) => k.set_value("ProxyOverride", v).map_err(|e| e.to_string())?,
            None => { let _ = k.delete_value("ProxyOverride"); }
        }
        notify();
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn read() -> Result<ProxyState, String> { Ok(ProxyState::default()) }
    pub fn snapshot() -> Result<Snapshot, String> { Ok(Snapshot { enable: None, server: None, bypass: None }) }
    pub fn set(_: &str, _: &str) -> Result<(), String> { Err("Системный прокси поддерживается только в Windows".into()) }
    pub fn restore(_: &Snapshot) -> Result<(), String> { Ok(()) }
}

pub use imp::{read, restore, set, snapshot};

/// Builds the WinINet `ProxyServer` value for a profile.
pub fn server_string(kind: &str, host: &str, port: u16) -> Result<String, String> {
    let host = host.trim();
    if host.is_empty() || host.contains(|c: char| c.is_whitespace() || c == ';' || c == '=') {
        return Err("Некорректный адрес сервера".into());
    }
    Ok(match kind {
        "socks5" | "socks" => format!("socks={host}:{port}"),
        _ => format!("{host}:{port}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats_server() {
        assert_eq!(server_string("http", "1.2.3.4", 8080).unwrap(), "1.2.3.4:8080");
        assert_eq!(server_string("socks5", "proxy.local", 1080).unwrap(), "socks=proxy.local:1080");
        assert!(server_string("http", "bad host", 1).is_err());
        assert!(server_string("http", "a;b", 1).is_err());
    }
}
