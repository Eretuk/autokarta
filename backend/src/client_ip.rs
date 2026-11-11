//! The client's address behind reverse proxies.
//!
//! `X-Forwarded-For` is just a request header: anyone who reaches the app
//! directly can write anything into it. It is believed only when the TCP
//! peer is a trusted proxy, and then only as far as the chain stays inside
//! trusted proxies: the first untrusted hop from the right is the client.

use std::{net::IpAddr, str::FromStr};

use axum::http::HeaderMap;

/// A network in CIDR notation: `10.0.0.0/8`, `2001:db8::/32`, or a single
/// address without a prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Net {
    addr: IpAddr,
    prefix: u8,
}

impl Net {
    fn contains(&self, ip: IpAddr) -> bool {
        match (self.addr, canonical(ip)) {
            (IpAddr::V4(net), IpAddr::V4(ip)) => {
                let mask = u32::MAX
                    .checked_shl(32 - u32::from(self.prefix))
                    .unwrap_or(0);
                u32::from(net) & mask == u32::from(ip) & mask
            }
            (IpAddr::V6(net), IpAddr::V6(ip)) => {
                let mask = u128::MAX
                    .checked_shl(128 - u32::from(self.prefix))
                    .unwrap_or(0);
                u128::from(net) & mask == u128::from(ip) & mask
            }
            _ => false,
        }
    }
}

impl FromStr for Net {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let (addr, prefix) = match s.split_once('/') {
            Some((a, p)) => (a, Some(p)),
            None => (s, None),
        };
        let addr: IpAddr = addr
            .parse()
            .map_err(|_| format!("not an IP address: {s}"))?;
        let addr = canonical(addr);
        let max = if addr.is_ipv4() { 32 } else { 128 };
        let prefix = match prefix {
            None => max,
            Some(p) => p
                .parse::<u8>()
                .ok()
                .filter(|p| *p <= max)
                .ok_or_else(|| format!("bad prefix length: {s}"))?,
        };
        Ok(Net { addr, prefix })
    }
}

/// `::ffff:10.0.0.1` (an IPv4 peer on a dual-stack socket) is `10.0.0.1`.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        v4 => v4,
    }
}

/// Loopback and private networks: a proxy on the same host, in a Docker
/// network or in a private LAN/VPC.
const PRIVATE: &[&str] = &[
    "127.0.0.0/8",
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "::1/128",
    "fc00::/7",
];

/// Proxies whose `X-Forwarded-For` is believed. Parsed from a
/// comma-separated list; the word `private` stands for [`PRIVATE`], `none`
/// for an empty list.
#[derive(Debug, Clone, Default)]
pub struct TrustedProxies(Vec<Net>);

impl FromStr for TrustedProxies {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let mut nets = Vec::new();
        for item in s.split(',').map(str::trim).filter(|i| !i.is_empty()) {
            match item {
                "none" => {}
                "private" => nets.extend(PRIVATE.iter().map(|n| n.parse::<Net>().unwrap())),
                other => nets.push(other.parse()?),
            }
        }
        Ok(TrustedProxies(nets))
    }
}

impl TrustedProxies {
    fn trusts(&self, ip: IpAddr) -> bool {
        self.0.iter().any(|n| n.contains(ip))
    }

    /// The client address: the peer itself unless it is a trusted proxy, in
    /// which case `X-Forwarded-For` is walked from the right past every
    /// trusted hop.
    pub fn client_ip(&self, peer: Option<IpAddr>, headers: &HeaderMap) -> Option<IpAddr> {
        let mut client = canonical(peer?);
        if !self.trusts(client) {
            return Some(client);
        }
        let hops = headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(','))
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>();
        for hop in hops.into_iter().rev() {
            // Garbage in the chain: stop at the last address we can vouch for.
            let Ok(ip) = hop.parse::<IpAddr>() else {
                break;
            };
            client = canonical(ip);
            if !self.trusts(client) {
                break;
            }
        }
        Some(client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn ip(s: &str) -> Option<IpAddr> {
        Some(s.parse().unwrap())
    }

    fn xff(values: &[&'static str]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for v in values {
            h.append("x-forwarded-for", HeaderValue::from_static(v));
        }
        h
    }

    #[test]
    fn parses_lists() {
        let t: TrustedProxies = "private, 203.0.113.7, 2001:db8::/32".parse().unwrap();
        assert!(t.trusts("172.18.0.1".parse().unwrap()));
        assert!(t.trusts("::ffff:10.1.2.3".parse().unwrap()));
        assert!(t.trusts("203.0.113.7".parse().unwrap()));
        assert!(!t.trusts("203.0.113.8".parse().unwrap()));
        assert!(t.trusts("2001:db8::1".parse().unwrap()));
        assert!(!t.trusts("8.8.8.8".parse().unwrap()));
        assert!("none".parse::<TrustedProxies>().unwrap().0.is_empty());
        assert!("10.0.0.0/33".parse::<TrustedProxies>().is_err());
        assert!("proxy.local".parse::<TrustedProxies>().is_err());
        let all: TrustedProxies = "0.0.0.0/0".parse().unwrap();
        assert!(all.trusts("8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn untrusted_peer_cannot_spoof() {
        let t: TrustedProxies = "private".parse().unwrap();
        let h = xff(&["1.2.3.4"]);
        assert_eq!(t.client_ip(ip("198.51.100.9"), &h), ip("198.51.100.9"));
    }

    #[test]
    fn trusted_chain_is_walked() {
        let t: TrustedProxies = "private".parse().unwrap();
        // Client wrote a fake hop; our proxy appended the real address.
        let h = xff(&["6.6.6.6, 198.51.100.9", "10.0.0.2"]);
        assert_eq!(t.client_ip(ip("172.18.0.1"), &h), ip("198.51.100.9"));
        // No header: the proxy itself.
        assert_eq!(
            t.client_ip(ip("172.18.0.1"), &HeaderMap::new()),
            ip("172.18.0.1")
        );
        // Garbage stops the walk at the last trusted hop.
        let h = xff(&["nonsense, 10.0.0.2"]);
        assert_eq!(t.client_ip(ip("172.18.0.1"), &h), ip("10.0.0.2"));
        assert_eq!(t.client_ip(None, &h), None);
    }
}
