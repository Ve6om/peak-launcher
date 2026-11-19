//! The Servers tab: a saved list of Minecraft servers with live status.
//!
//! Status comes from the server itself using Minecraft's own "server list
//! ping" (the same request the multiplayer screen sends), so no third-party
//! status site sees anyone's server list. Addresses without a port follow the
//! game's DNS SRV rule (_minecraft._tcp.<host>), like the game does.

use super::instances;
use super::paths::{self, Paths};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const DEFAULT_PORT: u16 = 25565;
const TIMEOUT: Duration = Duration::from_secs(5);
/// Status replies are small; anything bigger than this is not a Minecraft server.
const MAX_PACKET: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: String,
    pub name: String,
    pub address: String,
    /// The instance last used to join this server, offered first next time.
    #[serde(default)]
    pub instance_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    servers: Vec<Server>,
}

fn store_path(paths: &Paths) -> std::path::PathBuf {
    paths.root.join("servers.json")
}

pub async fn list(paths: &Paths) -> Result<Vec<Server>> {
    Ok(paths::read_json::<Store>(&store_path(paths)).await?.unwrap_or_default().servers)
}

async fn save_all(paths: &Paths, servers: Vec<Server>) -> Result<()> {
    paths::write_json(&store_path(paths), &Store { servers }).await
}

/// Add a server (empty id) or update one. Returns the saved list.
pub async fn save(paths: &Paths, mut server: Server) -> Result<Vec<Server>> {
    server.name = server.name.trim().to_string();
    server.address = server.address.trim().to_string();
    if server.address.is_empty() {
        return Err(Error::msg("Enter the server's address."));
    }
    parse_address(&server.address)?;
    if server.name.is_empty() {
        server.name = server.address.clone();
    }
    let mut all = list(paths).await?;
    if server.id.is_empty() {
        server.id = uuid::Uuid::new_v4().simple().to_string();
        all.push(server);
    } else if let Some(existing) = all.iter_mut().find(|s| s.id == server.id) {
        *existing = server;
    } else {
        all.push(server);
    }
    save_all(paths, all.clone()).await?;
    Ok(all)
}

pub async fn delete(paths: &Paths, id: &str) -> Result<Vec<Server>> {
    let mut all = list(paths).await?;
    all.retain(|s| s.id != id);
    save_all(paths, all.clone()).await?;
    Ok(all)
}

/// Move a server to a new position in the list.
pub async fn reorder(paths: &Paths, ids: &[String]) -> Result<Vec<Server>> {
    let mut all = list(paths).await?;
    all.sort_by_key(|s| ids.iter().position(|i| i == &s.id).unwrap_or(usize::MAX));
    save_all(paths, all.clone()).await?;
    Ok(all)
}

/// Remember which instance joined a server last.
pub async fn remember_instance(paths: &Paths, server_id: &str, instance_id: &str) -> Result<()> {
    let mut all = list(paths).await?;
    if let Some(s) = all.iter_mut().find(|s| s.id == server_id) {
        s.instance_id = Some(instance_id.to_string());
        save_all(paths, all).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------- addresses

/// Split "host", "host:port" or "[v6]:port" into host and an explicit port.
pub fn parse_address(address: &str) -> Result<(String, Option<u16>)> {
    let a = address.trim();
    let bad = || Error::msg("That doesn't look like a server address (e.g. play.example.com or 192.168.1.5:25565).");
    if a.is_empty() || a.contains(char::is_whitespace) || a.contains('/') {
        return Err(bad());
    }
    if let Some(rest) = a.strip_prefix('[') {
        let (host, after) = rest.split_once(']').ok_or_else(bad)?;
        let port = match after.strip_prefix(':') {
            Some(p) => Some(p.parse::<u16>().map_err(|_| bad())?),
            None if after.is_empty() => None,
            None => return Err(bad()),
        };
        return Ok((host.to_string(), port));
    }
    match a.rsplit_once(':') {
        // More than one colon without brackets: a bare IPv6 address.
        Some((host, _)) if host.contains(':') => Ok((a.to_string(), None)),
        Some((host, port)) if !host.is_empty() => Ok((host.to_string(), Some(port.parse::<u16>().map_err(|_| bad())?))),
        Some(_) => Err(bad()),
        None => Ok((a.to_string(), None)),
    }
}

/// Where to actually connect, following _minecraft._tcp SRV records when no port is given.
async fn resolve(address: &str) -> Result<(String, u16)> {
    let (host, port) = parse_address(address)?;
    if let Some(port) = port {
        return Ok((host, port));
    }
    if host.parse::<std::net::IpAddr>().is_err() {
        if let Ok(resolver) = hickory_resolver::TokioAsyncResolver::tokio_from_system_conf() {
            let lookup = tokio::time::timeout(Duration::from_secs(3), resolver.srv_lookup(format!("_minecraft._tcp.{host}."))).await;
            if let Ok(Ok(records)) = lookup {
                if let Some(srv) = records.iter().min_by_key(|r| (r.priority(), std::cmp::Reverse(r.weight()))) {
                    let target = srv.target().to_utf8();
                    return Ok((target.trim_end_matches('.').to_string(), srv.port()));
                }
            }
        }
    }
    Ok((host, DEFAULT_PORT))
}

// ---------------------------------------------------------------- protocol

fn write_varint(buf: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        if v & !0x7F == 0 {
            buf.push(v as u8);
            return;
        }
        buf.push(((v & 0x7F) | 0x80) as u8);
        v >>= 7;
    }
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_varint(buf, s.len() as i32);
    buf.extend_from_slice(s.as_bytes());
}

/// A packet: length, id, then the body.
fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::new();
    write_varint(&mut inner, id);
    inner.extend_from_slice(body);
    let mut out = Vec::new();
    write_varint(&mut out, inner.len() as i32);
    out.extend(inner);
    out
}

