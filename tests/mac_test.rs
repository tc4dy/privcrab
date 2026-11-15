#![cfg(target_os = "linux")]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use mockall::mock;
use privcrab::config::{MacBackup, State};
use privcrab::error::MacError;
use privcrab::modules::hostname::gen;
use privcrab::modules::mac::oui;
use privcrab::modules::mac::spoof::{restore_with, spoof_with};
use privcrab::modules::mac::{format_mac, is_valid_mac, parse_mac};
use privcrab::modules::probe::block::src_mac;
use privcrab::util::backup;
use privcrab::util::iface::{is_valid_name, parse_net_dev, Link};
use privcrab::util::privilege::cap_eff;

mock! {
    pub Nic {}
    impl Link for Nic {
        fn read_mac(&self, iface: &str) -> Result<String, MacError>;
        fn is_up(&self, iface: &str) -> Result<bool, MacError>;
        fn set_down(&self, iface: &str) -> Result<(), MacError>;
        fn set_up(&self, iface: &str) -> Result<(), MacError>;
        fn set_hwaddr(&self, iface: &str, mac: [u8; 6]) -> Result<(), MacError>;
    }
}

const ORIGINAL: &str = "00:11:22:33:44:55";
const ORIGINAL_BYTES: [u8; 6] = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
const FORGED: &str = "02:aa:bb:cc:dd:ee";
const FORGED_BYTES: [u8; 6] = [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee];

struct Store(PathBuf);

