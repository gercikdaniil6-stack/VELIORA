use base64::{engine::general_purpose, Engine};
use ip_network::IpNetwork;
use scraper::{Html, Selector};
use std::{collections::HashSet, io::Read, net::IpAddr, time::Duration};
use url::Url;

#[derive(Clone, Copy)]
pub struct Source {
    pub name: &'static str,
    pub url: &'static str,
    pub backoff: &'static str,
    pub telegram: bool,
}
pub const SOURCES: [Source; 3] = [
    Source {
        name: "Telegram",
        url: "https://t.me/s/ProxyMTProto",
        backoff: "backoff",
        telegram: true,
    },
    Source {
        name: "MTProxy list",
        url: "https://iwizard7.github.io/MTProxy_list/working.txt",
        backoff: "backoff_pages",
        telegram: false,
    },
    Source {
        name: "SoliSpirit",
        url: "https://raw.githubusercontent.com/SoliSpirit/mtproto/master/all_proxies.txt",
        backoff: "backoff_raw",
        telegram: false,
    },
];

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub secret: String,
}

pub fn public_ip(ip: IpAddr) -> bool {
    if !IpNetwork::from(ip).is_global() || ip.is_multicast() || ip.is_unspecified() {
        return false;
    }
    match ip {
        IpAddr::V4(v) => v.octets()[0] < 224 && v.octets()[0] != 0,
        IpAddr::V6(v) => {
            (0x2000..0x4000).contains(&v.segments()[0])
                && v.segments()[0] != 0x2002
                && !(v.segments()[0] == 0x2001 && v.segments()[1] < 0x200)
        }
    }
}

fn domain(s: &str) -> bool {
    s.len() <= 253
        && s.contains('.')
        && s.rsplit('.')
            .next()
            .is_some_and(|l| l.bytes().any(|c| c.is_ascii_alphabetic()))
        && ![
            ".local",
            ".localhost",
            ".internal",
            ".lan",
            ".home.arpa",
            ".test",
            ".invalid",
            ".onion",
        ]
        .iter()
        .any(|x| s.ends_with(x))
        && s.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
}

fn secret(raw: &str) -> Option<String> {
    if raw.len() > 550 || raw.is_empty() {
        return None;
    }
    let bytes =
        if raw.len() % 2 == 0 && raw.len() >= 32 && raw.bytes().all(|c| c.is_ascii_hexdigit()) {
            hex::decode(raw).ok()?
        } else {
            let s = raw.trim_end_matches('=');
            general_purpose::URL_SAFE_NO_PAD
                .decode(s)
                .or_else(|_| general_purpose::STANDARD_NO_PAD.decode(s))
                .ok()?
        };
    match bytes.as_slice() {
        b if b.len() == 16 => (),
        [0xdd, rest @ ..] if rest.len() == 16 => (),
        [0xee, rest @ ..]
            if rest.len() > 16 && std::str::from_utf8(&rest[16..]).ok().is_some_and(domain) =>
        {
            ()
        }
        _ => return None,
    }
    Some(hex::encode(bytes))
}

pub fn parse_link(s: &str) -> Option<Endpoint> {
    if s.len() > 1500 || s.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    let u = Url::parse(s).ok()?;
    let valid = match (u.scheme(), u.host_str(), u.path()) {
        ("tg", Some("proxy"), "" | "/") | ("https", Some("t.me"), "/proxy") => true,
        _ => false,
    };
    if !valid
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
        || u.fragment().is_some()
    {
        return None;
    }
    let (mut host, mut port, mut key) = (None, None, None);
    for (k, v) in u.query_pairs() {
        match k.as_ref() {
            "server" if host.is_none() => {
                let value = v.strip_suffix('.').unwrap_or(&v).to_ascii_lowercase();
                let bare = value
                    .strip_prefix('[')
                    .and_then(|s| s.strip_suffix(']'))
                    .unwrap_or(&value);
                let h = match url::Host::parse(bare).ok() {
                    Some(url::Host::Ipv4(ip)) if public_ip(ip.into()) => ip.to_string(),
                    Some(url::Host::Domain(d)) if d == bare && domain(&d) => d,
                    _ => {
                        let ip: IpAddr = bare.parse().ok()?;
                        if !public_ip(ip) {
                            return None;
                        }
                        ip.to_string()
                    }
                };
                host = Some(h);
            }
            "port" if port.is_none() && v.bytes().all(|c| c.is_ascii_digit()) => {
                port = Some(v.parse::<u16>().ok()?);
            }
            "secret" if key.is_none() => {
                key = Some(secret(&v)?);
            }
            "server" | "port" | "secret" => return None,
            _ => (),
        }
    }
    Some(Endpoint {
        host: host?,
        port: port.filter(|p| *p > 0)?,
        secret: key?,
    })
}

