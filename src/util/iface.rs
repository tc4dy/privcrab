use std::fs;
use std::io;
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::path::Path;
use std::process::Command;

use nix::errno::Errno;
use nix::sys::socket::{socket, AddressFamily, SockFlag, SockType};

use crate::error::MacError;

const IFF_UP: i16 = 0x1;

#[allow(dead_code)]
#[repr(C)]
#[derive(Clone, Copy)]
struct HwAddr {
    family: u16,
    data: [u8; 14],
}

#[allow(dead_code)]
#[repr(C)]
union IfData {
    flags: i16,
    hwaddr: HwAddr,
    pad: [u64; 3],
}

#[allow(dead_code)]
#[repr(C)]
struct IfReq {
    name: [u8; 16],
    data: IfData,
}

mod raw {
    use super::IfReq;

    nix::ioctl_read_bad!(get_flags, libc::SIOCGIFFLAGS, IfReq);
    nix::ioctl_write_ptr_bad!(set_flags, libc::SIOCSIFFLAGS, IfReq);
    nix::ioctl_read_bad!(get_hwaddr, libc::SIOCGIFHWADDR, IfReq);
    nix::ioctl_write_ptr_bad!(set_hwaddr, libc::SIOCSIFHWADDR, IfReq);
}

pub trait Link {
    fn read_mac(&self, iface: &str) -> Result<String, MacError>;
    fn is_up(&self, iface: &str) -> Result<bool, MacError>;
    fn set_down(&self, iface: &str) -> Result<(), MacError>;
    fn set_up(&self, iface: &str) -> Result<(), MacError>;
    fn set_hwaddr(&self, iface: &str, mac: [u8; 6]) -> Result<(), MacError>;
}

pub struct SysLink;

impl Link for SysLink {
    fn read_mac(&self, iface: &str) -> Result<String, MacError> {
        read_mac(iface)
    }

    fn is_up(&self, iface: &str) -> Result<bool, MacError> {
        is_up(iface)
    }

    fn set_down(&self, iface: &str) -> Result<(), MacError> {
        set_down(iface)
    }

    fn set_up(&self, iface: &str) -> Result<(), MacError> {
        set_up(iface)
    }

    fn set_hwaddr(&self, iface: &str, mac: [u8; 6]) -> Result<(), MacError> {
        set_hwaddr(iface, mac)
    }
}

pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 15
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn validate_name(name: &str) -> Result<(), MacError> {
    if is_valid_name(name) {
        Ok(())
    } else {
        Err(MacError::InvalidInterface(name.to_string()))
    }
}

