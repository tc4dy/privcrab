use std::fs;
use std::process::Command;

use crate::error::ServiceError;

const SERVICE_PATH: &str = "/etc/systemd/system/privcrab-mac.service";

pub fn install(iface: &str) -> Result<(), ServiceError> {
    let content = format!(
        r#"[Unit]
Description=privcrab persistent MAC randomization
After=network-pre.target
Before=network.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/privcrab mac spoof --iface {iface}
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
"#
    );

    fs::write(SERVICE_PATH, content).map_err(ServiceError::WriteFailed)?;

    let output = Command::new("systemctl")
        .args(["daemon-reload"])
        .output()
        .map_err(|_| ServiceError::SystemdUnavailable)?;
    if !output.status.success() {
        return Err(ServiceError::EnableFailed(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }

    let output = Command::new("systemctl")
        .args(["enable", "--now", "privcrab-mac.service"])
        .output()
        .map_err(|_| ServiceError::SystemdUnavailable)?;
    if !output.status.success() {
        return Err(ServiceError::EnableFailed(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }

    Ok(())
}

pub fn remove() -> Result<bool, ServiceError> {
    if fs::metadata(SERVICE_PATH).is_err() {
        return Ok(false);
    }

    let _ = Command::new("systemctl")
        .args(["disable", "--now", "privcrab-mac.service"])
        .output();

    let output = Command::new("systemctl")
        .arg("daemon-reload")
        .output()
        .map_err(|_| ServiceError::SystemdUnavailable)?;
    if !output.status.success() {
        return Err(ServiceError::DisableFailed(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }

    fs::remove_file(SERVICE_PATH).map_err(ServiceError::WriteFailed)?;
    Ok(true)
}
