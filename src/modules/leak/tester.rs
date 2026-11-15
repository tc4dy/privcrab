use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use hickory_resolver::TokioAsyncResolver;
use reqwest::Client;
use tokio::task::JoinSet;

use super::providers::{DC_RANGES, IPV4_ENDPOINTS, IPV6_ENDPOINTS};
use crate::error::LeakError;
use crate::style::{green, neg, note, ok, purple, turquoise, warn};

const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_BODY: usize = 8192;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeakReport {
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub dns_servers: Vec<String>,
    pub vpn_likely: bool,
}

pub fn in_cidr(ip: Ipv4Addr, net: Ipv4Addr, bits: u8) -> bool {
    if bits > 32 {
        return false;
    }
    if bits == 0 {
        return true;
    }
    let mask = u32::MAX << (32 - u32::from(bits));
    u32::from(ip) & mask == u32::from(net) & mask
}

pub fn in_dc_range(ip: Ipv4Addr) -> bool {
    DC_RANGES
        .iter()
        .any(|(net, bits)| in_cidr(ip, Ipv4Addr::from(*net), *bits))
}

pub async fn run() -> Result<LeakReport, LeakError> {
    run_with(false).await
}

pub async fn run_with(full: bool) -> Result<LeakReport, LeakError> {
    let client = Client::builder()
        .timeout(TIMEOUT)
        .user_agent(concat!("privcrab/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let (v6, v4, dns) = tokio::join!(
        gather(&client, IPV6_ENDPOINTS, !full),
        gather(&client, IPV4_ENDPOINTS, !full),
        dns_servers()
    );
    if full {
        show_endpoints(&v6);
        show_endpoints(&v4);
    }
    let ipv6 = pick(&v6, true);
    let ipv4 = pick(&v4, false);
    if ipv4.is_none() && ipv6.is_none() {
        return Err(LeakError::Timeout);
    }
    let dns_servers = match dns {
        Ok(list) => list,
        Err(err) => {
            tracing::debug!("{err:?}");
            Vec::new()
        }
    };
    let vpn_likely = match ipv4 {
        Some(IpAddr::V4(ip)) => in_dc_range(ip),
        _ => false,
    };
    Ok(LeakReport {
        ipv4: ipv4.map(|ip| ip.to_string()),
        ipv6: ipv6.map(|ip| ip.to_string()),
        dns_servers,
        vpn_likely,
    })
}

pub fn print(report: &LeakReport) {
    match &report.ipv6 {
        Some(ip) => println!("{}", note(&format!("ipv6: {}", green(ip)))),
        None => println!("{}", neg("ipv6 not reachable")),
    }
    match &report.ipv4 {
        Some(ip) => println!("{}", ok(&format!("ipv4: {}", green(ip)))),
        None => println!("{}", neg("ipv4 not reachable")),
    }
    if report.dns_servers.is_empty() {
        println!("{}", warn("dns resolver address could not be determined"));
    }
    for server in &report.dns_servers {
        println!(
            "{}",
            note(&format!(
                "dns resolver seen from outside: {}",
                turquoise(server)
            ))
        );
    }
    if report.vpn_likely {
        println!("{}", ok("ipv4 address is inside a known datacenter range"));
    } else {
        println!(
            "{}",
            neg("ipv4 address is not inside a known datacenter range")
        );
    }
}

fn pick(results: &[(String, Option<IpAddr>)], v6: bool) -> Option<IpAddr> {
    results
        .iter()
        .filter_map(|(_, ip)| *ip)
        .find(|ip| ip.is_ipv6() == v6)
}

fn show_endpoints(results: &[(String, Option<IpAddr>)]) {
    for (url, ip) in results {
        match ip {
            Some(ip) => println!(
                "{}",
                note(&format!("{} -> {}", purple(url), green(&ip.to_string())))
            ),
            None => println!("{}", note(&format!("{} -> failed", purple(url)))),
        }
    }
}

async fn fetch(client: &Client, url: &str) -> Option<IpAddr> {
    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(err) => {
            tracing::debug!("{url}: {err:?}");
            return None;
        }
    };
    if !response.status().is_success() {
        return None;
    }
    let bytes = match response.bytes().await {
        Ok(b) if b.len() <= MAX_BODY => b,
        Ok(b) => {
            tracing::debug!("{url}: response too large ({} bytes)", b.len());
            return None;
        }
        Err(err) => {
            tracing::debug!("{url}: {err:?}");
            return None;
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    let trimmed = text.trim();
    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return Some(ip);
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(ip_str) = value.get("ip").and_then(|v| v.as_str()) {
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                return Some(ip);
            }
        }
    }
    None
}

async fn gather(client: &Client, urls: &[&str], first_only: bool) -> Vec<(String, Option<IpAddr>)> {
    let mut set = JoinSet::new();
    for url in urls {
        let client = client.clone();
        let url = url.to_string();
        set.spawn(async move {
            let ip = fetch(&client, &url).await;
            (url, ip)
        });
    }
    let mut out = Vec::new();
    while let Some(joined) = set.join_next().await {
        if let Ok(item) = joined {
            let hit = item.1.is_some();
            out.push(item);
            if first_only && hit {
                break;
            }
        }
    }
    out
}

async fn dns_servers() -> Result<Vec<String>, LeakError> {
    let resolver = TokioAsyncResolver::tokio_from_system_conf()
        .map_err(|err| LeakError::DnsQueryFailed(err.to_string()))?;
    let found = tokio::time::timeout(TIMEOUT, resolver.lookup_ip("whoami.akamai.net"))
        .await
        .map_err(|_| LeakError::Timeout)?
        .map_err(|err| LeakError::DnsQueryFailed(err.to_string()))?;
    let mut out: Vec<String> = found.iter().map(|ip| ip.to_string()).collect();
    out.sort();
    out.dedup();
    Ok(out)
}