//! The parts of tailscaled's LocalAPI JSON that Pixelscale reads.
//! Every field defaults, so new or missing fields in other Tailscale versions do not break parsing.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "PascalCase")]
pub struct Status {
    /// "Running", "Stopped", "NeedsLogin", "NeedsMachineAuth", "Starting", "NoState".
    pub backend_state: String,
    #[serde(rename = "AuthURL")]
    pub auth_url: String,
    #[serde(rename = "Self")]
    pub this: Peer,
    pub peer: Option<HashMap<String, Peer>>,
    pub current_tailnet: Option<Tailnet>,
    pub health: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "PascalCase")]
pub struct Tailnet {
    pub name: String,
    #[serde(rename = "MagicDNSSuffix")]
    pub magic_dns_suffix: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "PascalCase")]
pub struct Peer {
    /// Stable node ID: what exit-node and Taildrop calls take.
    #[serde(rename = "ID")]
    pub id: String,
    pub host_name: String,
    #[serde(rename = "DNSName")]
    pub dns_name: String,
    #[serde(rename = "OS")]
    pub os: String,
    #[serde(rename = "TailscaleIPs")]
    pub tailscale_ips: Option<Vec<String>>,
    pub online: bool,
    /// This peer is the exit node in use.
    pub exit_node: bool,
    /// This peer offers itself as an exit node.
    pub exit_node_option: bool,
    /// RFC 3339; "0001-01-01T00:00:00Z" when unknown.
    pub last_seen: String,
    /// DERP region in use when the connection is relayed.
    pub relay: String,
    /// Non-empty when a direct connection is established.
    pub cur_addr: String,
}

impl Peer {
    /// Short name: the first label of the MagicDNS name, else the host name.
    pub fn name(&self) -> String {
        let dns = self.dns_name.trim_end_matches('.');
        match dns.split('.').next() {
            Some(first) if !first.is_empty() => first.to_string(),
            _ => self.host_name.clone(),
        }
    }

    /// Full MagicDNS name without the trailing dot, or the IPv4 address if there is none.
    pub fn address(&self) -> String {
        let dns = self.dns_name.trim_end_matches('.');
        if dns.is_empty() {
            self.ipv4()
        } else {
            dns.to_string()
        }
    }

    pub fn ipv4(&self) -> String {
        let ips = self.tailscale_ips.clone().unwrap_or_default();
        ips.iter()
            .find(|ip| !ip.contains(':'))
            .or(ips.first())
            .cloned()
            .unwrap_or_default()
    }

    pub fn os_label(&self) -> &str {
        match self.os.as_str() {
            "linux" => "Linux",
            "android" => "Android",
            "iOS" => "iOS",
            "macOS" => "macOS",
            "windows" => "Windows",
            "tvOS" => "tvOS",
            "" => "",
            other => other,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "PascalCase")]
pub struct Prefs {
    pub want_running: bool,
    pub route_all: bool,
    #[serde(rename = "CorpDNS")]
    pub corp_dns: bool,
    pub shields_up: bool,
    #[serde(rename = "ExitNodeID")]
    pub exit_node_id: String,
    #[serde(rename = "ExitNodeAllowLANAccess")]
    pub exit_node_allow_lan_access: bool,
    pub operator_user: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "PascalCase")]
pub struct Profile {
    #[serde(rename = "ID")]
    pub id: String,
    pub name: String,
    pub network_profile: NetworkProfile,
    pub user_profile: UserProfile,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "PascalCase")]
pub struct NetworkProfile {
    pub domain_name: String,
    #[serde(rename = "MagicDNSName")]
    pub magic_dns_name: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "PascalCase")]
pub struct UserProfile {
    pub login_name: String,
    pub display_name: String,
}

impl Profile {
    pub fn title(&self) -> String {
        [
            &self.network_profile.magic_dns_name,
            &self.network_profile.domain_name,
            &self.name,
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| "Tailnet".to_string())
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "PascalCase")]
pub struct FileTarget {
    pub node: TargetNode,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "PascalCase")]
pub struct TargetNode {
    #[serde(rename = "StableID")]
    pub stable_id: String,
    pub name: String,
}

impl TargetNode {
    pub fn short_name(&self) -> String {
        self.name
            .trim_end_matches('.')
            .split('.')
            .next()
            .unwrap_or("")
            .to_string()
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "PascalCase")]
pub struct WaitingFile {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "PascalCase")]
pub struct PingResult {
    pub latency_seconds: f64,
    /// Set when the reply came over a direct path.
    pub endpoint: String,
    #[serde(rename = "DERPRegionCode")]
    pub derp_region_code: String,
    pub err: String,
}

impl PingResult {
    pub fn describe(&self, name: &str) -> String {
        if !self.err.is_empty() {
            return format!("{name}: {}", self.err);
        }
        let ms = (self.latency_seconds * 1000.0).round() as u64;
        if !self.endpoint.is_empty() {
            format!("{name}: {ms} ms, direct")
        } else if !self.derp_region_code.is_empty() {
            format!("{name}: {ms} ms, via relay ({})", self.derp_region_code)
        } else {
            format!("{name}: {ms} ms")
        }
    }
}

