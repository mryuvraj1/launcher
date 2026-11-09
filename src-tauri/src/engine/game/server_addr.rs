const DEFAULT_PORT: u16 = 25565;
const INVALID_ADDRESS: &str = "Некорректный адрес сервера: проверь имя и порт (1-65535)";

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ServerAddress {
    pub host: String,
    pub port: u16,
    pub explicit_port: bool,
}

fn port(raw: &str) -> Result<u16, String> {
    raw.parse::<u16>().ok().filter(|port| *port != 0).ok_or_else(|| INVALID_ADDRESS.into())
}

pub(crate) fn parse_server_address(raw: &str) -> Result<ServerAddress, String> {
    let address = raw.trim();
    if address.is_empty() || address.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(INVALID_ADDRESS.into());
    }

    if let Some(rest) = address.strip_prefix('[') {
        let (host, tail) = rest.split_once(']').ok_or_else(|| INVALID_ADDRESS.to_string())?;
        if host.parse::<std::net::Ipv6Addr>().is_err() {
            return Err(INVALID_ADDRESS.into());
        }
        let (port, explicit_port) = if tail.is_empty() {
            (DEFAULT_PORT, false)
        } else {
            let raw_port = tail.strip_prefix(':').ok_or_else(|| INVALID_ADDRESS.to_string())?;
            (port(raw_port)?, true)
        };
        return Ok(ServerAddress { host: host.to_string(), port, explicit_port });
    }

    if address.contains(['[', ']']) {
        return Err(INVALID_ADDRESS.into());
    }
    if let Some((host, raw_port)) = address.rsplit_once(':') {
        if !host.contains(':') {
            if host.is_empty() {
                return Err(INVALID_ADDRESS.into());
            }
            return Ok(ServerAddress { host: host.to_string(), port: port(raw_port)?, explicit_port: true });
        }
    }

    if address.parse::<std::net::IpAddr>().is_ok() {
        return Ok(ServerAddress { host: address.to_string(), port: DEFAULT_PORT, explicit_port: false });
    }
    if address.contains(':') {
        return Err(INVALID_ADDRESS.into());
    }
    Ok(ServerAddress { host: address.to_string(), port: DEFAULT_PORT, explicit_port: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_host_ports_and_ipv6_without_changing_valid_endpoints() {
        let cases = [
            ("mc.example.net", "mc.example.net", 25565, false),
            ("mc.example.net:25565", "mc.example.net", 25565, true),
            ("mc.example.net:25577", "mc.example.net", 25577, true),
            ("[2001:db8::1]:25570", "2001:db8::1", 25570, true),
            ("[2001:db8::1]", "2001:db8::1", 25565, false),
            ("2001:db8::1", "2001:db8::1", 25565, false),
        ];
        for (raw, host, port, explicit_port) in cases {
            assert_eq!(parse_server_address(raw), Ok(ServerAddress { host: host.into(), port, explicit_port }), "{raw}");
        }
    }

    #[test]
    fn rejects_invalid_explicit_ports_and_malformed_ipv6() {
        for raw in ["mc.example.net:", "mc.example.net:nope", "mc.example.net:0", "mc.example.net:65536", "[2001:db8::1]nope", "[not-ipv6]:25565", "2001:db8::zz"] {
            assert!(parse_server_address(raw).is_err(), "accepted invalid address {raw}");
        }
    }
}