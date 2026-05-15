//! Hostname resolution via Tailscale.
//!
//! Resolves IP addresses to human-readable hostnames so that job metadata
//! (`submitted_by_name`) shows machine names instead of raw IPs. Resolution
//! order:
//! 1. Per-IP result cache (`DashMap`, populated on first lookup)
//! 2. Tailscale peer map (loaded once from `tailscale status --json` via
//!    `OnceLock`)
//! 3. Raw IP string if Tailscale is unavailable or the IP is not a peer

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::OnceLock;

use dashmap::DashMap;
use serde::Deserialize;
use tracing::{debug, warn};

/// Cache of resolved hostnames, keyed by IP string.
static HOSTNAME_CACHE: OnceLock<DashMap<String, String>> = OnceLock::new();

/// Tailscale peer map: IP → hostname (loaded once).
static TAILSCALE_MAP: OnceLock<HashMap<String, String>> = OnceLock::new();

fn cache() -> &'static DashMap<String, String> {
    HOSTNAME_CACHE.get_or_init(DashMap::new)
}

fn tailscale_map() -> &'static HashMap<String, String> {
    TAILSCALE_MAP.get_or_init(load_tailscale_map)
}

/// Resolve an IP address to a hostname.
///
/// Returns the Tailscale hostname if available, otherwise the raw IP string.
pub fn resolve_hostname(ip: &IpAddr) -> String {
    let ip_str = ip.to_string();

    // Check cache
    if let Some(cached) = cache().get(&ip_str) {
        return cached.clone();
    }

    // Try Tailscale map
    let result = tailscale_map()
        .get(&ip_str)
        .cloned()
        .unwrap_or_else(|| ip_str.clone());

    cache().insert(ip_str, result.clone());
    result
}

#[derive(Deserialize)]
struct TailscaleStatus {
    #[serde(rename = "Peer", default)]
    peer: HashMap<String, TailscalePeer>,
    #[serde(rename = "Self")]
    self_node: Option<TailscalePeer>,
}

#[derive(Deserialize)]
struct TailscalePeer {
    #[serde(rename = "HostName", default)]
    host_name: String,
    #[serde(rename = "TailscaleIPs", default)]
    tailscale_ips: Vec<String>,
}

/// Load the Tailscale peer map by running `tailscale status --json`.
fn load_tailscale_map() -> HashMap<String, String> {
    let output = std::process::Command::new("tailscale")
        .args(["status", "--json"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        Ok(_) => {
            debug!("tailscale status exited non-zero");
            return HashMap::new();
        }
        Err(e) => {
            debug!(error = %e, "tailscale not available");
            return HashMap::new();
        }
    };

    let status: TailscaleStatus = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "Failed to parse tailscale status JSON");
            return HashMap::new();
        }
    };

    let mut map = HashMap::new();

    // Collect all peers (including self) into the IP → hostname map
    let all_peers = status.peer.values().chain(status.self_node.as_ref());

    for peer in all_peers {
        if peer.host_name.is_empty() {
            continue;
        }
        for ip in &peer.tailscale_ips {
            map.insert(ip.clone(), peer.host_name.clone());
        }
    }

    debug!(peers = map.len(), "Loaded Tailscale peer map");
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_resolves() {
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        let result = resolve_hostname(&ip);
        // Should return "127.0.0.1" or a Tailscale name
        assert!(!result.is_empty());
    }

    #[test]
    fn cache_works() {
        let ip: IpAddr = "192.0.2.1".parse().unwrap();
        let r1 = resolve_hostname(&ip);
        let r2 = resolve_hostname(&ip);
        assert_eq!(r1, r2);
    }
}
