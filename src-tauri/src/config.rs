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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DiscordUdp {
    #[default]
    Bundle,
    Discovery,
    Direct,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiscordVerified {
    pub strategy: String,
    pub udp: DiscordUdp,
    pub version: String,
    pub at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ZapretConfig {
    /// File name of the selected strategy (general (ALT).bat …)
    pub strategy: Option<String>,
    pub auto_start: bool,
    pub auto_resolve_conflicts: bool,
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
    pub discord_udp: DiscordUdp,
    pub discord_verified: Option<DiscordVerified>,
    pub managed_sites: Vec<crate::managed_sites::Site>,
    pub managed_services: Vec<String>,
    pub managed_revision: u64,
}

impl Default for ZapretConfig {
    fn default() -> Self {
        Self {
            strategy: None,
            auto_start: false,
            auto_resolve_conflicts: true,
            game_filter: "off".into(),
            game_tcp: "1024-65535".into(),
            game_udp: "1024-65535".into(),
            ipset_mode: "basic".into(),
            fake_discord: None,
            fake_game: None,
            discord_udp: DiscordUdp::Bundle,
            discord_verified: None,
            managed_sites: Vec::new(),
            managed_services: Vec::new(),
            managed_revision: 0,
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
    pub app_auto_check_updates: bool,
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
            app_auto_check_updates: true,
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
        crate::managed_sites::validate(&z.managed_sites, &z.managed_services)?;
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
        let backup = self.path.with_extension("json.backup");
        for path in [&self.path, &backup] {
            if let Ok((config, legacy)) = Self::read(path) {
                if path == &backup {
                    crate::logs::warn("app", "Настройки загружены из резервной копии.");
                }
                if legacy {
                    if let Err(error) = self.save(&config) {
                        crate::logs::warn("app", format!("Не удалось защитить локальный ключ: {error}"));
                    }
                }
                return config;
            }
        }
        let config = Config::default();
        if matches!(self.path.try_exists(), Ok(false)) && matches!(backup.try_exists(), Ok(false)) {
            if let Err(error) = self.save(&config) {
                crate::logs::warn("app", error);
            }
        } else {
            crate::logs::warn("app", "Файлы настроек не читаются. Использованы значения по умолчанию на этот запуск.");
        }
        config
    }

    fn read(path: &Path) -> Result<(Config, bool), String> {
        let mut config: Config = serde_json::from_reader(std::io::BufReader::new(
            std::fs::File::open(path).map_err(|error| error.to_string())?,
        )).map_err(|_| "Не удалось прочитать файл настроек.")?;
        let legacy = match config.tgws.secret.strip_prefix("dpapi:") {
            Some(value) => {
                let bytes = hex::decode(value).map_err(|_| "Защищённый ключ повреждён.")?;
                config.tgws.secret = String::from_utf8(crate::catalog::vault::transform(&bytes, false)?)
                    .map_err(|_| "Защищённый ключ повреждён.")?;
                false
            }
            None => true,
        };
        config.validate()?;
        Ok((config, legacy))
    }

    fn write(path: &Path, config: &Config) -> Result<(), String> {
        use std::io::Write;
        let mut protected = config.clone();
        let key = crate::catalog::vault::transform(config.tgws.secret.as_bytes(), true)?;
        protected.tgws.secret = format!("dpapi:{}", hex::encode(key));
        let data = serde_json::to_vec_pretty(&protected).map_err(|error| error.to_string())?;
        let tmp = path.with_extension("tmp");
        let file_error = |error: std::io::Error| format!("Не удалось сохранить настройки: {error}");
        let mut file = std::fs::File::create(&tmp).map_err(file_error)?;
        file.write_all(&data).and_then(|_| file.sync_all()).map_err(file_error)?;
        drop(file);
        std::fs::rename(tmp, path).map_err(file_error)
    }

    pub fn save(&self, config: &Config) -> Result<(), String> {
        let backup = self.path.with_extension("json.backup");
        if let Ok((previous, _)) = Self::read(&self.path).or_else(|_| Self::read(&backup)) {
            Self::write(&backup, &previous)?;
        }
        Self::write(&self.path, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_backup_recovers_legacy_settings() {
        let dir = std::env::temp_dir().join(format!("veliora-config-{}", crate::win::random_hex(8)));
        let store = Store::new(&dir);
        let config = Config::default();
        std::fs::write(&store.path, serde_json::to_vec(&config).unwrap()).unwrap();
        assert_eq!(store.load(), config);
        for path in [&store.path, &store.path.with_extension("json.backup")] {
            let contents = std::fs::read_to_string(path).unwrap();
            assert!(contents.contains("dpapi:"));
            assert!(!contents.contains(&config.tgws.secret));
        }
        std::fs::write(&store.path, "corrupt").unwrap();
        assert_eq!(store.load(), config);
        std::fs::remove_dir_all(&dir).unwrap();
    }

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
