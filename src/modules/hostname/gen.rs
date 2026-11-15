use rand::Rng;

pub static ADJECTIVES: [&str; 40] = [
    "quiet", "fast", "clean", "dark", "light", "calm", "cold", "warm", "dry", "flat", "sharp",
    "slim", "soft", "bare", "bold", "brisk", "close", "crisp", "dense", "dull", "early", "firm",
    "fresh", "grey", "hard", "high", "idle", "keen", "late", "lean", "long", "loud", "mild",
    "near", "neat", "plain", "pure", "raw", "safe", "slow",
];

pub static NOUNS: [&str; 40] = [
    "node", "host", "desk", "rack", "unit", "box", "core", "link", "port", "gate", "bridge",
    "relay", "tower", "cell", "bay", "dock", "hub", "lab", "loop", "mesh", "pad", "pod", "pool",
    "ring", "slot", "stack", "stand", "base", "bench", "board", "cart", "crate", "frame", "grid",
    "lane", "mast", "pier", "pole", "shelf", "tray",
];

pub fn generate() -> String {
    let mut rng = rand::thread_rng();
    let adjective = ADJECTIVES[rng.gen_range(0..ADJECTIVES.len())];
    let noun = NOUNS[rng.gen_range(0..NOUNS.len())];
    format!("{adjective}-{noun}-{:04x}", rng.gen::<u16>())
}