/// Everything one refresh reads, so the interface can tell whether anything changed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// None when tailscaled could not be reached.
    pub status: Option<Status>,
    pub prefs: Prefs,
    pub profiles: Vec<Profile>,
    pub current_profile: String,
    pub targets: Vec<FileTarget>,
    pub waiting: Vec<WaitingFile>,
    pub error: Option<String>,
}

impl Snapshot {
    pub fn running(&self) -> bool {
        self.status
            .as_ref()
            .is_some_and(|s| s.backend_state == "Running")
    }

    /// Peers sorted: online first, then by name.
    pub fn peers(&self) -> Vec<Peer> {
        let mut v: Vec<Peer> = self
            .status
            .as_ref()
            .and_then(|s| s.peer.as_ref())
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        v.sort_by(|a, b| {
            b.online
                .cmp(&a.online)
                .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
        });
        v
    }

    pub fn exit_node(&self) -> Option<Peer> {
        self.peers().into_iter().find(|p| p.exit_node)
    }
}

/// "3 days ago" style text for an RFC 3339 timestamp, given the current Unix time.
pub fn relative_time(rfc3339: &str, now_unix: i64) -> String {
    let Some(then) = parse_rfc3339(rfc3339) else {
        return String::new();
    };
    if then <= 0 {
        return String::new();
    }
    let d = (now_unix - then).max(0);
    let (n, unit) = match d {
        0..=59 => return "last seen just now".to_string(),
        60..=3599 => (d / 60, "minute"),
        3600..=86_399 => (d / 3600, "hour"),
        86_400..=2_591_999 => (d / 86_400, "day"),
        2_592_000..=31_535_999 => (d / 2_592_000, "month"),
        _ => (d / 31_536_000, "year"),
    };
    format!("last seen {n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

/// Seconds since the Unix epoch for "YYYY-MM-DDTHH:MM:SS[.fff]Z". Returns None for other forms.
fn parse_rfc3339(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hh, mm, ss) = (num(11..13)?, num(14..16)?, num(17..19)?);
    // days from civil (Howard Hinnant's algorithm)
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_and_names() {
        let json = r#"{"BackendState":"Running","Self":{"ID":"a","HostName":"max-l5","DNSName":"max-l5.example.ts.net.","TailscaleIPs":["100.84.12.7","fd7a::1"],"Online":true},
          "Peer":{"k1":{"ID":"b","HostName":"Home Server","DNSName":"home-server.example.ts.net.","OS":"linux","TailscaleIPs":["fd7a::2","100.101.33.2"],"Online":false,"ExitNodeOption":true,"LastSeen":"2026-11-08T10:00:00Z"},
                  "k2":{"ID":"c","HostName":"pixel","DNSName":"pixel-8.example.ts.net.","OS":"android","Online":true}},
          "CurrentTailnet":{"Name":"example.ts.net","MagicDNSSuffix":"example.ts.net"},"SomethingNew":{"x":1}}"#;
        let s: Status = serde_json::from_str(json).unwrap();
        assert_eq!(s.this.name(), "max-l5");
        assert_eq!(s.this.ipv4(), "100.84.12.7");
        let snap = Snapshot {
            status: Some(s),
            ..Default::default()
        };
        assert!(snap.running());
        let peers = snap.peers();
        assert_eq!(peers[0].name(), "pixel-8"); // online first
        assert_eq!(peers[1].ipv4(), "100.101.33.2"); // IPv4 preferred even when listed second
        assert_eq!(peers[1].address(), "home-server.example.ts.net");
        assert_eq!(peers[1].os_label(), "Linux");
    }

    #[test]
    fn empty_peer_map_is_fine() {
        let s: Status = serde_json::from_str(r#"{"BackendState":"Stopped","Peer":null}"#).unwrap();
        let snap = Snapshot {
            status: Some(s),
            ..Default::default()
        };
        assert!(!snap.running());
        assert!(snap.peers().is_empty());
    }

    #[test]
    fn relative_times() {
        let now = parse_rfc3339("2026-11-11T10:00:00Z").unwrap();
        assert_eq!(
            relative_time("2026-11-08T10:00:00Z", now),
            "last seen 3 days ago"
        );
        assert_eq!(
            relative_time("2026-11-11T09:00:00.123456Z", now),
            "last seen 1 hour ago"
        );
        assert_eq!(relative_time("0001-01-01T00:00:00Z", now), "");
        assert_eq!(relative_time("garbage", now), "");
    }

    #[test]
    fn epoch_is_zero() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2000-03-01T00:00:00Z"), Some(951_868_800));
    }

    #[test]
    fn ping_text() {
        let direct = PingResult {
            latency_seconds: 0.0234,
            endpoint: "1.2.3.4:41641".into(),
            ..Default::default()
        };
        assert_eq!(direct.describe("home"), "home: 23 ms, direct");
        let relay = PingResult {
            latency_seconds: 0.061,
            derp_region_code: "fra".into(),
            ..Default::default()
        };
        assert_eq!(relay.describe("pixel"), "pixel: 61 ms, via relay (fra)");
        let err = PingResult {
            err: "no reply".into(),
            ..Default::default()
        };
        assert_eq!(err.describe("x"), "x: no reply");
    }
}
