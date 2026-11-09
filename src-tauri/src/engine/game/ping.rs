// Minecraft server ping: opens a connection to the server address and reads its public answer (online, version, name).
// Local addresses and addresses inside your network are rejected. Nothing about you is sent to the server except the status request itself.
// Minecraft Server List Ping (1.7+): handshake, status request, status JSON.
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::engine::host_is_local;

#[derive(serde::Serialize)]
pub struct PingResult {
    pub online: i64,
    pub max: i64,
    pub motd: String,
    pub version: String,
    pub favicon: Option<String>,
    pub ms: u64,
}

fn write_varint(buf: &mut Vec<u8>, mut v: i32) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v = ((v as u32) >> 7) as i32;
        if v != 0 {
            b |= 0x80;
        }
        buf.push(b);
        if v == 0 {
            break;
        }
    }
}

fn read_varint<R: Read>(r: &mut R) -> std::io::Result<i32> {
    let mut num = 0i32;
    let mut shift = 0;
    loop {
        let mut b = [0u8; 1];
        r.read_exact(&mut b)?;
        num |= ((b[0] & 0x7f) as i32) << shift;
        if b[0] & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 35 {
            break;
        }
    }
    Ok(num)
}

// The MOTD is either a plain string or a chat component {text, extra:[...]}.
fn motd_text(v: &serde_json::Value) -> String {
    if let Some(s) = v.as_str() {
        return s.to_string();
    }
    let mut out = String::new();
    if let Some(t) = v["text"].as_str() {
        out.push_str(t);
    }
    if let Some(arr) = v["extra"].as_array() {
        for e in arr {
            out.push_str(&motd_text(e));
        }
    }
    // Strip legacy formatting codes (section sign plus one character).
    out.chars()
        .collect::<Vec<_>>()
        .split(|c| *c == '§')
        .enumerate()
        .map(|(i, part)| if i == 0 { part.iter().collect::<String>() } else { part.iter().skip(1).collect::<String>() })
        .collect::<Vec<_>>()
        .join("")
}

const LOCAL_REFUSED: &str = "локальные и домашние адреса лаунчер не проверяет";

/// A bracketed IPv6 literal has to be split before the last colon is taken for
/// a port, otherwise `[::1]:25565` and `::1` both end up as garbage hosts.
fn split_host_port(addr: &str) -> (String, u16) {
    if let Some((host, tail)) = addr.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
        let port = tail.strip_prefix(':').and_then(|p| p.parse().ok()).unwrap_or(25565);
        return (host.to_string(), port);
    }
    match addr.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => (h.to_string(), p.parse().unwrap_or(25565)),
        _ => (addr.to_string(), 25565),
    }
}

const SRV_WAIT: Duration = Duration::from_secs(3);
const MAX_SRV_TARGETS: usize = 4;
const MAX_ADDRESSES_PER_TARGET: usize = 2;

fn ordered_srv_candidates(mut records: Vec<(u16, String)>) -> Vec<String> {
    records.retain(|(_, endpoint)| !endpoint.is_empty());
    records.sort_by_key(|(priority, _)| *priority);
    let mut candidates = Vec::new();
    for (_, endpoint) in records {
        if !candidates.contains(&endpoint) {
            candidates.push(endpoint);
        }
        if candidates.len() == MAX_SRV_TARGETS {
            break;
        }
    }
    candidates
}

fn srv_name(addr: &str) -> Option<String> {
    let (host, port) = split_host_port(addr);
    let explicit_port = port != 25565 || addr.ends_with(":25565");
    if explicit_port || host.is_empty() || host.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }
    Some(format!("_minecraft._tcp.{}", host.trim_end_matches('.')))
}

/// Candidate endpoints for a status check. A name without a port is looked up
/// as `_minecraft._tcp.<name>` first; an unavailable SRV lookup falls back to
/// the address the player entered.
pub async fn resolve_srv(addr: &str) -> Vec<String> {
    use hickory_resolver::TokioResolver;
    static RESOLVER: std::sync::OnceLock<Option<TokioResolver>> = std::sync::OnceLock::new();
    let Some(name) = srv_name(addr) else { return vec![addr.to_string()] };
    let resolver = RESOLVER.get_or_init(|| TokioResolver::builder_tokio().map(|b| b.build()).ok());
    let Some(resolver) = resolver else { return vec![addr.to_string()] };
    let found = match tokio::time::timeout(SRV_WAIT, resolver.srv_lookup(name)).await {
        Ok(Ok(found)) => found,
        _ => return vec![addr.to_string()],
    };
    let records = found
        .iter()
        .filter_map(|r| {
            let target = r.target().to_utf8().trim_end_matches('.').to_string();
            (!target.is_empty() && r.port() != 0).then(|| (r.priority(), format!("{target}:{}", r.port())))
        })
        .collect();
    let candidates = ordered_srv_candidates(records);
    if candidates.is_empty() { vec![addr.to_string()] } else { candidates }
}

