use std::io;

use thiserror::Error;

#[cfg(target_os = "linux")]
pub type SysError = nix::Error;
#[cfg(not(target_os = "linux"))]
pub type SysError = io::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Mac(#[from] MacError),
    #[error(transparent)]
    Dns(#[from] DnsError),
    #[error(transparent)]
    Leak(#[from] LeakError),
    #[error(transparent)]
    Hostname(#[from] HostnameError),
    #[error(transparent)]
    Probe(#[from] ProbeError),
    #[error(transparent)]
    Backup(#[from] BackupError),
    #[error(transparent)]
    Priv(#[from] PrivError),
    #[error(transparent)]
    Config(#[from] ConfigError),
}

#[derive(Debug, Error)]
pub enum MacError {
    #[error("interface {0} not found")]
    InterfaceNotFound(String),
    #[error("invalid interface name: {0}")]
    InvalidInterface(String),
    #[error("permission denied")]
    PermissionDenied,
    #[error("invalid MAC address: {0}")]
    InvalidMac(String),
    #[error("could not bring interface down")]
    IfaceDown(#[source] SysError),
    #[error("could not bring interface up")]
    IfaceUp(#[source] SysError),
    #[error("could not write hardware address")]
    WriteHwaddr(#[source] SysError),
    #[error("could not read hardware address")]
    ReadHwaddr(#[source] io::Error),
    #[error("driver reports {actual} after writing {expected}")]
    HwaddrMismatch { expected: String, actual: String },
    #[error("saved MAC for {0} must be restored first")]
    BackupPending(String),
    #[error(transparent)]
    Backup(#[from] BackupError),
}

#[derive(Debug, Error)]
pub enum DnsError {
    #[error("provider {0} is unreachable")]
    ProviderUnreachable(String),
    #[error("TLS handshake with {0} failed")]
    TlsHandshake(String),
    #[error("resolver returned an invalid response")]
    InvalidResponse,
    #[error("could not write system resolver configuration")]
    SystemResolverWrite(#[source] io::Error),
    #[error("{0} failed")]
    CommandFailed(String),
    #[error(transparent)]
    Backup(#[from] BackupError),
}

#[derive(Debug, Error)]
pub enum LeakError {
    #[error("request failed")]
    HttpFailed(#[from] reqwest::Error),
    #[error("all endpoints timed out")]
    Timeout,
    #[error("no IPv6 route available")]
    Ipv6Unroutable,
    #[error("resolver query failed: {0}")]
    DnsQueryFailed(String),
}

#[derive(Debug, Error)]
pub enum HostnameError {
    #[error("could not read current hostname")]
    ReadFailed(#[source] SysError),
    #[error("could not set hostname")]
    SetFailed(#[source] SysError),
    #[error("could not write /etc/hostname")]
    WriteFailed(#[source] io::Error),
    #[error(transparent)]
    Backup(#[from] BackupError),
}

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("invalid interface name: {0}")]
    InvalidInterface(String),
    #[error("monitor mode failed on {0}")]
    MonitorModeFailed(String),
    #[error("could not open capture on {0}")]
    CaptureOpen(String),
    #[error("could not apply capture filter on {0}")]
    FilterSet(String),
    #[error("could not wait for interrupt signal")]
    Signal(#[source] io::Error),
    #[error(transparent)]
    Mac(#[from] MacError),
}

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("could not read saved state")]
    ReadFailed(#[source] io::Error),
    #[error("could not write saved state")]
    WriteFailed(#[source] io::Error),
    #[error("saved state is corrupt: {0}")]
    Corrupt(String),
}

#[derive(Debug, Error)]
pub enum PrivError {
    #[error("root privileges required")]
    RootRequired,
    #[error("CAP_NET_ADMIN capability missing")]
    CapNetAdminMissing,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not parse state file")]
    ParseFailed(#[source] serde_json::Error),
    #[error("state directory unavailable")]
    PathUnavailable,
}
