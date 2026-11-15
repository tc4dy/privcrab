use std::collections::HashSet;
use std::net::{Ipv4Addr, Ipv6Addr};

use privcrab::modules::leak::providers::{DC_RANGES, IPV4_ENDPOINTS, IPV6_ENDPOINTS};
use privcrab::modules::leak::tester::{in_cidr, in_dc_range, run_with, LeakReport};

#[test]
fn endpoint_lists_are_usable() {
    assert!(IPV4_ENDPOINTS.len() >= 3);
    assert!(IPV6_ENDPOINTS.len() >= 3);
    let all: Vec<&str> = IPV4_ENDPOINTS
        .iter()
        .chain(IPV6_ENDPOINTS.iter())
        .copied()
        .collect();
    for url in &all {
        assert!(url.starts_with("https://"), "not https: {url}");
    }
    let unique: HashSet<&str> = all.iter().copied().collect();
    assert_eq!(unique.len(), all.len());
}

#[test]
fn cidr_matching_follows_prefix_length() {
    let net = Ipv4Addr::new(10, 0, 0, 0);
    assert!(in_cidr(Ipv4Addr::new(10, 1, 2, 3), net, 8));
    assert!(!in_cidr(Ipv4Addr::new(11, 0, 0, 1), net, 8));
    assert!(in_cidr(Ipv4Addr::new(203, 0, 113, 9), net, 0));
    assert!(in_cidr(net, net, 32));
    assert!(!in_cidr(Ipv4Addr::new(10, 0, 0, 1), net, 32));
    assert!(!in_cidr(net, net, 33));
}

#[test]
fn cidr_matching_handles_unaligned_boundaries() {
    let net = Ipv4Addr::new(104, 16, 0, 0);
    assert!(in_cidr(Ipv4Addr::new(104, 16, 0, 1), net, 13));
    assert!(in_cidr(Ipv4Addr::new(104, 23, 255, 255), net, 13));
    assert!(!in_cidr(Ipv4Addr::new(104, 24, 0, 1), net, 13));
    assert!(!in_cidr(Ipv4Addr::new(104, 15, 255, 255), net, 13));
}

#[test]
fn datacenter_ranges_are_well_formed() {
    for (net, bits) in DC_RANGES {
        assert!((1..=32).contains(bits), "bad prefix length {bits}");
        let value = u32::from_be_bytes(*net);
        let mask = u32::MAX << (32 - u32::from(*bits));
        assert_eq!(value & !mask, 0, "host bits set in {net:?}/{bits}");
    }
}

#[test]
fn datacenter_lookup_matches_known_ranges_only() {
    assert!(in_dc_range(Ipv4Addr::new(159, 65, 10, 20)));
    assert!(in_dc_range(Ipv4Addr::new(5, 9, 1, 1)));
    assert!(!in_dc_range(Ipv4Addr::new(8, 8, 8, 8)));
    assert!(!in_dc_range(Ipv4Addr::new(192, 168, 1, 1)));
    assert!(!in_dc_range(Ipv4Addr::new(10, 0, 0, 1)));
}

#[test]
fn empty_report_has_no_findings() {
    let report = LeakReport::default();
    assert!(report.ipv4.is_none());
    assert!(report.ipv6.is_none());
    assert!(report.dns_servers.is_empty());
    assert!(!report.vpn_likely);
}

#[test]
fn live_report_is_consistent_when_network_is_available() {
    if let Ok(report) = tokio_test::block_on(run_with(false)) {
        assert!(report.ipv4.is_some() || report.ipv6.is_some());
        if let Some(text) = &report.ipv4 {
            assert!(text.parse::<Ipv4Addr>().is_ok());
        }
        if let Some(text) = &report.ipv6 {
            assert!(text.parse::<Ipv6Addr>().is_ok());
        }
    }
}