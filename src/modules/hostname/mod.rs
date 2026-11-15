pub mod gen;

use std::fs;
use std::path::Path;

use nix::unistd::{gethostname, sethostname};

use crate::config::HostnameBackup;
use crate::error::HostnameError;
use crate::util::backup;

const HOSTNAME_FILE: &str = "/etc/hostname";
const HOSTS_FILE: &str = "/etc/hosts";
const AVAHI_PID: &str = "/var/run/avahi-daemon/pid";

pub fn randomize() -> Result<String, HostnameError> {
    let current = gethostname()
        .map_err(HostnameError::ReadFailed)?
        .to_string_lossy()
        .into_owned();
    let mut created = false;
    backup::update(|state| {
        if state.hostname.is_none() {
            state.hostname = Some(HostnameBackup {
                original: current.clone(),
            });
            created = true;
        }
    })?;
    let name = gen::generate();
    if let Err(err) = sethostname(&name) {
        if created {
            let _ = backup::update(|state| state.hostname = None);
        }
        return Err(HostnameError::SetFailed(err));
    }
    persist(&name);
    hosts_notice(&current, &name);
    avahi_notice();
    Ok(name)
}

pub fn restore() -> Result<bool, HostnameError> {
    let state = backup::load()?;
    let saved = match state.hostname {
        Some(saved) => saved,
        None => return Ok(false),
    };
    sethostname(&saved.original).map_err(HostnameError::SetFailed)?;
    persist(&saved.original);
    backup::update(|state| state.hostname = None)?;
    avahi_notice();
    Ok(true)
}

fn persist(name: &str) {
    if let Err(err) = fs::write(HOSTNAME_FILE, format!("{name}\n")) {
        tracing::debug!("{err:?}");
        eprintln!("[!] could not write {HOSTNAME_FILE}: the change will not survive a reboot");
    }
}

fn hosts_notice(old: &str, new: &str) {
    match fs::read_to_string(HOSTS_FILE) {
        Ok(text) if text.contains(old) => {
            eprintln!("[!] {HOSTS_FILE} still references {old}");
            eprintln!("[>] update it to avoid sudo delays and GUI auth issues");
            let _ = new;
        }
        Ok(_) => {}
        Err(err) => tracing::debug!("{err:?}"),
    }
}

fn avahi_notice() {
    if Path::new(AVAHI_PID).exists() {
        eprintln!("[!] avahi-daemon is running — restart it to update mDNS: systemctl restart avahi-daemon");
    }
}