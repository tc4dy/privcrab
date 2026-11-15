use std::net::{IpAddr, Ipv4Addr};

#[derive(Debug, Clone, Copy)]
pub struct Provider {
    pub name: &'static str,
    pub doh_url: &'static str,
    pub dot_host: &'static str,
    pub dot_port: u16,
    pub ips: &'static [IpAddr],
}

impl Provider {
    pub fn doh_host(&self) -> &'static str {
        self.doh_url
            .trim_start_matches("https://")
            .split('/')
            .next()
            .unwrap_or(self.doh_url)
    }
}

const CLOUDFLARE_IPS: [IpAddr; 2] = [
    IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
    IpAddr::V4(Ipv4Addr::new(1, 0, 0, 1)),
];

const QUAD9_IPS: [IpAddr; 2] = [
    IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9)),
    IpAddr::V4(Ipv4Addr::new(149, 112, 112, 112)),
];

const NEXTDNS_IPS: [IpAddr; 2] = [
    IpAddr::V4(Ipv4Addr::new(45, 90, 28, 0)),
    IpAddr::V4(Ipv4Addr::new(45, 90, 30, 0)),
];

pub static PROVIDERS: [Provider; 3] = [
    Provider {
        name: "cloudflare",
        doh_url: "https://cloudflare-dns.com/dns-query",
        dot_host: "one.one.one.one",
        dot_port: 853,
        ips: &CLOUDFLARE_IPS,
    },
    Provider {
        name: "quad9",
        doh_url: "https://dns.quad9.net/dns-query",
        dot_host: "dns.quad9.net",
        dot_port: 853,
        ips: &QUAD9_IPS,
    },
    Provider {
        name: "nextdns",
        doh_url: "https://dns.nextdns.io/dns-query",
        dot_host: "dns.nextdns.io",
        dot_port: 853,
        ips: &NEXTDNS_IPS,
    },
];

pub fn find(name: &str) -> Option<&'static Provider> {
    PROVIDERS
        .iter()
        .find(|provider| provider.name.eq_ignore_ascii_case(name))
}