pub struct Post {
    pub id: i64,
    pub date: String,
    pub proxies: HashSet<Endpoint>,
}
pub struct Page {
    pub posts: Vec<Post>,
    pub before: Option<i64>,
    pub links: usize,
}
impl Page {
    pub fn valid(&self) -> usize {
        self.posts
            .iter()
            .flat_map(|post| &post.proxies)
            .collect::<HashSet<_>>()
            .len()
    }
}

pub fn parse_page(s: &str) -> Result<Page, String> {
    let html = Html::parse_document(s);
    let sel = |s| Selector::parse(s).expect("static selector");
    if html.select(&sel(".tgme_channel_history")).next().is_none() {
        return Err("Источник каталога недоступен.".into());
    }
    let links = sel("a[href]");
    let text = sel(".tgme_widget_message_text");
    let date = sel("time[datetime]");
    let mut posts = Vec::new();
    let mut found = 0;
    for msg in html.select(&sel("[data-post]")) {
        let Some(id) = msg
            .attr("data-post")
            .and_then(|s| s.split_once('/'))
            .filter(|(channel, _)| channel.eq_ignore_ascii_case("ProxyMTProto"))
            .map(|(_, id)| id)
            .and_then(|s| s.parse::<i64>().ok())
            .filter(|v| *v > 0)
        else {
            continue;
        };
        let stamp = msg
            .select(&date)
            .next()
            .and_then(|e| e.attr("datetime"))
            .unwrap_or("");
        let mut proxies = HashSet::new();
        for candidate in msg.select(&links).filter_map(|a| a.attr("href")).chain(
            msg.select(&text)
                .flat_map(|node| node.text())
                .flat_map(str::split_whitespace),
        ) {
            collect(candidate, &mut found, &mut proxies);
        }
        posts.push(Post {
            id,
            date: stamp.to_string(),
            proxies,
        });
    }
    let before = html
        .select(&sel("a[data-before]"))
        .next()
        .and_then(|e| e.attr("data-before")?.parse().ok());
    Ok(Page {
        posts,
        before,
        links: found,
    })
}

fn collect(token: &str, found: &mut usize, proxies: &mut HashSet<Endpoint>) {
    let token =
        token.trim_matches(|c: char| matches!(c, '<' | '>' | '(' | ')' | '"' | ',' | '.' | ';'));
    if token.starts_with("tg://proxy?") || token.starts_with("https://t.me/proxy?") {
        *found += 1;
        if let Some(proxy) = parse_link(token) {
            proxies.insert(proxy);
        }
    }
}
fn parse_list(body: &str) -> Page {
    let mut proxies = HashSet::new();
    let mut links = 0;
    for line in body.lines().take(5_000) {
        collect(
            line.trim().trim_start_matches('\u{feff}'),
            &mut links,
            &mut proxies,
        );
    }
    Page {
        posts: vec![Post {
            id: 0,
            date: String::new(),
            proxies,
        }],
        before: None,
        links,
    }
}
#[derive(Debug)]
pub struct FetchError {
    pub status: Option<u16>,
    pub message: &'static str,
    pub delay: u64,
    pub retry_after: u64,
    pub links: usize,
}