pub fn ping(addr: &str) -> Result<PingResult, String> {
    ping_one(addr)
}

pub fn ping_candidates(candidates: &[String]) -> Result<PingResult, String> {
    ping_candidates_with(candidates, ping_one)
}

fn ping_candidates_with(
    candidates: &[String],
    mut attempt: impl FnMut(&str) -> Result<PingResult, String>,
) -> Result<PingResult, String> {
    let started = std::time::Instant::now();
    let mut failures = Vec::new();
    for endpoint in candidates.iter().take(MAX_SRV_TARGETS) {
        match attempt(endpoint) {
            Ok(mut result) => {
                result.ms = started.elapsed().as_millis() as u64;
                return Ok(result);
            }
            Err(error) => failures.push(format!("{endpoint}: {error}")),
        }
    }
    if failures.is_empty() {
        Err("не удалось разрешить адрес".into())
    } else {
        Err(failures.join("; "))
    }
}

fn connect_public_addresses<T>(
    socks: impl Iterator<Item = std::net::SocketAddr>,
    mut connect: impl FnMut(&std::net::SocketAddr) -> Result<T, String>,
) -> Result<T, String> {
    let mut attempts = 0;
    let mut last_error = None;
    let mut saw_local = false;
    for sock in socks {
        // Check every resolved address, not just the first DNS answer. Never
        // connect to a local/LAN answer, even when the name has public answers too.
        if host_is_local(&sock.ip().to_string()) {
            saw_local = true;
            continue;
        }
        if attempts == MAX_ADDRESSES_PER_TARGET {
            break;
        }
        attempts += 1;
        match connect(&sock) {
            Ok(connected) => return Ok(connected),
            Err(error) => last_error = Some(error),
        }
    }
    match (saw_local, last_error) {
        (true, None) => Err(LOCAL_REFUSED.into()),
        (_, Some(error)) => Err(error),
        (false, None) => Err("не удалось разрешить адрес".into()),
    }
}

