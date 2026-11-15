use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::config::{state_path, MacBackup};
use crate::error::MacError;
use crate::modules::mac::{format_mac, parse_mac};
use crate::util::backup;
use crate::util::iface::{validate_name, Link, SysLink};

const SETTLE: Duration = Duration::from_millis(150);

pub fn spoof(iface: &str, mac: &str) -> Result<(), MacError> {
    spoof_with(&SysLink, &state_path(), iface, mac, SETTLE)
}

pub fn restore(iface: &str) -> Result<bool, MacError> {
    restore_with(&SysLink, &state_path(), iface, SETTLE)
}

pub fn spoof_with<L: Link>(
    link: &L,
    store: &Path,
    iface: &str,
    mac: &str,
    settle: Duration,
) -> Result<(), MacError> {
    let wanted = parse_target(mac)?;
    validate_name(iface)?;
    let original = link.read_mac(iface)?;
    let was_up = link.is_up(iface).unwrap_or(true);
    let created = reserve(store, iface, &original)?;

    let mut guard = MacGuard::new(link, iface, was_up);
    let outcome = write_mac(link, iface, wanted, settle, was_up);
    guard.disarm();

    if outcome.is_err() && created {
        release_if_unchanged(link, store, iface, &original);
    }

    outcome
}

pub fn restore_with<L: Link>(
    link: &L,
    store: &Path,
    iface: &str,
    settle: Duration,
) -> Result<bool, MacError> {
    validate_name(iface)?;
    let state = backup::load_from(store)?;
    let saved = match state.mac {
        Some(saved) if saved.iface == iface => saved,
        _ => return Ok(false),
    };
    let wanted = parse_mac(&saved.original)
        .ok_or_else(|| MacError::InvalidMac(saved.original.clone()))?;
    write_mac(link, iface, wanted, settle, true)?;
    backup::update_at(store, |state| state.mac = None)?;
    Ok(true)
}

struct MacGuard<'a, L: Link> {
    link: &'a L,
    iface: &'a str,
    was_up: bool,
    armed: bool,
}

impl<'a, L: Link> MacGuard<'a, L> {
    fn new(link: &'a L, iface: &'a str, was_up: bool) -> Self {
        Self {
            link,
            iface,
            was_up,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl<'a, L: Link> Drop for MacGuard<'a, L> {
    fn drop(&mut self) {
        if self.armed {
            if !self.was_up {
                let _ = self.link.set_down(self.iface);
            } else {
                let _ = self.link.set_up(self.iface);
            }
        }
    }
}

fn parse_target(mac: &str) -> Result<[u8; 6], MacError> {
    let bytes = parse_mac(mac).ok_or_else(|| MacError::InvalidMac(mac.to_string()))?;
    if bytes[0] & 1 == 1 || bytes == [0u8; 6] {
        return Err(MacError::InvalidMac(mac.to_string()));
    }
    Ok(bytes)
}

fn reserve(store: &Path, iface: &str, original: &str) -> Result<bool, MacError> {
    let mut state = backup::load_from(store)?;
    if let Some(saved) = &state.mac {
        if saved.iface == iface {
            return Ok(false);
        }
        return Err(MacError::BackupPending(saved.iface.clone()));
    }
    state.mac = Some(MacBackup {
        iface: iface.to_string(),
        original: original.to_string(),
    });
    backup::save_to(store, &state)?;
    Ok(true)
}

fn release_if_unchanged<L: Link>(link: &L, store: &Path, iface: &str, original: &str) {
    if link.read_mac(iface).is_ok_and(|now| now == original) {
        let _ = backup::update_at(store, |state| state.mac = None);
    }
}

fn write_mac<L: Link>(
    link: &L,
    iface: &str,
    wanted: [u8; 6],
    settle: Duration,
    was_up: bool,
) -> Result<(), MacError> {
    let expected = format_mac(&wanted);
    let mut actual = String::new();
    for _ in 0..3 {
        link.set_down(iface)?;
        let wrote = link.set_hwaddr(iface, wanted);
        let raised = if was_up { link.set_up(iface) } else { Ok(()) };
        wrote?;
        raised?;
        thread::sleep(settle);
        actual = link.read_mac(iface)?;
        if actual == expected {
            return Ok(());
        }
    }
    Err(MacError::HwaddrMismatch { expected, actual })
}