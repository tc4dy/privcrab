pub static IPV4_ENDPOINTS: &[&str] = &[
    "https://api.ipify.org",
    "https://ipv4.icanhazip.com",
    "https://api4.my-ip.io/v2/ip.json",
];

pub static IPV6_ENDPOINTS: &[&str] = &[
    "https://api64.ipify.org",
    "https://ipv6.icanhazip.com",
    "https://api6.my-ip.io/v2/ip.json",
];

pub static DC_RANGES: &[([u8; 4], u8)] = &[
    ([5, 9, 0, 0], 16),
    ([159, 65, 0, 0], 16),
    ([104, 16, 0, 0], 13),
    ([3, 0, 0, 0], 8),
    ([18, 0, 0, 0], 8),
];