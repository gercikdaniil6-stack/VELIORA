use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub domain: String,
    pub enabled: bool,
    pub strategy: Option<String>,
}

pub const HOST_FILE: &str = "list-veliora-managed.txt";
pub const PRESETS: &[(&str, &[&str])] = &[
    ("openai", &["chatgpt.com", "openai.com", "oaistatic.com", "oaiusercontent.com"]),
    ("anthropic", &["claude.ai", "anthropic.com"]),
    ("gemini", &["gemini.google.com", "aistudio.google.com", "generativelanguage.googleapis.com"]),
    ("grok", &["grok.com", "x.ai"]),
    ("telegram", &["telegram.org", "t.me"]),
    ("soundcloud", &["soundcloud.com", "sndcdn.com"]),
    ("twitch", &["twitch.tv", "ttvnw.net", "jtvnw.net"]),
    ("tiktok", &["tiktok.com", "tiktokcdn.com"]),
    ("github", &["github.com", "githubusercontent.com", "githubassets.com"]),
];

pub fn domain(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.len() > 2048 || raw.contains(['\\', '\0']) {
        return None;
    }
    let normalized;
    let input = if raw.contains("://") { raw } else {
        normalized = format!("https://{raw}");
        &normalized
    };
    let url = reqwest::Url::parse(input).ok()?;
    if !matches!(url.scheme(), "http" | "https") || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let domain = url.domain()?.trim_end_matches('.').to_ascii_lowercase();
    crate::config::valid_domain(&domain).then_some(domain)
}

pub fn matches_domain(domain: &str, host: &str) -> bool {
    domain == host || domain.strip_suffix(host).is_some_and(|prefix| prefix.ends_with('.'))
}

pub fn preset(domain: &str) -> Option<&'static str> {
    PRESETS.iter().flat_map(|(id, names)| names.iter().map(move |host| (*id, *host)))
        .filter(|(_, host)| matches_domain(domain, host))
        .max_by_key(|(_, host)| host.len())
        .map(|(id, _)| id)
}

pub fn single_domain(input: &str) -> Result<String, String> {
    let text = input.trim();
    if text.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("Вставьте одну ссылку на сайт.".into());
    }
    domain(text).ok_or_else(|| "Не удалось распознать сайт. Пример: https://example.com".into())
}

/// Check a public HTTPS endpoint without forwarding the user's URL, credentials,
/// proxy settings or redirects. Use the same addresses that were validated.
pub async fn available(host: &str) -> bool {
    use std::time::Duration;
    let resolved = tokio::time::timeout(Duration::from_secs(4), tokio::net::lookup_host((host, 443))).await;
    let Ok(Ok(addresses)) = resolved else { return false; };
    let addresses: Vec<_> = addresses.take(33).collect();
    if addresses.is_empty() || addresses.len() > 32 || addresses.iter().any(|ip| !crate::catalog::public_ip(ip.ip())) {
        return false;
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(host, &addresses)
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(8))
        .build();
    let Ok(client) = client else { return false; };
    client.get(format!("https://{host}/")).send().await
        .is_ok_and(|response| response.status().is_success())
}

pub fn validate(sites: &[Site], presets: &[String]) -> Result<(), String> {
    if sites.len() > 5000 {
        return Err("Можно сохранить до 5000 сайтов.".into());
    }
    let mut seen = BTreeSet::new();
    for site in sites {
        if site.name.as_ref().is_some_and(|name| name.chars().count() > 80 || name.chars().any(char::is_control)) {
            return Err("Название сайта должно содержать до 80 символов без переносов строк.".into());
        }
        if domain(&site.domain).as_deref() != Some(site.domain.as_str()) || !seen.insert(&site.domain) {
            return Err("Список содержит некорректные или повторяющиеся домены.".into());
        }
        if site.strategy.as_ref().is_some_and(|s| s.is_empty() || s.len() > 200 || !s.ends_with(".bat") || s.contains(['/', '\\', ':']) || s.contains("..")) {
            return Err("Некорректное имя стратегии сайта.".into());
        }
    }
    let mut seen = BTreeSet::new();
    for id in presets {
        if !seen.insert(id) || !PRESETS.iter().any(|(key, _)| *key == id) {
            return Err("Неизвестный или повторяющийся сервис.".into());
        }
    }
    Ok(())
}

pub fn domains<'a>(sites: &'a [Site], presets: &[String], strategy: &str) -> BTreeSet<&'a str> {
    let mut domains = BTreeSet::new();
    for (id, names) in PRESETS {
        if presets.iter().any(|p| p == id) { domains.extend(names.iter().copied()); }
    }
    domains.extend(sites.iter().filter(|s| s.enabled && s.strategy.as_deref().is_none_or(|name| name == strategy)).map(|s| s.domain.as_str()));
    domains
}

pub fn write(dir: &Path, domains: &BTreeSet<&str>) -> Result<(), String> {
    let path = dir.join("lists").join(HOST_FILE);
    let temp = path.with_extension("new");
    let body = domains.iter().copied().collect::<Vec<_>>().join("\r\n") + "\r\n";
    std::fs::write(&temp, body).and_then(|_| std::fs::rename(&temp, &path))
        .map_err(|e| format!("Не удалось записать список VELIORA: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_web_urls_and_rejects_credentials_ips_and_schemes() {
        assert_eq!(domain("https://EXAMPLE.com.:443/blog").as_deref(), Some("example.com"));
        assert_eq!(domain("пример.рф").as_deref(), Some("xn--e1afmkfd.xn--p1ai"));
        for input in ["ftp://example.com", "user@example.com", "https://127.0.0.1", "^example.com", "example"] {
            assert_eq!(domain(input), None, "{input}");
        }
    }

    #[test]
    fn reads_existing_sites_without_a_name_and_validates_new_titles() {
        let mut site: Site = serde_json::from_str(r#"{"domain":"example.com","enabled":true,"strategy":null}"#).unwrap();
        assert_eq!(site.name, None);
        assert!(validate(std::slice::from_ref(&site), &[]).is_ok());
        site.name = Some("Мой сайт".into());
        let data = serde_json::to_string(&site).unwrap();
        assert_eq!(serde_json::from_str::<Site>(&data).unwrap(), site);
        site.name = Some("Мой\nсайт".into());
        assert!(validate(&[site], &[]).is_err());
    }

    #[test]
    fn matches_only_profile_domains_and_accepts_one_url() {
        assert_eq!(preset("www.chatgpt.com"), Some("openai"));
        assert_eq!(preset("media.sndcdn.com"), Some("soundcloud"));
        assert_eq!(preset("notgithub.com"), None);
        assert_eq!(preset("github.com.example.org"), None);
        assert!(single_domain("example.org\nexample.net").is_err());
        assert_eq!(single_domain("https://Example.org/path?token=abc").unwrap(), "example.org");
    }

    #[test]
    fn filters_disabled_and_other_strategy_sites() {
        let sites = vec![
            Site { name: None, domain: "example.com".into(), enabled: true, strategy: Some("general.bat".into()) },
            Site { name: None, domain: "example.org".into(), enabled: false, strategy: None },
        ];
        assert_eq!(domains(&sites, &["github".into()], "general.bat").len(), 4);
        assert!(domains(&sites, &[], "other.bat").is_empty());
        assert!(validate(&[sites[0].clone(), sites[0].clone()], &[]).is_err());
    }
}