fn ping_one(addr: &str) -> Result<PingResult, String> {
    let (host, port) = split_host_port(addr);
    // The webview picks the address, so without this the command is a probe for
    // the local machine, the LAN and cloud metadata. The literal host is judged
    // before any resolution, so a machine without IPv6 still refuses `::1`
    // instead of failing with a lookup error.
    if host_is_local(&host) {
        return Err(LOCAL_REFUSED.into());
    }
    let socks = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| e.to_string())?;
    let start = std::time::Instant::now();
    let mut s = connect_public_addresses(socks, |sock| {
        TcpStream::connect_timeout(sock, Duration::from_secs(4)).map_err(|e| e.to_string())
    })?;
    s.set_read_timeout(Some(Duration::from_secs(4))).ok();
    s.set_write_timeout(Some(Duration::from_secs(4))).ok();

    // Handshake (next state = 1 = status)
    let mut data = Vec::new();
    write_varint(&mut data, 0x00);
    write_varint(&mut data, -1);
    write_varint(&mut data, host.len() as i32);
    data.extend_from_slice(host.as_bytes());
    data.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut data, 1);
    let mut pkt = Vec::new();
    write_varint(&mut pkt, data.len() as i32);
    pkt.extend_from_slice(&data);
    s.write_all(&pkt).map_err(|e| e.to_string())?;

    // Status request: empty packet 0x00.
    let mut req = Vec::new();
    write_varint(&mut req, 0x00);
    let mut pkt2 = Vec::new();
    write_varint(&mut pkt2, req.len() as i32);
    pkt2.extend_from_slice(&req);
    s.write_all(&pkt2).map_err(|e| e.to_string())?;

    // Response: packet length, packet id, JSON length, JSON.
    let _len = read_varint(&mut s).map_err(|e| e.to_string())?;
    let _pid = read_varint(&mut s).map_err(|e| e.to_string())?;
    let jlen = read_varint(&mut s).map_err(|e| e.to_string())? as usize;
    if jlen == 0 || jlen > 2_000_000 {
        return Err("некорректный ответ".into());
    }
    let mut json = vec![0u8; jlen];
    s.read_exact(&mut json).map_err(|e| e.to_string())?;
    let ms = start.elapsed().as_millis() as u64;
    let v: serde_json::Value = serde_json::from_slice(&json).map_err(|e| e.to_string())?;
    Ok(PingResult {
        online: v["players"]["online"].as_i64().unwrap_or(0),
        max: v["players"]["max"].as_i64().unwrap_or(0),
        version: v["version"]["name"].as_str().unwrap_or("").to_string(),
        motd: motd_text(&v["description"]),
        favicon: v["favicon"].as_str().map(|s| s.to_string()),
        ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srv_candidates_keep_priority_and_limit_fallbacks() {
        let records = vec![
            (20, "backup.example:25567".into()),
            (10, "primary.example:25566".into()),
            (10, "primary-two.example:25566".into()),
            (10, "primary.example:25566".into()),
            (30, "last.example:25568".into()),
            (40, "ignored.example:25569".into()),
        ];
        assert_eq!(
            ordered_srv_candidates(records),
            vec![
                "primary.example:25566".to_string(),
                "primary-two.example:25566".to_string(),
                "backup.example:25567".to_string(),
                "last.example:25568".to_string(),
            ],
        );
    }

    #[test]
    fn ping_candidates_try_the_next_endpoint_after_a_failure() {
        let candidates = vec!["first.example:25565".into(), "second.example:25565".into()];
        let mut tried = Vec::new();
        let result = ping_candidates_with(&candidates, |candidate| {
            tried.push(candidate.to_string());
            if candidate == "first.example:25565" {
                Err("unreachable".into())
            } else {
                Ok(PingResult {
                    online: 1,
                    max: 10,
                    motd: "ok".into(),
                    version: "test".into(),
                    favicon: None,
                    ms: 0,
                })
            }
        });
        assert_eq!(result.map(|r| r.motd), Ok("ok".into()));
        assert_eq!(tried, candidates);
    }

    #[test]
    fn address_fallback_skips_local_answers_and_tries_another_public_address() {
        let socks: [std::net::SocketAddr; 3] = [
            "127.0.0.1:25565".parse().unwrap(),
            "203.0.113.10:25565".parse().unwrap(),
            "198.51.100.20:25565".parse().unwrap(),
        ];
        let mut tried = Vec::new();
        let result = connect_public_addresses(socks.into_iter(), |sock| {
            tried.push(sock.ip().to_string());
            if sock.ip().to_string() == "203.0.113.10" {
                Err("unreachable".into())
            } else {
                Ok("connected")
            }
        });
        assert_eq!(result, Ok("connected"));
        assert_eq!(tried, vec!["203.0.113.10".to_string(), "198.51.100.20".to_string()]);
    }

    /// address -> verdict. The webview reaches this command directly, so a ping
    /// must never turn into a port scan of the user's machine or LAN.
    #[test]
    fn srv_is_looked_up_only_for_a_bare_name() {
        let cases = [
            ("mcru.me", Some("_minecraft._tcp.mcru.me"), "a bare name is how players type a server that lives behind SRV"),
            ("Play.Example.org.", Some("_minecraft._tcp.Play.Example.org"), "a trailing root dot must not double into the query"),
            ("mcru.me:25565", None, "an explicit port means the player already chose the endpoint"),
            ("mcru.me:25577", None, "a non-default port bypasses SRV in the game too"),
            ("203.0.113.40", None, "an IP literal has no SRV record to look up"),
            ("[2001:db8::1]:25565", None, "an IPv6 literal must not be turned into a DNS name"),
        ];
        for (addr, want, why) in cases {
            assert_eq!(srv_name(addr).as_deref(), want, "{addr}: {why}");
        }
    }

    #[test]
    fn local_targets_are_refused_before_any_connect() {
        let cases = [
            ("127.0.0.1:22", "loopback exposes services bound to the user's machine"),
            ("localhost:8080", "loopback by name is the same target"),
            ("192.168.1.1:80", "private ranges expose the user's router"),
            ("169.254.169.254:80", "link-local is where cloud metadata lives"),
            ("[::1]:25565", "IPv6 loopback must be refused too"),
            ("::1", "a bare IPv6 literal must not be mangled into a port"),
            ("[fd00::1]:25565", "unique local IPv6 is the user's own network"),
        ];
        for (addr, why) in cases {
            let err = match ping(addr) {
                Ok(_) => panic!("{addr} was pinged instead of refused: {why}"),
                Err(e) => e,
            };
            assert!(
                err.contains("локальные"),
                "{addr} failed with «{err}» instead of the local-address refusal: {why}",
            );
        }
    }
}
