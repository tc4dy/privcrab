#![cfg(target_os = "linux")]

use privcrab::modules::dns::providers::{find, PROVIDERS};
use privcrab::modules::dns::{
    kind_of, parse_default_iface, parse_dns_key, render_resolv, Protocol, ResolvKind,
};

#[test]
fn provider_list_is_complete() {
    assert_eq!(PROVIDERS.len(), 3);
    let mut names: Vec<&str> = PROVIDERS.iter().map(|provider| provider.name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names, vec!["cloudflare", "nextdns", "quad9"]);
}

#[test]
fn provider_entries_are_usable() {
    for provider in PROVIDERS.iter() {
        assert!(provider.doh_url.starts_with("https://"));
        assert!(!provider.doh_host().is_empty());
        assert!(!provider.doh_host().contains('/'));
        assert!(!provider.dot_host.is_empty());
        assert_eq!(provider.dot_port, 853);
        assert!(!provider.ips.is_empty());
    }
}

#[test]
fn provider_hosts_match_expected_values() {
    let cloudflare = find("cloudflare");
    assert_eq!(
        cloudflare.map(|provider| provider.doh_host()),
        Some("cloudflare-dns.com")
    );
    assert_eq!(
        cloudflare.map(|provider| provider.dot_host),
        Some("one.one.one.one")
    );
    assert_eq!(
        find("quad9").map(|provider| provider.dot_host),
        Some("dns.quad9.net")
    );
}

#[test]
fn provider_lookup_ignores_case() {
    assert!(find("Quad9").is_some());
    assert!(find("NEXTDNS").is_some());
    assert!(find("unknown").is_none());
}

#[test]
fn resolv_kind_is_detected_from_link_target() {
    assert_eq!(
        kind_of("/run/systemd/resolve/stub-resolv.conf"),
        ResolvKind::SystemdResolved
    );
    assert_eq!(
        kind_of("../run/systemd/resolve/resolv.conf"),
        ResolvKind::SystemdResolved
    );
    assert_eq!(
        kind_of("/run/NetworkManager/resolv.conf"),
        ResolvKind::NetworkManager
    );
    assert_eq!(kind_of("/etc/resolv.conf.head"), ResolvKind::OtherSymlink);
}

#[test]
fn resolv_contents_are_rendered() {
    let Some(provider) = find("cloudflare") else {
        panic!("cloudflare missing");
    };
    assert_eq!(
        render_resolv(provider.ips),
        "nameserver 1.1.1.1\nnameserver 1.0.0.1\noptions edns0\n"
    );
}

#[test]
fn default_interface_is_read_from_route_table() {
    let table = concat!(
        "Iface\tDestination\tGateway\tFlags\tRefCnt\tUse\tMetric\tMask\tMTU\tWindow\tIRTT\n",
        "wlan0\t0002A8C0\t00000000\t0001\t0\t0\t600\t00FFFFFF\t0\t0\t0\n",
        "wlan0\t00000000\t0102A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0\n",
    );
    assert_eq!(parse_default_iface(table), Some("wlan0".to_string()));
    let no_default = concat!(
        "Iface\tDestination\tGateway\tFlags\tRefCnt\tUse\tMetric\tMask\tMTU\tWindow\tIRTT\n",
        "wlan0\t0002A8C0\t00000000\t0001\t0\t0\t600\t00FFFFFF\t0\t0\t0\n",
    );
    assert_eq!(parse_default_iface(no_default), None);
    assert_eq!(parse_default_iface(""), None);
}

#[test]
fn network_manager_dns_key_is_parsed() {
    assert_eq!(
        parse_dns_key("[main]\nplugin=ifupdown\ndns=none\n"),
        Some("none".to_string())
    );
    assert_eq!(
        parse_dns_key("[main]\n  dns=default  \n"),
        Some("default".to_string())
    );
    assert_eq!(parse_dns_key("[main]\nplugin=ifupdown\n"), None);
}

#[test]
fn protocol_labels_are_stable() {
    assert_eq!(Protocol::Doh.label(), "DoH");
    assert_eq!(Protocol::Dot.label(), "DoT");
}