impl Store {
    fn new(name: &str) -> Store {
        let dir =
            std::env::temp_dir().join(format!("privcrab-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Store(dir.join("state.json"))
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        if let Some(dir) = self.0.parent() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

fn saved_mac(iface: &str, original: &str) -> State {
    State {
        mac: Some(MacBackup {
            iface: iface.to_string(),
            original: original.to_string(),
        }),
        ..State::default()
    }
}

fn run_cli(args: &[&str]) -> (bool, String) {
    match Command::new(env!("CARGO_BIN_EXE_privcrab"))
        .args(args)
        .output()
    {
        Ok(out) => (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        ),
        Err(_) => (false, String::new()),
    }
}

#[test]
fn parse_mac_accepts_valid_addresses() {
    assert_eq!(
        parse_mac("aa:bb:cc:dd:ee:ff"),
        Some([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff])
    );
    assert_eq!(
        parse_mac("AA:BB:CC:DD:EE:FF"),
        Some([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff])
    );
    assert!(is_valid_mac(ORIGINAL));
}

#[test]
fn parse_mac_rejects_malformed_input() {
    for text in [
        "",
        "aa:bb:cc:dd:ee",
        "aa:bb:cc:dd:ee:ff:00",
        "aabbccddeeff",
        "gg:bb:cc:dd:ee:ff",
        "a:b:c:d:e:f",
        "+a:bb:cc:dd:ee:ff",
        "aa-bb-cc-dd-ee-ff",
        "aa:bb:cc:dd:ee:ff:",
    ] {
        assert!(!is_valid_mac(text), "accepted {text:?}");
    }
}

#[test]
fn format_mac_round_trips() {
    assert_eq!(format_mac(&FORGED_BYTES), FORGED);
    assert_eq!(parse_mac(&format_mac(&ORIGINAL_BYTES)), Some(ORIGINAL_BYTES));
}

#[test]
fn random_mac_is_local_and_unicast() {
    for _ in 0..200 {
        let text = oui::random_mac();
        let Some(bytes) = parse_mac(&text) else {
            panic!("unparseable address {text}");
        };
        assert_eq!(bytes[0] & 1, 0);
        assert_eq!(bytes[0] & 2, 2);
    }
}

#[test]
fn vendor_lookup_is_case_insensitive_and_unicast() {
    for name in ["intel", "INTEL", "Intel"] {
        let Some(text) = oui::random_mac_for_vendor(name) else {
            panic!("no address for {name}");
        };
        let Some(bytes) = parse_mac(&text) else {
            panic!("unparseable address {text}");
        };
        assert_eq!(bytes[0] & 1, 0);
    }
    assert!(oui::random_mac_for_vendor("no-such-vendor").is_none());
}

#[test]
fn vendor_table_covers_required_vendors() {
    let vendors = oui::vendors();
    for name in [
        "Apple",
        "Intel",
        "Realtek",
        "Dell",
        "Lenovo",
        "Samsung",
        "Cisco",
        "VMware",
        "Microsoft",
        "Google",
    ] {
        assert!(vendors.contains(&name), "missing {name}");
        assert!(oui::random_mac_for_vendor(name).is_some());
    }
}

#[test]
fn prefix_parser_checks_shape() {
    assert_eq!(oui::parse_prefix("3C:07:54"), Some([0x3c, 0x07, 0x54]));
    assert_eq!(oui::parse_prefix("3C:07"), None);
    assert_eq!(oui::parse_prefix("3C:07:54:11"), None);
    assert_eq!(oui::parse_prefix("ZZ:07:54"), None);
}

#[test]
fn interface_names_are_validated() {
    assert!(is_valid_name("wlan0"));
    assert!(is_valid_name("wlan-0_a"));
    assert!(!is_valid_name(""));
    assert!(!is_valid_name("abcdefghijklmnop"));
    assert!(!is_valid_name("../etc"));
    assert!(!is_valid_name("eth0:1"));
    assert!(!is_valid_name("eth 0"));
}

#[test]
fn net_dev_is_parsed() {
    let text = concat!(
        "Inter-|   Receive                                                |  Transmit\n",
        " face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n",
        "    lo:    1000      10    0    0    0     0          0         0     1000      10    0    0    0     0       0          0\n",
        "  eth0: 2000000    1500    0    0    0     0          0         0   300000     900    0    0    0     0       0          0\n",
    );
    assert_eq!(parse_net_dev(text), vec!["lo".to_string(), "eth0".to_string()]);
}

#[test]
fn hostname_lists_have_expected_shape() {
    assert_eq!(gen::ADJECTIVES.len(), 40);
    assert_eq!(gen::NOUNS.len(), 40);
    for word in gen::ADJECTIVES.iter().chain(gen::NOUNS.iter()) {
        assert!(!word.is_empty());
        assert!(word.bytes().all(|b| b.is_ascii_lowercase()));
    }
    let mut adjectives = gen::ADJECTIVES.to_vec();
    adjectives.sort_unstable();
    adjectives.dedup();
    assert_eq!(adjectives.len(), 40);
    let mut nouns = gen::NOUNS.to_vec();
    nouns.sort_unstable();
    nouns.dedup();
    assert_eq!(nouns.len(), 40);
}

#[test]
fn generated_hostname_is_valid() {
    for _ in 0..200 {
        let name = gen::generate();
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 3, "bad shape {name}");
        assert!(gen::ADJECTIVES.contains(&parts[0]));
        assert!(gen::NOUNS.contains(&parts[1]));
        assert_eq!(parts[2].len(), 4);
        assert!(parts[2]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        assert!(name.len() <= 63);
    }
}

fn frame(radiotap_len: u16, mac_header: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8, 0u8];
    out.extend_from_slice(&radiotap_len.to_le_bytes());
    out.resize(usize::from(radiotap_len), 0);
    out.extend_from_slice(mac_header);
    out
}

fn mac_header() -> Vec<u8> {
    let mut header = vec![0x40, 0x00, 0x00, 0x00];
    header.extend_from_slice(&[0xff; 6]);
    header.extend_from_slice(&[0x02, 0x11, 0x22, 0x33, 0x44, 0x66]);
    header
}

#[test]
fn src_mac_reads_transmitter_after_radiotap() {
    let bytes = frame(8, &mac_header());
    assert_eq!(src_mac(&bytes), Some([0x02, 0x11, 0x22, 0x33, 0x44, 0x66]));
    let longer = frame(24, &mac_header());
    assert_eq!(src_mac(&longer), Some([0x02, 0x11, 0x22, 0x33, 0x44, 0x66]));
}

#[test]
fn src_mac_rejects_short_and_inconsistent_frames() {
    assert_eq!(src_mac(&[]), None);
    assert_eq!(src_mac(&[0, 0, 8]), None);
    let header = mac_header();
    let truncated = frame(8, &header[..header.len() - 1]);
    assert_eq!(src_mac(&truncated), None);
    let mut oversized = frame(8, &header);
    oversized[2] = 0xff;
    oversized[3] = 0xff;
    assert_eq!(src_mac(&oversized), None);
}

#[test]
fn cap_eff_reads_effective_set_only() {
    let status = "Name:\tprivcrab\nCapBnd:\t000001ffffffffff\nCapEff:\t0000000000001000\n";
    assert_eq!(cap_eff(status), Some(0x1000));
    assert_eq!(cap_eff("CapBnd:\t000001ffffffffff\n"), None);
    assert_eq!(cap_eff("CapEff: 0000000000001000\n"), None);
    assert_eq!(cap_eff("CapEff:\tnot-hex\n"), None);
}

#[test]
fn state_round_trips_through_disk() {
    let store = Store::new("state-roundtrip");
    let state = saved_mac("wlan0", ORIGINAL);
    assert!(backup::save_to(&store.0, &state).is_ok());
    assert_eq!(backup::load_from(&store.0).unwrap(), state);
}

#[test]
fn missing_state_is_empty() {
    let store = Store::new("state-missing");
    assert!(backup::load_from(&store.0).unwrap().is_empty());
}

#[test]
fn corrupt_state_is_treated_as_empty() {
    let store = Store::new("state-corrupt");
    if let Some(dir) = store.0.parent() {
        assert!(fs::create_dir_all(dir).is_ok());
    }
    assert!(fs::write(&store.0, "{ not json").is_ok());
    assert!(backup::load_from(&store.0).unwrap().is_empty());
}

#[test]
fn partial_state_is_valid() {
    let parsed = State::from_json(r#"{"mac":{"iface":"wlan0","original":"00:11:22:33:44:55"}}"#);
    match parsed {
        Ok(state) => {
            assert!(state.mac.is_some());
            assert!(state.hostname.is_none());
            assert!(state.dns.is_none());
        }
        Err(err) => panic!("parse failed: {err}"),
    }
}

#[test]
fn spoof_saves_state_then_writes_mac() {
    let store = Store::new("spoof-ok");
    let mut nic = MockNic::new();
    nic.expect_read_mac()
        .times(1)
        .returning(|_| Ok(ORIGINAL.to_string()));
    nic.expect_read_mac()
        .times(1)
        .returning(|_| Ok(FORGED.to_string()));
    nic.expect_is_up().times(1).returning(|_| Ok(true));
    nic.expect_set_down().times(1).returning(|_| Ok(()));
    nic.expect_set_hwaddr()
        .times(1)
        .withf(|iface, mac| iface == "wlan0" && *mac == FORGED_BYTES)
        .returning(|_, _| Ok(()));
    nic.expect_set_up().times(1).returning(|_| Ok(()));
    let outcome = spoof_with(&nic, &store.0, "wlan0", FORGED, Duration::ZERO);
    assert_eq!(outcome.map_err(|err| err.to_string()), Ok(()));
    assert_eq!(backup::load_from(&store.0).unwrap(), saved_mac("wlan0", ORIGINAL));
}

#[test]
fn spoof_reports_driver_mismatch_and_releases_backup() {
    let store = Store::new("spoof-mismatch");
    let mut nic = MockNic::new();
    nic.expect_read_mac()
        .times(5)
        .returning(|_| Ok(ORIGINAL.to_string()));
    nic.expect_is_up().times(1).returning(|_| Ok(true));
    nic.expect_set_down().times(3).returning(|_| Ok(()));
    nic.expect_set_hwaddr().times(3).returning(|_, _| Ok(()));
    nic.expect_set_up().times(3).returning(|_| Ok(()));
    match spoof_with(&nic, &store.0, "wlan0", FORGED, Duration::ZERO) {
        Err(MacError::HwaddrMismatch { expected, actual }) => {
            assert_eq!(expected, FORGED);
            assert_eq!(actual, ORIGINAL);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert!(backup::load_from(&store.0).unwrap().mac.is_none());
}

#[test]
fn spoof_keeps_original_when_repeated_on_same_interface() {
    let store = Store::new("spoof-repeat");
    assert!(backup::save_to(&store.0, &saved_mac("wlan0", ORIGINAL)).is_ok());
    let mut nic = MockNic::new();
    nic.expect_read_mac()
        .times(1)
        .returning(|_| Ok(FORGED.to_string()));
    nic.expect_read_mac()
        .times(1)
        .returning(|_| Ok("02:00:00:00:00:01".to_string()));
    nic.expect_is_up().times(1).returning(|_| Ok(true));
    nic.expect_set_down().times(1).returning(|_| Ok(()));
    nic.expect_set_hwaddr().times(1).returning(|_, _| Ok(()));
    nic.expect_set_up().times(1).returning(|_| Ok(()));
    let outcome = spoof_with(&nic, &store.0, "wlan0", "02:00:00:00:00:01", Duration::ZERO);
    assert_eq!(outcome.map_err(|err| err.to_string()), Ok(()));
    assert_eq!(backup::load_from(&store.0).unwrap(), saved_mac("wlan0", ORIGINAL));
}

#[test]
fn spoof_refuses_second_interface_while_backup_is_pending() {
    let store = Store::new("spoof-pending");
    assert!(backup::save_to(&store.0, &saved_mac("wlan1", ORIGINAL)).is_ok());
    let mut nic = MockNic::new();
    nic.expect_read_mac()
        .times(1)
        .returning(|_| Ok(ORIGINAL.to_string()));
    nic.expect_is_up().times(1).returning(|_| Ok(true));
    match spoof_with(&nic, &store.0, "wlan0", FORGED, Duration::ZERO) {
        Err(MacError::BackupPending(other)) => assert_eq!(other, "wlan1"),
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert_eq!(backup::load_from(&store.0).unwrap(), saved_mac("wlan1", ORIGINAL));
}

#[test]
fn spoof_rejects_bad_input_before_touching_the_link() {
    let store = Store::new("spoof-bad");
    let nic = MockNic::new();
    for mac in ["zz", "01:00:5e:00:00:01", "00:00:00:00:00:00"] {
        let outcome = spoof_with(&nic, &store.0, "wlan0", mac, Duration::ZERO);
        assert!(matches!(outcome, Err(MacError::InvalidMac(_))), "accepted {mac}");
    }
    let outcome = spoof_with(&nic, &store.0, "../x", FORGED, Duration::ZERO);
    assert!(matches!(outcome, Err(MacError::InvalidInterface(_))));
}

#[test]
fn restore_writes_saved_mac_and_clears_state() {
    let store = Store::new("restore-ok");
    assert!(backup::save_to(&store.0, &saved_mac("wlan0", ORIGINAL)).is_ok());
    let mut nic = MockNic::new();
    nic.expect_set_down().times(1).returning(|_| Ok(()));
    nic.expect_set_hwaddr()
        .times(1)
        .withf(|iface, mac| iface == "wlan0" && *mac == ORIGINAL_BYTES)
        .returning(|_, _| Ok(()));
    nic.expect_set_up().times(1).returning(|_| Ok(()));
    nic.expect_read_mac()
        .times(1)
        .returning(|_| Ok(ORIGINAL.to_string()));
    let outcome = restore_with(&nic, &store.0, "wlan0", Duration::ZERO);
    assert_eq!(outcome.map_err(|err| err.to_string()), Ok(true));
    assert!(backup::load_from(&store.0).unwrap().mac.is_none());
}

#[test]
fn restore_without_matching_state_does_nothing() {
    let store = Store::new("restore-none");
    let nic = MockNic::new();
    let outcome = restore_with(&nic, &store.0, "wlan0", Duration::ZERO);
    assert_eq!(outcome.map_err(|err| err.to_string()), Ok(false));
    assert!(backup::save_to(&store.0, &saved_mac("wlan1", ORIGINAL)).is_ok());
    let outcome = restore_with(&nic, &store.0, "wlan0", Duration::ZERO);
    assert_eq!(outcome.map_err(|err| err.to_string()), Ok(false));
    assert_eq!(backup::load_from(&store.0).unwrap(), saved_mac("wlan1", ORIGINAL));
}

#[test]
fn binary_reports_version() {
    let (ok, out) = run_cli(&["--version"]);
    assert!(ok);
    assert!(out.contains("0.1.2"));
}