async fn read_varint(stream: &mut TcpStream) -> Result<i32> {
    let mut value: u32 = 0;
    for i in 0..5 {
        let byte = stream.read_u8().await?;
        value |= ((byte & 0x7F) as u32) << (7 * i);
        if byte & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err(Error::msg("Not a Minecraft server (bad number in reply)"))
}

fn varint_from(bytes: &[u8], pos: &mut usize) -> Option<i32> {
    let mut value: u32 = 0;
    for i in 0..5 {
        let byte = *bytes.get(*pos)?;
        *pos += 1;
        value |= ((byte & 0x7F) as u32) << (7 * i);
        if byte & 0x80 == 0 {
            return Some(value as i32);
        }
    }
    None
}

/// Read one whole packet and return its id and body.
async fn read_packet(stream: &mut TcpStream) -> Result<(i32, Vec<u8>)> {
    let len = read_varint(stream).await?;
    if len <= 0 || len as usize > MAX_PACKET {
        return Err(Error::msg("Not a Minecraft server (odd reply size)"));
    }
    let mut data = vec![0u8; len as usize];
    stream.read_exact(&mut data).await?;
    let mut pos = 0;
    let id = varint_from(&data, &mut pos).ok_or_else(|| Error::msg("Not a Minecraft server"))?;
    Ok((id, data[pos..].to_vec()))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub online: bool,
    /// Why it couldn't be reached, when offline.
    pub error: Option<String>,
    pub version: Option<String>,
    pub protocol: Option<i64>,
    pub players_online: Option<i64>,
    pub players_max: Option<i64>,
    /// A few player names, when the server shares them.
    pub players_sample: Vec<String>,
    /// The message of the day as Minecraft text (a string or a chat component).
    pub motd: Value,
    /// The server's 64x64 icon as a data: URL.
    pub favicon: Option<String>,
    pub ping_ms: Option<u64>,
}

impl Status {
    fn offline(error: String) -> Status {
        Status {
            online: false,
            error: Some(error),
            version: None,
            protocol: None,
            players_online: None,
            players_max: None,
            players_sample: Vec::new(),
            motd: Value::Null,
            favicon: None,
            ping_ms: None,
        }
    }
}

/// Turn the server's JSON reply into a Status.
fn status_from(json: &Value, ping_ms: Option<u64>) -> Status {
    let favicon = json["favicon"].as_str().filter(|f| f.starts_with("data:image/png;base64,")).map(|f| f.replace(['\n', '\r'], ""));
    Status {
        online: true,
        error: None,
        version: json["version"]["name"].as_str().map(String::from),
        protocol: json["version"]["protocol"].as_i64(),
        players_online: json["players"]["online"].as_i64(),
        players_max: json["players"]["max"].as_i64(),
        players_sample: json["players"]["sample"]
            .as_array()
            .map(|a| a.iter().filter_map(|p| p["name"].as_str().map(String::from)).take(12).collect())
            .unwrap_or_default(),
        motd: json["description"].clone(),
        favicon,
        ping_ms,
    }
}

async fn ping_inner(address: &str) -> Result<Status> {
    let (host, port) = resolve(address).await?;
    let mut stream = tokio::time::timeout(TIMEOUT, TcpStream::connect((host.as_str(), port)))
        .await
        .map_err(|_| Error::msg("Timed out connecting"))??;
    stream.set_nodelay(true).ok();

    // Handshake (next state: status), then the status request.
    let mut hs = Vec::new();
    write_varint(&mut hs, -1); // protocol version: "any" for a status check
    write_string(&mut hs, &host);
    hs.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut hs, 1);
    let mut out = packet(0x00, &hs);
    out.extend(packet(0x00, &[]));
    stream.write_all(&out).await?;

    let (id, body) = tokio::time::timeout(TIMEOUT, read_packet(&mut stream))
        .await
        .map_err(|_| Error::msg("Timed out waiting for the server"))??;
    if id != 0x00 {
        return Err(Error::msg("Not a Minecraft server"));
    }
    let mut pos = 0;
    let len = varint_from(&body, &mut pos).ok_or_else(|| Error::msg("Not a Minecraft server"))? as usize;
    let text = body.get(pos..pos + len).ok_or_else(|| Error::msg("Not a Minecraft server"))?;
    let json: Value = serde_json::from_slice(text).map_err(|_| Error::msg("The server sent a status Peak couldn't read"))?;

    // Latency: send a ping with a token and time the echo. Some servers skip it.
    let started = Instant::now();
    let token = 0x5045_414Bi64; // "PEAK"
    let ping_ms = async {
        stream.write_all(&packet(0x01, &token.to_be_bytes())).await.ok()?;
        let (id, body) = tokio::time::timeout(Duration::from_secs(3), read_packet(&mut stream)).await.ok()?.ok()?;
        (id == 0x01 && body.get(..8) == Some(&token.to_be_bytes())).then(|| started.elapsed().as_millis() as u64)
    }
    .await;
    Ok(status_from(&json, ping_ms))
}