pub fn client(direct: bool) -> Result<reqwest::blocking::Client, String> {
    let builder = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(7))
        .timeout(Duration::from_secs(14))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("VELIORA/", env!("CARGO_PKG_VERSION")));
    let builder = if direct { builder.no_proxy() } else { builder };
    builder
        .build()
        .map_err(|_| "Не удалось создать HTTPS-клиент.".into())
}
fn network_error(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        return "Истекло время ожидания. Проверьте сеть и системный прокси.";
    }
    let mut cause = std::error::Error::source(e);
    while let Some(error) = cause {
        let message = error.to_string().to_ascii_lowercase();
        if message.contains("dns") || message.contains("11001") || message.contains("11002") {
            return "Ошибка DNS. Проверьте настройки DNS или разрешение имён в системном прокси.";
        }
        if message.contains("tunnel") || message.contains("proxy authentication") {
            return "Прокси не открыл HTTPS-туннель. Проверьте системный прокси.";
        }
        if message.contains("certificate")
            || message.contains("tls")
            || message.contains("schannel")
        {
            return "Ошибка защищённого соединения. Проверьте время Windows и доверенные сертификаты.";
        }
        cause = error.source();
    }
    if e.is_connect() {
        "Не удалось установить соединение. Проверьте доступ в Интернет и системный прокси."
    } else {
        "Не удалось выполнить сетевой запрос. Проверьте подключение."
    }
}
fn response_error(status: u16, retry: Option<&str>) -> FetchError {
    let delay = retry.and_then(|s| {
        s.trim().parse::<u64>().ok().or_else(|| {
            let when = httpdate::parse_http_date(s).ok()?;
            Some(
                when.duration_since(std::time::SystemTime::now())
                    .unwrap_or_default()
                    .as_secs(),
            )
        })
    });
    let (message, default) = match status {
        429 => (
            "Источник ограничил частоту запросов. Дождитесь автоматической попытки.",
            900,
        ),
        403 => (
            "Источник отказал в доступе. Проверьте доступ к нему в браузере.",
            1800,
        ),
        407 => (
            "Системный прокси требует авторизацию. Проверьте настройки системного прокси.",
            600,
        ),
        300..=399 => (
            "Неожиданное перенаправление. Проверьте, нужна ли авторизация в вашей сети.",
            600,
        ),
        600..=999 => (
            "Получен нестандартный HTTP-статус. По самому статусу нельзя определить отправителя.",
            600,
        ),
        500..=599 => ("Источник временно недоступен. Повторим автоматически.", 300),
        _ => (
            "Источник вернул ошибку HTTP. Проверьте доступ к нему в браузере.",
            600,
        ),
    };
    FetchError {
        status: Some(status),
        message,
        delay: default,
        retry_after: delay.unwrap_or(0),
        links: 0,
    }
}

