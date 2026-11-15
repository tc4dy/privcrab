pub mod block;

use std::fmt::Debug;

use netlink_wi::interface::InterfaceType;
use netlink_wi::NlSocket;

use crate::error::ProbeError;
use crate::util::iface::{is_valid_name, set_down, set_up};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Monitor,
    Managed,
    Other,
}

pub fn check_name(iface: &str) -> Result<(), ProbeError> {
    if is_valid_name(iface) {
        Ok(())
    } else {
        Err(ProbeError::InvalidInterface(iface.to_string()))
    }
}

pub fn mode(iface: &str) -> Result<Mode, ProbeError> {
    check_name(iface)?;
    let socket = NlSocket::connect().map_err(|err| netlink_failure(iface, err))?;
    Ok(find(&socket, iface)?.1)
}

pub fn enter_monitor(iface: &str) -> Result<(), ProbeError> {
    if mode(iface)? == Mode::Monitor {
        return Ok(());
    }
    println!("[*] switching {iface} to monitor mode");
    switch(iface, InterfaceType::Monitor)?;
    if mode(iface)? == Mode::Monitor {
        Ok(())
    } else {
        Err(ProbeError::MonitorModeFailed(iface.to_string()))
    }
}

pub fn leave_monitor(iface: &str) -> Result<bool, ProbeError> {
    if mode(iface)? != Mode::Monitor {
        return Ok(false);
    }
    switch(iface, InterfaceType::Station)?;
    if mode(iface)? == Mode::Managed {
        Ok(true)
    } else {
        Err(ProbeError::MonitorModeFailed(iface.to_string()))
    }
}

fn netlink_failure<E: Debug>(iface: &str, err: E) -> ProbeError {
    tracing::debug!("{err:?}");
    ProbeError::MonitorModeFailed(iface.to_string())
}

fn find(socket: &NlSocket, iface: &str) -> Result<(u32, Mode), ProbeError> {
    let list = socket
        .list_interfaces()
        .map_err(|err| netlink_failure(iface, err))?;
    let found = list
        .into_iter()
        .find(|entry| entry.name == iface)
        .ok_or_else(|| ProbeError::MonitorModeFailed(iface.to_string()))?;
    let mode = match found.interface_type {
        Some(InterfaceType::Monitor) => Mode::Monitor,
        Some(InterfaceType::Station) => Mode::Managed,
        _ => Mode::Other,
    };
    Ok((found.interface_index, mode))
}

fn switch(iface: &str, target: InterfaceType) -> Result<(), ProbeError> {
    let socket = NlSocket::connect().map_err(|err| netlink_failure(iface, err))?;
    let (index, _) = find(&socket, iface)?;
    set_down(iface)?;
    let changed = socket
        .set_interface(index, target)
        .map_err(|err| netlink_failure(iface, err));
    let raised = set_up(iface);
    changed?;
    raised?;
    Ok(())
}