/// Check a server. Never fails: an unreachable server comes back as offline.
pub async fn ping(address: &str) -> Status {
    match ping_inner(address).await {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            let friendly = if msg.contains("refused") {
                "Nothing is running at that address".to_string()
            } else if msg.contains("lookup") || msg.contains("No such host") || msg.contains("failed to lookup") || msg.contains("nodename") {
                "Couldn't find that address".to_string()
            } else {
                msg.trim_start_matches("I/O error: ").to_string()
            };
            Status::offline(friendly)
        }
    }
}

// ---------------------------------------------------------------- servers.dat

/// The servers saved in an instance's own multiplayer list (servers.dat, NBT).
pub async fn from_instance(paths: &Paths, inst_id: &str) -> Result<Vec<(String, String)>> {
    let file = instances::game_dir(paths, inst_id).join("servers.dat");
    let bytes = match tokio::fs::read(&file).await {
        Ok(b) => b,
        Err(_) => return Ok(Vec::new()),
    };
    read_servers_dat(&bytes).ok_or_else(|| Error::msg("Couldn't read this instance's server list"))
}

/// Minimal NBT reader for servers.dat: a compound with a "servers" list of
/// compounds that have "name" and "ip" strings.
fn read_servers_dat(bytes: &[u8]) -> Option<Vec<(String, String)>> {
    struct R<'a> {
        b: &'a [u8],
        p: usize,
    }
    impl R<'_> {
        fn take(&mut self, n: usize) -> Option<&[u8]> {
            let s = self.b.get(self.p..self.p + n)?;
            self.p += n;
            Some(s)
        }
        fn u8(&mut self) -> Option<u8> {
            Some(self.take(1)?[0])
        }
        fn i16(&mut self) -> Option<i16> {
            Some(i16::from_be_bytes(self.take(2)?.try_into().ok()?))
        }
        fn i32(&mut self) -> Option<i32> {
            Some(i32::from_be_bytes(self.take(4)?.try_into().ok()?))
        }
        fn string(&mut self) -> Option<String> {
            let len = self.i16()? as u16 as usize;
            Some(String::from_utf8_lossy(self.take(len)?).into_owned())
        }
        /// Skip a payload of the given tag type.
        fn skip(&mut self, tag: u8, depth: u8) -> Option<()> {
            if depth > 32 {
                return None;
            }
            match tag {
                1 => self.take(1).map(|_| ()),
                2 => self.take(2).map(|_| ()),
                3 | 5 => self.take(4).map(|_| ()),
                4 | 6 => self.take(8).map(|_| ()),
                7 => {
                    let n = self.i32()?.max(0) as usize;
                    self.take(n).map(|_| ())
                }
                8 => self.string().map(|_| ()),
                9 => {
                    let t = self.u8()?;
                    let n = self.i32()?.max(0);
                    for _ in 0..n {
                        self.skip(t, depth + 1)?;
                    }
                    Some(())
                }
                10 => loop {
                    let t = self.u8()?;
                    if t == 0 {
                        return Some(());
                    }
                    self.string()?;
                    self.skip(t, depth + 1)?;
                },
                11 => {
                    let n = self.i32()?.max(0) as usize;
                    self.take(n * 4).map(|_| ())
                }
                12 => {
                    let n = self.i32()?.max(0) as usize;
                    self.take(n * 8).map(|_| ())
                }
                _ => None,
            }
        }
    }

    let mut r = R { b: bytes, p: 0 };
    if r.u8()? != 10 {
        return None;
    }
    r.string()?; // root name
    let mut out = Vec::new();
    loop {
        let tag = r.u8()?;
        if tag == 0 {
            break;
        }
        let name = r.string()?;
        if tag == 9 && name == "servers" {
            let item = r.u8()?;
            let count = r.i32()?.max(0);
            for _ in 0..count {
                if item != 10 {
                    r.skip(item, 1)?;
                    continue;
                }
                let (mut sname, mut ip) = (String::new(), String::new());
                loop {
                    let t = r.u8()?;
                    if t == 0 {
                        break;
                    }
                    let key = r.string()?;
                    match (t, key.as_str()) {
                        (8, "name") => sname = r.string()?,
                        (8, "ip") => ip = r.string()?,
                        _ => r.skip(t, 2)?,
                    }
                }
                if !ip.trim().is_empty() {
                    out.push((if sname.trim().is_empty() { ip.clone() } else { sname }, ip));
                }
            }
        } else {
            r.skip(tag, 1)?;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_addresses() {
        assert_eq!(parse_address("play.example.com").unwrap(), ("play.example.com".into(), None));
        assert_eq!(parse_address(" mc.example.net:25570 ").unwrap(), ("mc.example.net".into(), Some(25570)));
        assert_eq!(parse_address("192.168.1.5:25565").unwrap(), ("192.168.1.5".into(), Some(25565)));
        assert_eq!(parse_address("[::1]:25566").unwrap(), ("::1".into(), Some(25566)));
        assert_eq!(parse_address("2001:db8::1").unwrap(), ("2001:db8::1".into(), None));
        assert!(parse_address("").is_err());
        assert!(parse_address("has space.com").is_err());
        assert!(parse_address("host:notaport").is_err());
        assert!(parse_address("host:99999").is_err());
        assert!(parse_address("https://example.com").is_err());
    }

    #[test]
    fn varints_round_trip() {
        for v in [0, 1, 127, 128, 255, 25565, 2097151, i32::MAX, -1] {
            let mut buf = Vec::new();
            write_varint(&mut buf, v);
            let mut pos = 0;
            assert_eq!(varint_from(&buf, &mut pos), Some(v), "value {v}");
            assert_eq!(pos, buf.len());
        }
        let mut b = Vec::new();
        write_varint(&mut b, -1);
        assert_eq!(b, [0xff, 0xff, 0xff, 0xff, 0x0f]);
    }

    #[test]
    fn reads_status_replies() {
        let reply = json!({
            "version": { "name": "Paper 1.21.4", "protocol": 769 },
            "players": { "online": 12, "max": 100, "sample": [{ "name": "Alex", "id": "x" }] },
            "description": { "text": "Hello", "color": "gold" },
            "favicon": "data:image/png;base64,iVBORw0K\nGgo="
        });
        let s = status_from(&reply, Some(42));
        assert!(s.online);
        assert_eq!(s.version.as_deref(), Some("Paper 1.21.4"));
        assert_eq!((s.players_online, s.players_max), (Some(12), Some(100)));
        assert_eq!(s.players_sample, ["Alex"]);
        assert_eq!(s.favicon.as_deref(), Some("data:image/png;base64,iVBORw0KGgo="));
        assert_eq!(s.ping_ms, Some(42));
        // A favicon that isn't a PNG data URL is dropped.
        let odd = status_from(&json!({ "favicon": "javascript:alert(1)" }), None);
        assert_eq!(odd.favicon, None);
    }

    /// Build a servers.dat the way Minecraft writes it.
    fn nbt_servers(entries: &[(&str, &str)]) -> Vec<u8> {
        fn s(b: &mut Vec<u8>, v: &str) {
            b.extend((v.len() as u16).to_be_bytes());
            b.extend(v.as_bytes());
        }
        let mut b = vec![10];
        s(&mut b, "");
        b.push(9);
        s(&mut b, "servers");
        b.push(10);
        b.extend((entries.len() as i32).to_be_bytes());
        for (name, ip) in entries {
            b.push(8);
            s(&mut b, "ip");
            s(&mut b, ip);
            b.push(8);
            s(&mut b, "name");
            s(&mut b, name);
            b.push(8);
            s(&mut b, "icon");
            s(&mut b, "aWNvbg==");
            b.push(1);
            s(&mut b, "acceptTextures");
            b.push(1);
            b.push(0);
        }
        b.push(0);
        b
    }

    #[test]
    fn reads_servers_dat() {
        let data = nbt_servers(&[("Hypixel", "mc.hypixel.net"), ("", "localhost:25566")]);
        assert_eq!(
            read_servers_dat(&data).unwrap(),
            vec![("Hypixel".into(), "mc.hypixel.net".into()), ("localhost:25566".into(), "localhost:25566".into())]
        );
        assert_eq!(read_servers_dat(&nbt_servers(&[])).unwrap(), vec![]);
        assert!(read_servers_dat(b"garbage").is_none());
        assert!(read_servers_dat(&data[..data.len() - 5]).is_none(), "cut-off file");
    }

    /// A fake server on this computer that answers like Minecraft does.
    async fn fake_server(reply: Value) -> u16 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let (id, _) = read_packet(&mut s).await.unwrap();
            assert_eq!(id, 0, "handshake");
            let (id, _) = read_packet(&mut s).await.unwrap();
            assert_eq!(id, 0, "status request");
            let text = reply.to_string();
            let mut body = Vec::new();
            write_string(&mut body, &text);
            s.write_all(&packet(0, &body)).await.unwrap();
            let (id, token) = read_packet(&mut s).await.unwrap();
            assert_eq!(id, 1, "ping");
            s.write_all(&packet(1, &token)).await.unwrap();
        });
        port
    }

    #[tokio::test]
    async fn pings_a_server_over_the_network() {
        let port = fake_server(json!({
            "version": { "name": "1.21.4", "protocol": 769 },
            "players": { "online": 3, "max": 20 },
            "description": "A Minecraft Server"
        }))
        .await;
        let s = ping(&format!("127.0.0.1:{port}")).await;
        assert!(s.online, "{:?}", s.error);
        assert_eq!(s.players_online, Some(3));
        assert_eq!(s.motd, json!("A Minecraft Server"));
        assert!(s.ping_ms.is_some());
    }

    #[tokio::test]
    async fn reports_unreachable_servers_as_offline() {
        // Grab a free port, then close it so nothing is listening.
        let port = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().port();
        let s = ping(&format!("127.0.0.1:{port}")).await;
        assert!(!s.online);
        assert!(s.error.is_some());
    }

    /// Ping a real public server: PEAK_TEST_SERVER=mc.hypixel.net cargo test --lib -- --ignored --nocapture servers_live
    #[tokio::test]
    #[ignore]
    async fn servers_live() {
        let addr = std::env::var("PEAK_TEST_SERVER").unwrap_or_else(|_| "mc.hypixel.net".into());
        let s = ping(&addr).await;
        println!(
            "{addr}: online={} version={:?} players={:?}/{:?} ping={:?}ms favicon={} motd={}",
            s.online,
            s.version,
            s.players_online,
            s.players_max,
            s.ping_ms,
            s.favicon.is_some(),
            s.motd.to_string().chars().take(120).collect::<String>()
        );
        assert!(s.online, "{:?}", s.error);
    }
}
