use super::feed::{public_ip, Endpoint};
use libloading::os::windows::Library;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    ffi::{c_char, CStr, CString},
    net::{IpAddr, SocketAddr, TcpStream},
    path::Path,
    time::{Duration, Instant},
};

type SendJson = unsafe extern "C" fn(i32, *const c_char);
type ReceiveJson = unsafe extern "C" fn(f64) -> *const c_char;

pub struct Tester {
    _library: std::mem::ManuallyDrop<Library>,
    client: i32,
    send: SendJson,
    receive: ReceiveJson,
    sequence: u64,
}
#[derive(Clone, serde::Serialize)]
pub struct Sample {
    pub at: i64,
    pub result: String,
    pub tcp: Option<i64>,
    pub elapsed: Option<i64>,
    #[serde(skip)]
    pub ip: Option<IpAddr>,
}
impl Sample {
    fn failure(code: &str, tcp: Option<i64>) -> Self {
        Self {
            at: super::now(),
            result: code.into(),
            tcp,
            elapsed: None,
            ip: None,
        }
    }
}
impl Tester {
    pub fn load(path: &Path) -> Result<Self, String> {
        // This DLL and its schema are pinned together in package-lock.json.
        let bytes = std::fs::read(path).map_err(|_| "Библиотека TDLib отсутствует в сборке.")?;
        if hex::encode(Sha256::digest(&bytes))
            != "3eb3619ce74e889d3a0e5c0c68aaa0457c5a52c538f8777b73b933744ff4f4ec"
        {
            return Err("Не совпадает контрольная сумма TDLib.".into());
        }
        unsafe {
            // Load dependencies only from the DLL's own directory and System32.
            let library = Library::load_with_flags(path, 0x100 | 0x800)
                .map_err(|_| "Не удалось запустить проверку серверов. Переустановите VELIORA из полного установщика Windows x64.")?;
            let sym = "Версия TDLib несовместима.";
            let execute = *library
                .get::<unsafe extern "C" fn(*const c_char) -> *const c_char>(b"td_execute\0")
                .map_err(|_| sym)?;
            execute(c"{\"@type\":\"setLogVerbosityLevel\",\"new_verbosity_level\":0}".as_ptr());
            let create = *library
                .get::<unsafe extern "C" fn() -> i32>(b"td_create_client_id\0")
                .map_err(|_| sym)?;
            let send = *library.get::<SendJson>(b"td_send\0").map_err(|_| sym)?;
            let receive = *library
                .get::<ReceiveJson>(b"td_receive\0")
                .map_err(|_| sym)?;
            Ok(Self {
                _library: std::mem::ManuallyDrop::new(library),
                client: create(),
                send,
                receive,
                sequence: 0,
            })
        }
    }
    pub fn check(&mut self, endpoint: &Endpoint, through_local: bool) -> Sample {
        let dns = tauri::async_runtime::block_on(async {
            tokio::time::timeout(
                Duration::from_secs(3),
                tokio::net::lookup_host((endpoint.host.as_str(), endpoint.port)),
            )
            .await
            .ok()?
            .ok()
            .map(|it| it.collect::<Vec<_>>())
        });
        let Some(mut addresses) = dns else {
            return Sample::failure("dns", None);
        };
        if addresses.is_empty() || addresses.iter().any(|a| !public_ip(a.ip())) {
            return Sample::failure("address", None);
        }
        addresses.sort_by_key(|a| !a.is_ipv4());
        addresses.dedup();
        let mut last = "tcp";
        let mut connected = None;
        for address in addresses.into_iter().take(3) {
            let start = Instant::now();
            match TcpStream::connect_timeout(&address, Duration::from_secs(2)) {
                Ok(socket) => {
                    drop(socket);
                    connected = Some((address, start.elapsed().as_millis() as i64));
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => last = "timeout",
                Err(_) => (),
            }
        }
        let Some((address, tcp)) = connected else {
            return Sample::failure(last, None);
        };
        if !through_local {
            return self.handshake(address, &endpoint.secret, tcp);
        }
        // A machine can route local forwarding differently from a direct probe.
        // Validate the path Telegram will actually use before committing the selection.
        let Ok(relay) = tauri::async_runtime::block_on(super::relay::Relay::start(0, address))
        else {
            return Sample::failure("local", Some(tcp));
        };
        let mut sample = self.handshake(([127, 0, 0, 1], relay.port).into(), &endpoint.secret, tcp);
        tauri::async_runtime::block_on(relay.close());
        if sample.result == "ok" {
            sample.ip = Some(address.ip());
        }
        sample
    }
    pub(super) fn handshake(&mut self, address: SocketAddr, secret: &str, tcp: i64) -> Sample {
        self.sequence += 1;
        let query = json!({"@type":"testProxy","proxy":{"@type":"proxy","server":address.ip().to_string(),"port":address.port(),
            "type":{"@type":"proxyTypeMtproto","secret":secret}},"dc_id":2,"timeout":6.0,"@extra":self.sequence});
        let text = CString::new(query.to_string()).expect("JSON escapes NUL");
        let start = Instant::now();
        unsafe { (self.send)(self.client, text.as_ptr()) };
        while start.elapsed() < Duration::from_secs(8) {
            let p = unsafe { (self.receive)(0.25) };
            if p.is_null() {
                continue;
            }
            let Ok(v) = serde_json::from_slice::<Value>(unsafe { CStr::from_ptr(p) }.to_bytes())
            else {
                continue;
            };
            if v["@extra"].as_u64() != Some(self.sequence)
                || v["@client_id"].as_i64() != Some(self.client as i64)
            {
                continue;
            }
            if v["@type"] == "ok" {
                return Sample {
                    at: super::now(),
                    result: "ok".into(),
                    tcp: Some(tcp),
                    elapsed: Some(start.elapsed().as_millis() as i64),
                    ip: Some(address.ip()),
                };
            }
            let timeout = v["code"] == 408
                || v["message"]
                    .as_str()
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .contains("timeout");
            return Sample::failure(if timeout { "timeout" } else { "protocol" }, Some(tcp));
        }
        Sample::failure("timeout", Some(tcp))
    }
}