pub fn fetch(
    client: &reqwest::blocking::Client,
    source: Source,
    cursor: Option<(&str, i64)>,
) -> Result<Page, FetchError> {
    let mut q = client.get(source.url);
    if let Some((key, id)) = cursor {
        q = q.query(&[(key, id)]);
    }
    let res = q.send().map_err(|error| FetchError {
        status: None,
        message: network_error(&error),
        delay: 120,
        retry_after: 0,
        links: 0,
    })?;
    let status = res.status().as_u16();
    if status != 200 {
        return Err(response_error(
            status,
            res.headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok()),
        ));
    }
    let invalid = |message, links| FetchError {
        status: Some(status),
        message,
        delay: 600,
        retry_after: 0,
        links,
    };
    let mut bytes = Vec::new();
    if let Err(error) = res.take(2_000_001).read_to_end(&mut bytes) {
        let message = if matches!(
            error.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        ) || error
            .get_ref()
            .and_then(|e| e.downcast_ref::<reqwest::Error>())
            .is_some_and(reqwest::Error::is_timeout)
        {
            "Истекло время загрузки ответа. Проверьте скорость подключения."
        } else {
            "Ответ источника оборвался. Повторим автоматически."
        };
        return Err(invalid(message, 0));
    }
    if bytes.len() > 2_000_000 {
        return Err(invalid("Источник вернул слишком большой ответ.", 0));
    }
    let body = std::str::from_utf8(&bytes)
        .map_err(|_| invalid("Кодировка ответа источника изменилась.", 0))?;
    let page = if source.telegram {
        parse_page(body).map_err(|_| invalid("Изменилась структура страницы или источник вернул страницу входа. Используем резерв при доступности.", 0))?
    } else {
        parse_list(body)
    };
    if page.valid() == 0 && (cursor.is_none() || page.links != 0) {
        return Err(invalid(
            "Ответ получен, но действительных ссылок нет. Повторим автоматически.",
            page.links,
        ));
    }
    Ok(page)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn http_and_payload_failures_keep_their_reason() {
        use std::io::Write;
        fn request(status: &str, headers: &str, body: &str) -> Result<Page, FetchError> {
            let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let response = format!("HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let worker = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                let _ = socket.read(&mut [0_u8; 4096]);
                socket.write_all(response.as_bytes()).unwrap();
            });
            let client = reqwest::blocking::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap();
            let result = fetch(
                &client,
                Source {
                    url: Box::leak(url.into_boxed_str()),
                    ..SOURCES[0]
                },
                None,
            );
            worker.join().unwrap();
            result
        }
        let forbidden = request("403 Forbidden", "", "").err().unwrap();
        assert_eq!(forbidden.status, Some(403));
        assert!(forbidden.message.contains("отказал"));
        let limited = request("429 Too Many Requests", "Retry-After: 137\r\n", "")
            .err()
            .unwrap();
        assert_eq!(limited.status, Some(429));
        assert_eq!(limited.retry_after, 137);
        let actual_603 = request("603 Example", "", "").err().unwrap();
        assert_eq!(actual_603.status, Some(603));
        assert!(actual_603.message.contains("нестандартный"));
        let html = request("200 OK", "", "<h1>Sign in</h1>").err().unwrap();
        assert!(html.message.contains("структура"));
        assert_eq!(html.status, Some(200));
    }

    #[test]
    #[ignore = "downloads the three public feeds"]
    fn live_feeds() {
        let http = client(false).unwrap();
        for source in SOURCES {
            let page =
                fetch(&http, source, None).unwrap_or_else(|e| panic!("{}: {e:?}", source.name));
            eprintln!(
                "{} HTTP 200: {} links, {} valid",
                source.name,
                page.links,
                page.valid()
            );
            assert!(page.valid() > 0);
        }
    }

    #[test]
    fn canonical_and_hostile_links() {
        let key = "00112233445566778899aabbccddeeff";
        let link = |h: &str| format!("tg://proxy?server={h}&port=443&secret={key}");
        for host in [
            "127.0.0.1",
            "169.254.169.254",
            "0x7f000001",
            "10.0.0.1",
            "[::1]",
            "printer.local",
            "host%0d",
        ] {
            assert!(parse_link(&link(host)).is_none(), "{host}");
        }
        let good = link("Example.org.");
        let e = parse_link(&good).unwrap();
        assert_eq!(e.host, "example.org");
        assert_eq!(
            e,
            parse_link(&good.replace("tg://proxy", "https://t.me/proxy")).unwrap()
        );
        for bad in [
            good.clone() + "&port=2",
            good.replace("port=443", "port=0"),
            good.replace(key, "xyz"),
            good.replace("tg://proxy", "https://t.me.evil.org/proxy"),
        ] {
            assert!(parse_link(&bad).is_none());
        }
    }
    #[test]
    fn equivalent_encodings_and_tls_secrets() {
        for bytes in [
            vec![0xab; 16],
            [vec![0xdd], vec![0xab; 16]].concat(),
            [vec![0xee], vec![0xab; 16], b"example.org".to_vec()].concat(),
        ] {
            assert_eq!(
                secret(&hex::encode(&bytes)),
                secret(&general_purpose::URL_SAFE_NO_PAD.encode(&bytes))
            );
        }
        assert!(secret(&hex::encode(
            [vec![0xee], vec![0xab; 16], b"bad/path".to_vec()].concat()
        ))
        .is_none());
    }

    #[test]
    fn message_dedup_and_multiple() {
        let link="tg://proxy?server=one.example.org&amp;port=443&amp;secret=00112233445566778899aabbccddeeff";
        let h = format!(
            r#"<section class="tgme_channel_history"><div class="tgme_widget_message" data-post="ProxyMTProto/13"><a class="tgme_widget_message_date"><time datetime="2026-01-01"></time></a><div class="tgme_widget_message_text"><a href="{link}">1</a><a href="{link}">2</a><a href="{}">3</a></div></div><a data-before="13"></a></section>"#,
            link.replace("one.", "two.")
        );
        let page = parse_page(&h).unwrap();
        assert_eq!(page.posts[0].proxies.len(), 2);
        assert_eq!(page.before, Some(13));
    }
}
