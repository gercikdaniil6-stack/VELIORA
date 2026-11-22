use super::{
    feed::{Endpoint, Page},
    now,
    probe::Sample,
    vault,
};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub type R<T> = Result<T, String>;
pub trait DbResult<T> {
    fn db(self) -> R<T>;
}
impl<T> DbResult<T> for rusqlite::Result<T> {
    fn db(self) -> R<T> {
        self.map_err(|e| format!("Ошибка локального каталога: {e}"))
    }
}

pub fn init(path: &Path) -> R<Connection> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Не удалось создать папку каталога: {error}"))?;
    }
    let c = Connection::open(path).db()?;
    c.busy_timeout(std::time::Duration::from_secs(3)).db()?;
    c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
    CREATE TABLE IF NOT EXISTS pref(k TEXT PRIMARY KEY,v INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS source_report(source TEXT PRIMARY KEY,report TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS endpoint(
      id INTEGER PRIMARY KEY, digest BLOB UNIQUE NOT NULL,host TEXT NOT NULL,port INTEGER NOT NULL,key BLOB NOT NULL,
      post INTEGER NOT NULL,date TEXT NOT NULL,fav INTEGER NOT NULL DEFAULT 0,at INTEGER NOT NULL DEFAULT 0,
      result TEXT NOT NULL DEFAULT 'new', tcp INTEGER,elapsed INTEGER,success INTEGER NOT NULL DEFAULT 0,
      ip TEXT,score REAL NOT NULL DEFAULT 0,checks INTEGER NOT NULL DEFAULT 0,oks INTEGER NOT NULL DEFAULT 0,
      streak INTEGER NOT NULL DEFAULT 0,due INTEGER NOT NULL DEFAULT 0);
    CREATE INDEX IF NOT EXISTS endpoint_post ON endpoint(post);
    CREATE INDEX IF NOT EXISTS endpoint_due ON endpoint(due);
    CREATE INDEX IF NOT EXISTS endpoint_fresh ON endpoint(result,at);
    CREATE TABLE IF NOT EXISTS sample(id INTEGER NOT NULL REFERENCES endpoint(id),at INTEGER NOT NULL,result TEXT NOT NULL,tcp INTEGER,elapsed INTEGER);
    CREATE INDEX IF NOT EXISTS sample_id ON sample(id,at DESC);").db()?;
    let has_seen: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('endpoint') WHERE name='seen')",
            [],
            |row| row.get(0),
        )
        .db()?;
    if !has_seen {
        c.execute(
            "ALTER TABLE endpoint ADD COLUMN seen INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .db()?;
    }
    let has_verified: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('endpoint') WHERE name='verified')",
            [],
            |r| r.get(0),
        )
        .db()?;
    if !has_verified {
        c.execute(
            "ALTER TABLE endpoint ADD COLUMN verified INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .db()?;
    }
    Ok(c)
}
pub fn pref(c: &Connection, k: &str, default: i64) -> R<i64> {
    Ok(
        c.query_row("SELECT v FROM pref WHERE k=?", [k], |r| r.get(0))
            .optional()
            .db()?
            .unwrap_or(default),
    )
}
pub fn set(c: &Connection, k: &str, v: i64) -> R<()> {
    c.execute(
        "INSERT INTO pref VALUES(?1,?2) ON CONFLICT(k) DO UPDATE SET v=excluded.v",
        params![k, v],
    )
    .db()?;
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceReport {
    pub name: String,
    pub attempted_at: i64,
    pub next_at: i64,
    pub duration_ms: u64,
    pub direct: bool,
    pub http_status: Option<u16>,
    pub links: usize,
    pub valid: usize,
    pub failures: u32,
    pub reason: Option<String>,
}
pub fn source_report(c: &Connection, key: &str) -> R<Option<SourceReport>> {
    let value: Option<String> = c
        .query_row(
            "SELECT report FROM source_report WHERE source=?",
            [key],
            |r| r.get(0),
        )
        .optional()
        .db()?;
    value
        .map(|value| {
            serde_json::from_str(&value)
                .map_err(|_| "Не удалось прочитать диагностику источника.".to_string())
        })
        .transpose()
}
pub fn store_source_report(c: &mut Connection, key: &str, report: &SourceReport) -> R<()> {
    let tx = c.transaction().db()?;
    let value =
        serde_json::to_string(report).map_err(|_| "Не удалось сохранить диагностику источника.")?;
    tx.execute("INSERT INTO source_report VALUES(?,?) ON CONFLICT(source) DO UPDATE SET report=excluded.report", params![key,value]).db()?;
    set(&tx, key, report.next_at)?;
    tx.commit().db()
}
pub fn new_verification(c: &Connection) -> R<()> {
    c.execute(
        "UPDATE endpoint SET verified=0,due=min(due,?1) WHERE result='ok'",
        [now()],
    )
    .db()?;
    Ok(())
}
pub fn ingest(c: &mut Connection, page: &Page, history: bool, telegram: bool) -> R<()> {
    let tx = c.transaction().db()?;
    let seen = if history { 0 } else { now() };
    for post in &page.posts {
        for e in &post.proxies {
            let digest =
                Sha256::digest(format!("{}|{}|{}", e.host, e.port, e.secret).as_bytes()).to_vec();
            let changed=tx.execute("UPDATE endpoint SET date=CASE WHEN post<?2 THEN ?3 ELSE date END,post=max(post,?2),seen=max(seen,?4) WHERE digest=?1",
                                 params![digest,post.id,post.date,seen]).db()?;
            if changed == 0 {
                tx.execute("INSERT INTO endpoint(digest,host,port,key,post,date,due,seen) VALUES(?,?,?,?,?,?,?,?)",
            params![digest,e.host,e.port,vault::transform(e.secret.as_bytes(),true)?,post.id,post.date,now(),seen]).db()?;
            }
        }
    }
    let latest = pref(&tx, "latest", 0)?;
    let gap = !history
        && page
            .posts
            .iter()
            .map(|post| post.id)
            .min()
            .is_some_and(|oldest| oldest > latest.saturating_add(1));
    if telegram && (history || latest == 0 || gap) {
        let prior = if history {
            pref(&tx, "before", i64::MAX)?
        } else {
            i64::MAX
        };
        let before = page.before.filter(|v| *v > 0 && *v < prior).unwrap_or(0);
        set(&tx, "before", before)?;
    }
    if !history {
        if let Some(id) = page
            .posts
            .iter()
            .map(|p| p.id)
            .filter(|id| telegram && *id > 0)
            .max()
        {
            set(&tx, "latest", pref(&tx, "latest", 0)?.max(id))?;
        }
        set(&tx, "sync", now())?;
    }
    tx.commit().db()
}
pub fn endpoint(c: &Connection, id: i64) -> R<Endpoint> {
    let (host, port, key): (String, u16, Vec<u8>) = c
        .query_row("SELECT host,port,key FROM endpoint WHERE id=?", [id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .db()?;
    Ok(Endpoint {
        host,
        port,
        secret: String::from_utf8(vault::transform(&key, false)?).map_err(|_| "Повреждён ключ.")?,
    })
}

// A small prior stops one lucky response from outranking a well-tested server.
// Penalise consecutive failures, recent failures and handshake-time variation.
fn quality(samples: &[(bool, i64)]) -> f64 {
    if samples.is_empty() {
        return 0.;
    }
    let ok: Vec<f64> = samples.iter().filter(|s| s.0).map(|s| s.1 as f64).collect();
    if ok.is_empty() {
        return 0.;
    }
    let mean = ok.iter().sum::<f64>() / ok.len() as f64;
    let jitter = ok.iter().map(|t| (t - mean).abs()).sum::<f64>() / ok.len() as f64;
    let recent = samples.iter().take(4).filter(|s| !s.0).count() as f64;
    let ratio = (ok.len() as f64 + 1.) / (samples.len() as f64 + 2.);
    75. * ratio + 18. / (1. + mean / 700.) + 7. / (1. + jitter / 180.) - recent * 3.
}
pub fn record(c: &mut Connection, id: i64, s: &Sample) -> R<()> {
    let tx = c.transaction().db()?;
    tx.execute(
        "INSERT INTO sample VALUES(?,?,?,?,?)",
        params![id, s.at, s.result, s.tcp, s.elapsed],
    )
    .db()?;
    tx.execute("DELETE FROM sample WHERE id=?1 AND rowid NOT IN(SELECT rowid FROM sample WHERE id=?1 ORDER BY at DESC,rowid DESC LIMIT 24)",[id]).db()?;
    let recent=tx.prepare("SELECT result='ok',COALESCE(elapsed,0) FROM sample WHERE id=? ORDER BY at DESC,rowid DESC LIMIT 12").db()?
        .query_map([id],|r|Ok((r.get::<_,bool>(0)?,r.get::<_,i64>(1)?))).db()?.collect::<Result<Vec<_>,_>>().db()?;
    let streak = recent.iter().take_while(|s| !s.0).count() as i64;
    let pause = match streak {
        0 => 600,
        1 => 900,
        2 => 3600,
        _ => 86400,
    };
    tx.execute("UPDATE endpoint SET at=?2,result=?3,tcp=?4,elapsed=?5,ip=?6,score=?7,checks=?8,oks=?9,streak=?10,due=?11,
       verified=(?3='ok'),success=CASE WHEN ?3='ok' THEN ?2 ELSE success END WHERE id=?1",
       params![id,s.at,s.result,s.tcp,s.elapsed,s.ip.map(|x|x.to_string()),quality(&recent),recent.len() as i64,recent.iter().filter(|s|s.0).count() as i64,streak,s.at+pause]).db()?;
    recommend(&tx, s.at)?;
    tx.commit().db()
}
pub const FRESH: i64 = 1800;
pub(super) fn best(c: &Connection, t: i64) -> R<Option<i64>> {
    c.query_row("SELECT id FROM endpoint WHERE result='ok' AND verified=1 AND at>=?1 ORDER BY score-(?2-at)/180.0 DESC, at DESC,id LIMIT 1",
                params![t-FRESH,t],|r|r.get(0)).optional().db()
}
pub fn recommend(c: &Connection, t: i64) -> R<()> {
    if pref(c, "auto", 0)? == 0 {
        return Ok(());
    }
    let old = pref(c, "selected", 0)?;
    if old != 0 {
        let (streak, at): (i64, i64) = c
            .query_row("SELECT streak,at FROM endpoint WHERE id=?", [old], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .db()?;
        if t - pref(c, "switched", 0)? < 600 || (streak < 2 && at >= t - FRESH) {
            return Ok(());
        }
    }
    if let Some(next) = best(c, t)? {
        if next != old {
            set(c, "selected", next)?;
            set(c, "switched", t)?;
        }
    }
    Ok(())
}

pub fn choose(c: &Connection, manual: bool, turn: u8) -> R<Option<i64>> {
    let t = now();
    let selected = pref(c, "selected", 0)?;
    let opened = pref(c, "opened", 0)?;
    let auto = pref(c, "auto", 0)? != 0;
    c.query_row("SELECT id FROM endpoint WHERE due<=?1 OR (?2 AND id IN (?3,?4) AND at<=?1-180)
       OR (?5 AND post >= (SELECT max(post)-40 FROM endpoint) AND at<=?1-180 AND streak<3)
       ORDER BY CASE WHEN ?2 AND id IN (?3,?4) AND at<=?1-180 THEN 0
                     WHEN (?5 OR ?6=0) AND post >= (SELECT max(post)-40 FROM endpoint) AND streak<3 THEN 1
                     WHEN ?6=1 AND (success>0 OR fav=1) THEN 1 ELSE 2 END,
         CASE WHEN ?5 THEN at ELSE due END ASC,digest,id LIMIT 1",
        params![t,auto,selected,opened,manual,turn],|r|r.get(0)).optional().db()
}
#[derive(Serialize)]
pub struct Item {
    pub id: i64,
    pub host: String,
    pub port: u16,
    pub post: i64,
    pub date: String,
    pub fav: bool,
    pub at: i64,
    pub result: String,
    pub tcp: Option<i64>,
    pub elapsed: Option<i64>,
    pub success: i64,
    pub checks: i64,
    pub oks: i64,
    pub streak: i64,
    pub stale: bool,
    pub verified: bool,
}
const FIELDS: &str =
    "id,host,port,post,date,fav,at,result,tcp,elapsed,success,checks,oks,streak,seen,verified";
fn item(r: &Row) -> rusqlite::Result<Item> {
    Ok(Item {
        id: r.get(0)?,
        host: r.get(1)?,
        port: r.get(2)?,
        post: r.get(3)?,
        date: r.get(4)?,
        fav: r.get(5)?,
        at: r.get(6)?,
        result: r.get(7)?,
        tcp: r.get(8)?,
        elapsed: r.get(9)?,
        success: r.get(10)?,
        checks: r.get(11)?,
        oks: r.get(12)?,
        streak: r.get(13)?,
        stale: r.get::<_, i64>(14)? < now() - 86400,
        verified: r.get(15)?,
    })
}
fn get(c: &Connection, id: i64) -> R<Option<Item>> {
    c.query_row(
        &format!("SELECT {FIELDS} FROM endpoint WHERE id=?"),
        [id],
        item,
    )
    .optional()
    .db()
}

#[derive(Serialize)]
pub struct View {
    pub items: Vec<Item>,
    pub total: i64,
    pub matched: i64,
    pub available: i64,
    pub best: Option<Item>,
    pub selected: Option<Item>,
    pub opened: Option<Item>,
    pub auto: bool,
    pub interval: i64,
    pub sync: i64,
    pub before: i64,
    pub busy: String,
    pub error: String,
    pub sources: Vec<Option<SourceReport>>,
    pub direct: bool,
}
pub fn view(
    c: &Connection,
    search: &str,
    filter: &str,
    sort: &str,
    offset: u32,
    busy: String,
    error: String,
) -> R<View> {
    let clause = match filter {
        "fav" => "fav=1",
        "ok" => "result='ok' AND verified=1 AND at>=?2",
        "failed" => "at>0 AND result!='ok'",
        "archive" => "streak>=3",
        _ => "1",
    };
    let order=match sort {"latency"=>"elapsed IS NULL,elapsed","new"=>"post DESC","stability"=>"checks=0, (oks+1.0)/(checks+2) DESC",_=>"CASE WHEN result='ok' AND verified=1 AND at>=?2 THEN 0 WHEN result='new' THEN 1 ELSE 2 END,score-(?2+1800-at)/180.0 DESC"};
    let cond =
        format!("WHERE instr(lower(host || ':' || port),lower(?1))>0 AND {clause} AND ?2>=0");
    let t = now();
    let matched = c
        .query_row(
            &format!("SELECT count(*) FROM endpoint {cond}"),
            params![search, t - FRESH],
            |r| r.get(0),
        )
        .db()?;
    let items = c
        .prepare(&format!(
            "SELECT {FIELDS} FROM endpoint {cond} ORDER BY {order},post DESC,id LIMIT 24 OFFSET ?3"
        ))
        .db()?
        .query_map(params![search, t - FRESH, offset], item)
        .db()?
        .collect::<Result<Vec<_>, _>>()
        .db()?;
    let (total, available) = c
        .query_row(
            "SELECT count(*),count(CASE WHEN result='ok' AND verified=1 AND at>=?1 THEN 1 END) FROM endpoint",
            [t - FRESH],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .db()?;
    Ok(View {
        items,
        total,
        matched,
        available,
        best: get(c, best(c, t)?.unwrap_or(0))?,
        selected: get(c, pref(c, "selected", 0)?)?,
        opened: get(c, pref(c, "opened", 0)?)?,
        auto: pref(c, "auto", 0)? != 0,
        interval: pref(c, "interval", 15)?,
        sync: pref(c, "sync", 0)?,
        before: pref(c, "before", 0)?,
        busy,
        error,
        sources: super::feed::SOURCES
            .iter()
            .map(|source| source_report(c, source.backoff))
            .collect::<R<_>>()?,
        direct: pref(c, "feed_direct", 0)? != 0,
    })
}
pub fn history(c: &Connection, id: i64) -> R<Vec<Sample>> {
    let mut st=c.prepare("SELECT at,result,tcp,elapsed FROM sample WHERE id=? ORDER BY at DESC,rowid DESC LIMIT 24").db()?;
    let rows = st
        .query_map([id], |r| {
            Ok(Sample {
                at: r.get(0)?,
                result: r.get(1)?,
                tcp: r.get(2)?,
                elapsed: r.get(3)?,
                ip: None,
            })
        })
        .db()?;
    rows.collect::<Result<Vec<_>, _>>().db()
}
pub struct Target {
    pub id: i64,
    pub host: String,
    pub address: std::net::SocketAddr,
    pub secret: String,
}
impl Target {
    pub fn local_link(&self, port: u16) -> String {
        let mut url = url::Url::parse("tg://proxy").unwrap();
        url.query_pairs_mut()
            .append_pair("server", "127.0.0.1")
            .append_pair("port", &port.to_string())
            .append_pair("secret", &self.secret);
        url.into()
    }
}

pub fn candidates(c: &Connection, id: i64) -> R<Vec<i64>> {
    if id > 0 {
        return Ok(vec![id]);
    }
    let mut st=c.prepare("SELECT id FROM endpoint WHERE result='ok' OR at=0 OR at<?1
       ORDER BY (result='ok' AND verified=1 AND at>=?2) DESC, CASE WHEN result='ok' THEN score-min(12,(?3-at)/180.0) ELSE 0 END DESC, (at=0) DESC,post/20 DESC,digest LIMIT 32").db()?;
    let rows = st
        .query_map(params![now() - 180, now() - FRESH, now()], |r| r.get(0))
        .db()?;
    rows.collect::<Result<Vec<_>, _>>().db()
}
pub fn verified(c: &Connection, id: i64) -> R<Target> {
    let ip: String = c
        .query_row(
            "SELECT ip FROM endpoint WHERE id=? AND result='ok' AND verified=1 AND at>=?",
            params![id, now() - FRESH],
            |r| r.get(0),
        )
        .optional()
        .db()?
        .ok_or("Сервер не прошёл проверку подключения.")?;
    let address = ip.parse().map_err(|_| "Повреждён проверенный адрес.")?;
    if !super::feed::public_ip(address) {
        return Err("Этот адрес недоступен.".into());
    }
    let e = endpoint(c, id)?;
    Ok(Target {
        id,
        host: e.host,
        address: std::net::SocketAddr::new(address, e.port),
        secret: e.secret,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranking_prefers_reliability() {
        assert!(quality(&[(true, 600); 8]) > quality(&[(true, 10), (false, 0), (false, 0)]));
    }
    #[test]
    fn persistence_and_failover() {
        let directory = std::env::temp_dir().join(format!("veliora-db-{}", crate::win::random_hex(8)));
        let path = directory.join("catalog.sqlite3");
        let mut c = init(&path).unwrap();
        for id in 1..=2 {
            c.execute("INSERT INTO endpoint(id,digest,host,port,key,post,date) VALUES(?1,?1,'x.org',443,x'00',1,'d')",[id]).unwrap();
        }
        set(&c, "auto", 1).unwrap();
        let t = now();
        let mut s = Sample {
            at: t,
            result: "ok".into(),
            tcp: Some(30),
            elapsed: Some(300),
            ip: Some("8.8.8.8".parse().unwrap()),
        };
        record(&mut c, 1, &s).unwrap();
        record(&mut c, 2, &s).unwrap();
        assert_eq!(pref(&c, "selected", 0).unwrap(), 1);
        s.result = "timeout".into();
        s.elapsed = None;
        record(&mut c, 1, &s).unwrap();
        record(&mut c, 1, &s).unwrap();
        assert_eq!(pref(&c, "selected", 0).unwrap(), 1);
        set(&c, "switched", t - 601).unwrap();
        recommend(&c, t).unwrap();
        assert_eq!(pref(&c, "selected", 0).unwrap(), 2);
        assert!(best(&c, t + FRESH + 1).unwrap().is_none());
        assert!(verified(&c, 1).is_err());
        assert_eq!(
            view(&c, "", "ok", "rank", 0, "".into(), "".into())
                .unwrap()
                .matched,
            1
        );
        drop(c);
        let c = init(&path).unwrap();
        assert_eq!(pref(&c, "selected", 0).unwrap(), 2);
        new_verification(&c).unwrap();
        let cache = view(&c, "", "all", "rank", 0, String::new(), String::new()).unwrap();
        assert_eq!((cache.total, cache.available), (2, 0));
        assert!(cache.best.is_none());
        assert!(cache.items.iter().all(|item| !item.verified));
        drop(c);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
