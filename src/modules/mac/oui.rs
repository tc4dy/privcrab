use rand::seq::SliceRandom;
use rand::Rng;

use super::format_mac;

static OUI: &[(&str, &str)] = &[
    ("00:03:93", "Apple"),
    ("00:1C:B3", "Apple"),
    ("3C:07:54", "Apple"),
    ("F0:18:98", "Apple"),
    ("00:13:E8", "Intel"),
    ("00:1B:21", "Intel"),
    ("3C:A9:F4", "Intel"),
    ("A4:34:D9", "Intel"),
    ("00:E0:4C", "Realtek"),
    ("00:06:5B", "Dell"),
    ("00:14:22", "Dell"),
    ("B8:AC:6F", "Dell"),
    ("F8:BC:12", "Dell"),
    ("28:D2:44", "Lenovo"),
    ("98:FA:9B", "Lenovo"),
    ("00:12:47", "Samsung"),
    ("8C:77:12", "Samsung"),
    ("FC:A1:3E", "Samsung"),
    ("00:00:0C", "Cisco"),
    ("00:1B:0D", "Cisco"),
    ("00:05:69", "VMware"),
    ("00:0C:29", "VMware"),
    ("00:50:56", "VMware"),
    ("00:15:5D", "Microsoft"),
    ("00:50:F2", "Microsoft"),
    ("28:18:78", "Microsoft"),
    ("3C:5A:B4", "Google"),
    ("F4:F5:D8", "Google"),
];

pub fn parse_prefix(prefix: &str) -> Option<[u8; 3]> {
    let mut out = [0u8; 3];
    let mut parts = prefix.split(':');
    for slot in &mut out {
        *slot = u8::from_str_radix(parts.next()?, 16).ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some(out)
}

pub fn vendors() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = OUI.iter().map(|(_, name)| *name).collect();
    names.sort_unstable();
    names.dedup();
    names
}

pub fn random_mac_for_vendor(vendor: &str) -> Option<String> {
    let mut rng = rand::thread_rng();
    let prefixes: Vec<&str> = OUI
        .iter()
        .filter(|(_, name)| name.eq_ignore_ascii_case(vendor))
        .map(|(prefix, _)| *prefix)
        .collect();
    let head = parse_prefix(prefixes.choose(&mut rng)?)?;
    let bytes = [
        head[0] & 0xFE,
        head[1],
        head[2],
        rng.gen(),
        rng.gen(),
        rng.gen(),
    ];
    Some(format_mac(&bytes))
}

pub fn random_mac() -> String {
    let mut rng = rand::thread_rng();
    let mut bytes = match OUI
        .choose(&mut rng)
        .and_then(|(prefix, _)| parse_prefix(prefix))
    {
        Some(head) => [head[0], head[1], head[2], rng.gen(), rng.gen(), rng.gen()],
        None => rng.gen::<[u8; 6]>(),
    };
    bytes[0] = (bytes[0] & 0xFE) | 0x02;
    format_mac(&bytes)
}