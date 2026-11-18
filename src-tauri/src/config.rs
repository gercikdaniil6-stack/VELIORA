//! Persistent settings (config.json in the app data folder). Written
//! atomically with a .backup copy; unknown or missing fields fall back to
//! defaults so older files keep loading.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct TgwsConfig {
    pub host: String,
    pub port: u16,
    /// 32 hex chars (MTProto secret without the "dd" prefix)
    pub secret: String,
    /// Optional own Cloudflare Worker domain (name.user.workers.dev)
    pub worker_domain: String,
    pub auto_start: bool,
    pub verbose: bool,
}

impl Default for TgwsConfig {
    fn default() -> Self {
        Self { host: "127.0.0.1".into(), port: 1443, secret: crate::win::random_hex(16), worker_domain: String::new(), auto_start: false, verbose: false }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ZapretConfig {
    /// File name of the selected strategy (general (ALT).bat …)
    pub strategy: Option<String>,
    pub auto_start: bool,
    /// off | all | tcp | udp
    pub game_filter: String,
    pub game_tcp: String,
    pub game_udp: String,
    /// basic | full | any
    pub ipset_mode: String,
    /// Replacement for ACTIVE_DISCORD_UDP.bin (file in bin/), None = bundle default
    pub fake_discord: Option<String>,
    /// Replacement for ACTIVE_GAME_UDP.bin
    pub fake_game: Option<String>,
}

impl Default for ZapretConfig {
    fn default() -> Self {
        Self {
            strategy: None,
            auto_start: false,
            game_filter: "off".into(),
            game_tcp: "1024-65535".into(),
            game_udp: "1024-65535".into(),
            ipset_mode: "basic".into(),
            fake_discord: None,
            fake_game: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AppPrefs {
    pub launch_at_login: bool,
    pub silent_start: bool,
    pub tray: bool,
    /// Close button hides the window to the tray (when the tray is on)
    pub close_to_tray: bool,
    pub hide_taskbar: bool,
    pub auto_check_updates: bool,
    pub show_splash: bool,
    pub reduce_transparency: bool,
}

impl Default for AppPrefs {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            silent_start: false,
            tray: true,
            close_to_tray: true,
            hide_taskbar: false,
            auto_check_updates: true,
            show_splash: true,
            reduce_transparency: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub tgws: TgwsConfig,
    pub zapret: ZapretConfig,
    pub app: AppPrefs,
    /// Theme and appearance settings, owned by the UI (src/theme/engine.ts).
    pub appearance: Option<serde_json::Value>,
}

pub fn valid_secret(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn valid_host(s: &str) -> bool {
    if s.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    !s.is_empty()
        && s.len() <= 253
        && s.split('.').all(|l| !l.is_empty() && l.len() <= 63 && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') && !l.starts_with('-') && !l.ends_with('-'))
}

/// Accepts "name.user.workers.dev" style domains (no scheme, no path, not an IP).
pub fn valid_domain(s: &str) -> bool {
    s.parse::<std::net::IpAddr>().is_err() && s.contains('.') && valid_host(s)
}

/// "1024-65535,5000" style port lists.
pub fn valid_ports(s: &str) -> bool {
    !s.is_empty()
        && s.split(',').all(|part| {
            let mut it = part.trim().splitn(2, '-');
            let a = it.next().and_then(|x| x.trim().parse::<u32>().ok());
            match (a, it.next()) {
                (Some(a), None) => (1..=65535).contains(&a),
                (Some(a), Some(b)) => b.trim().parse::<u32>().ok().is_some_and(|b| (1..=65535).contains(&a) && (a..=65535).contains(&b)),
                _ => false,
            }
        })
}

impl Config {
    /// Rejects values that would make a component fail to start.
    pub fn validate(&self) -> Result<(), String> {
        let t = &self.tgws;
        if !valid_host(t.host.trim()) {
            return Err("Хост должен быть IP-адресом или доменным именем.".into());
        }
        if t.port == 0 {
            return Err("Порт должен быть числом от 1 до 65535.".into());
        }
        if !valid_secret(&t.secret) {
            return Err("Ключ должен состоять ровно из 32 шестнадцатеричных символов.".into());
        }
        if !t.worker_domain.is_empty() && !valid_domain(&t.worker_domain) {
            return Err(format!("Не похоже на домен: {}. Укажите адрес без https:// и без пути.", t.worker_domain));
        }
        let z = &self.zapret;
        if !["off", "all", "tcp", "udp"].contains(&z.game_filter.as_str()) {
            return Err("Неизвестный режим игрового фильтра.".into());
        }
        if !valid_ports(&z.game_tcp) || !valid_ports(&z.game_udp) {
            return Err("Порты: через запятую отдельные порты или диапазоны «от-до», от 1 до 65535.".into());
        }
        if !["basic", "full", "any"].contains(&z.ipset_mode.as_str()) {
            return Err("Неизвестный режим IPSet.".into());
        }
        Ok(())
    }
}

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(dir: &Path) -> Self {
        let _ = std::fs::create_dir_all(dir);
        Self { path: dir.join("config.json") }
    }

    pub fn load(&self) -> Config {
        for p in [self.path.clone(), self.path.with_extension("json.backup")] {
            if let Ok(bytes) = std::fs::read(&p) {
                if let Ok(mut c) = serde_json::from_slice::<Config>(&bytes) {
                    if !valid_secret(&c.tgws.secret) {
                        c.tgws.secret = crate::win::random_hex(16);
                    }
                    return c;
                }
            }
        }
        let c = Config::default();
        let _ = self.save(&c);
        c
    }

    pub fn save(&self, c: &Config) -> Result<(), String> {
        let data = serde_json::to_vec_pretty(c).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, &data).map_err(|e| format!("Не удалось записать настройки: {e}"))?;
        if self.path.exists() {
            let _ = std::fs::copy(&self.path, self.path.with_extension("json.backup"));
        }
        std::fs::rename(&tmp, &self.path).map_err(|e| format!("Файл настроек занят: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validators() {
        assert!(valid_ports("1024-65535"));
        assert!(valid_ports("1024-1934,1936-65535"));
        assert!(!valid_ports("0-10"));
        assert!(!valid_ports("100-50"));
        assert!(!valid_ports("abc"));
        assert!(valid_domain("name.user.workers.dev"));
        assert!(!valid_domain("https://x.dev"));
        assert!(!valid_domain("1.2.3.4"));
        assert!(valid_host("127.0.0.1"));
        assert!(valid_secret(&crate::win::random_hex(16)));
        assert!(Config::default().validate().is_ok());
    }
}
