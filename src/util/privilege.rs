use std::fs;

use nix::unistd::geteuid;

use crate::error::PrivError;

const CAP_NET_ADMIN: u64 = 1 << 12;
const CAP_NET_RAW: u64 = 1 << 13;

pub fn cap_eff(status: &str) -> Option<u64> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("CapEff:\t"))
        .and_then(|value| u64::from_str_radix(value.trim(), 16).ok())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    None,
    NetAdmin,
    NetAdminRaw,
    Root,
}

impl Need {
    pub fn check(self) -> Result<(), PrivError> {
        match self {
            Need::None => Ok(()),
            Need::Root => {
                if geteuid().is_root() {
                    Ok(())
                } else {
                    Err(PrivError::RootRequired)
                }
            }
            Need::NetAdmin => require_caps(CAP_NET_ADMIN),
            Need::NetAdminRaw => require_caps(CAP_NET_ADMIN | CAP_NET_RAW),
        }
    }

    pub fn require(self) {
        if self.check().is_ok() {
            return;
        }
        match self {
            Need::Root => {
                eprintln!("[!] this command requires root");
                eprintln!("[>] run with sudo");
            }
            Need::NetAdmin => {
                eprintln!("[!] CAP_NET_ADMIN is required");
                eprintln!("[>] run as root or: sudo setcap cap_net_admin+ep ./privcrab");
            }
            Need::NetAdminRaw => {
                eprintln!("[!] CAP_NET_ADMIN and CAP_NET_RAW are required");
                eprintln!("[>] run as root or: sudo setcap cap_net_admin,cap_net_raw+ep ./privcrab");
            }
            Need::None => {}
        }
        std::process::exit(1);
    }
}

fn require_caps(mask: u64) -> Result<(), PrivError> {
    if geteuid().is_root() {
        return Ok(());
    }
    let status =
        fs::read_to_string("/proc/self/status").map_err(|_| PrivError::CapNetAdminMissing)?;
    match cap_eff(&status) {
        Some(bits) if bits & mask == mask => Ok(()),
        _ => Err(PrivError::CapNetAdminMissing),
    }
}