pub fn parse_net_dev(text: &str) -> Vec<String> {
    text.lines()
        .skip(2)
        .filter_map(|line| line.split_once(':'))
        .map(|(name, _)| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

pub fn list() -> io::Result<Vec<String>> {
    let text = fs::read_to_string("/proc/net/dev")?;
    Ok(parse_net_dev(&text))
}

pub fn list_interfaces_with_mac() -> Result<Vec<(String, String)>, MacError> {
    let base = Path::new("/sys/class/net");
    let entries = fs::read_dir(base).map_err(MacError::ListInterfacesFailed)?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(MacError::ListInterfacesFailed)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_valid_name(&name) {
            continue;
        }
        let mac_path = base.join(&name).join("address");
        if let Ok(mac) = fs::read_to_string(mac_path) {
            out.push((name, mac.trim().to_ascii_lowercase()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn exists(name: &str) -> bool {
    is_valid_name(name) && fs::metadata(format!("/sys/class/net/{name}")).is_ok()
}

pub fn read_mac(iface: &str) -> Result<String, MacError> {
    validate_name(iface)?;
    match fs::read_to_string(format!("/sys/class/net/{iface}/address")) {
        Ok(text) => Ok(text.trim().to_ascii_lowercase()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            Err(MacError::InterfaceNotFound(iface.to_string()))
        }
        Err(err) => Err(MacError::ReadHwaddr(err)),
    }
}

pub fn is_up(iface: &str) -> Result<bool, MacError> {
    validate_name(iface)?;
    let sock = open_socket().map_err(|err| classify(err, iface, MacError::IfaceDown))?;
    let fd = sock.as_raw_fd();
    let mut req = make_req(iface);
    let flags = read_flags(fd, &mut req).map_err(|err| classify(err, iface, MacError::IfaceDown))?;
    Ok(flags & IFF_UP != 0)
}

pub fn set_down(iface: &str) -> Result<(), MacError> {
    validate_name(iface)?;
    change_flags(iface, false).map_err(|err| classify(err, iface, MacError::IfaceDown))
}

pub fn set_up(iface: &str) -> Result<(), MacError> {
    validate_name(iface)?;
    change_flags(iface, true).map_err(|err| classify(err, iface, MacError::IfaceUp))
}

pub fn set_hwaddr(iface: &str, mac: [u8; 6]) -> Result<(), MacError> {
    validate_name(iface)?;
    let sock = open_socket().map_err(|err| classify(err, iface, MacError::WriteHwaddr))?;
    let fd = sock.as_raw_fd();
    let mut req = make_req(iface);
    let family =
        read_family(fd, &mut req).map_err(|err| classify(err, iface, MacError::WriteHwaddr))?;
    let mut data = [0u8; 14];
    data[..6].copy_from_slice(&mac);
    req.data.hwaddr = HwAddr { family, data };
    write_hwaddr(fd, &req).map_err(|err| classify(err, iface, MacError::WriteHwaddr))
}

fn classify(err: Errno, iface: &str, other: fn(Errno) -> MacError) -> MacError {
    match err {
        Errno::EPERM | Errno::EACCES => MacError::PermissionDenied,
        Errno::ENODEV => MacError::InterfaceNotFound(iface.to_string()),
        err => other(err),
    }
}

fn make_req(iface: &str) -> IfReq {
    let mut name = [0u8; 16];
    for (slot, byte) in name.iter_mut().zip(iface.bytes().take(15)) {
        *slot = byte;
    }
    IfReq {
        name,
        data: IfData { pad: [0; 3] },
    }
}

fn open_socket() -> Result<OwnedFd, Errno> {
    socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::SOCK_CLOEXEC,
        None,
    )
}

fn read_flags(fd: RawFd, req: &mut IfReq) -> Result<i16, Errno> {
    unsafe { raw::get_flags(fd, req) }?;
    let flags = unsafe { req.data.flags };
    Ok(flags)
}

fn read_family(fd: RawFd, req: &mut IfReq) -> Result<u16, Errno> {
    unsafe { raw::get_hwaddr(fd, req) }?;
    let family = unsafe { req.data.hwaddr.family };
    Ok(family)
}

fn write_flags(fd: RawFd, req: &IfReq) -> Result<(), Errno> {
    unsafe { raw::set_flags(fd, req) }.map(|_| ())
}

fn write_hwaddr(fd: RawFd, req: &IfReq) -> Result<(), Errno> {
    unsafe { raw::set_hwaddr(fd, req) }.map(|_| ())
}

fn change_flags(iface: &str, up: bool) -> Result<(), Errno> {
    let sock = open_socket()?;
    let fd = sock.as_raw_fd();
    let mut req = make_req(iface);
    let current = read_flags(fd, &mut req)?;
    let wanted = if up {
        current | IFF_UP
    } else {
        current & !IFF_UP
    };
    req.data.flags = wanted;
    write_flags(fd, &req)
}

pub fn active_network_manager() -> Option<&'static str> {
    let managers = [
        ("NetworkManager", "systemctl is-active NetworkManager"),
        ("iwd", "systemctl is-active iwd"),
        ("systemd-networkd", "systemctl is-active systemd-networkd"),
    ];
    for (name, cmd) in managers {
        let output = Command::new("sh").arg("-c").arg(cmd).output().ok()?;
        if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "active" {
            return Some(name);
        }
    }
    None
}

pub fn warn_about_conflicts(iface: &str) {
    if let Some(manager) = active_network_manager() {
        match manager {
            "NetworkManager" => {
                eprintln!(
                    "[!] NetworkManager is active and may overwrite the MAC address on reconnect."
                );
                eprintln!(
                    "[>] Consider setting `wifi.cloned-mac-address=stable` or `ethernet.cloned-mac-address=stable` in NetworkManager."
                );
            }
            "iwd" => {
                eprintln!(
                    "[!] iwd is active and may randomize MAC addresses independently."
                );
                eprintln!(
                    "[>] Set `AddressRandomization=disabled` in /etc/iwd/main.conf to avoid conflicts."
                );
            }
            "systemd-networkd" => {
                eprintln!(
                    "[!] systemd-networkd is active and may apply MACAddressPolicy."
                );
                eprintln!(
                    "[>] Create a .link file with `MACAddressPolicy=none` for {iface} to prevent conflicts."
                );
            }
            _ => {}
        }
    }
}
