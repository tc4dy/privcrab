pub mod doh;
pub mod dot;
pub mod providers;

use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use hickory_resolver::error::{ResolveError, ResolveErrorKind};
use hickory_resolver::TokioAsyncResolver;

use self::providers::Provider;
use crate::config::DnsBackup;
use crate::error::DnsError;
use crate::util::backup;
use crate::util::iface::is_valid_name;

const RESOLV: &str = "/etc/resolv.conf";
const RESOLV_TMP: &str = "/etc/resolv.conf.privcrab.tmp";
const SETTLE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Doh,
    Dot,
}

impl Protocol {
    pub fn label(self) -> &'static str {
        match self {
            Protocol::Doh => "DoH",
            Protocol::Dot => "DoT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvKind {
    PlainFile,
    SystemdResolved,
    NetworkManager,
    OtherSymlink,
}

impl ResolvKind {
    pub fn to_u8(self) -> u8 {
        match self {
            ResolvKind::PlainFile => 0,
            ResolvKind::SystemdResolved => 1,
            ResolvKind::NetworkManager => 2,
            ResolvKind::OtherSymlink => 3,
        }
    }

    pub fn from_u8(value: u8) -> ResolvKind {
        match value {
            1 => ResolvKind::SystemdResolved,
            2 => ResolvKind::NetworkManager,
            3 => ResolvKind::OtherSymlink,
            _ => ResolvKind::PlainFile,
        }
    }
}

pub fn kind_of(target: &str) -> ResolvKind {
    if target.contains("systemd") {
        ResolvKind::SystemdResolved
    } else if target.contains("NetworkManager") {
        ResolvKind::NetworkManager
    } else {
        ResolvKind::OtherSymlink
    }
}

pub fn resolv_conf_kind() -> ResolvKind {
    match fs::read_link(RESOLV) {
        Err(_) => ResolvKind::PlainFile,
        Ok(target) => kind_of(&target.to_string_lossy()),
    }
}

pub fn render_resolv(ips: &[IpAddr]) -> String {
    let servers: String = ips.iter().map(|ip| format!("nameserver {ip}\n")).collect();
    servers + "options edns0\n"
}

pub fn parse_default_iface(text: &str) -> Option<String> {
    text.lines().skip(1).find_map(|line| {
        let mut fields = line.split_whitespace();
        let name = fields.next()?;
        let destination = fields.next()?;
        (destination == "00000000").then(|| name.to_string())
    })
}

pub fn parse_dns_key(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("dns="))
        .map(|value| value.trim().to_string())
}

pub async fn set(provider: &Provider, protocol: Protocol) -> Result<(), DnsError> {
    match protocol {
        Protocol::Doh => doh::set_doh(provider).await,
        Protocol::Dot => dot::set_dot(provider).await,
    }
}

pub async fn test() -> Result<Vec<IpAddr>, DnsError> {
    system_lookup("one.one.one.one", "system resolver").await
}

pub fn restore() -> Result<bool, DnsError> {
    let state = backup::load()?;
    let saved = match state.dns {
        Some(saved) => saved,
        None => return Ok(false),
    };
    match &saved.iface {
        Some(iface) => {
            if !is_valid_name(iface) {
                return Err(DnsError::CommandFailed("resolvectl revert".to_string()));
            }
            resolvectl(&["revert", iface])?;
        }
        None => {
            let kind = saved
                .kind
                .map(ResolvKind::from_u8)
                .unwrap_or(ResolvKind::PlainFile);
            write_resolv(kind, &saved.resolv_contents)?;
        }
    }
    backup::update(|state| state.dns = None)?;
    Ok(true)
}

pub(crate) async fn check_with(
    resolver: &TokioAsyncResolver,
    name: &str,
    label: &str,
) -> Result<Vec<IpAddr>, DnsError> {
    let found = resolver
        .lookup_ip(name)
        .await
        .map_err(|err| classify(&err, label))?;
    let ips: Vec<IpAddr> = found.iter().collect();
    if ips.is_empty() {
        return Err(DnsError::InvalidResponse);
    }
    Ok(ips)
}

pub(crate) async fn apply(provider: &Provider, protocol: Protocol) -> Result<(), DnsError> {
    let kind = resolv_conf_kind();
    if protocol == Protocol::Doh && kind != ResolvKind::SystemdResolved {
        return Err(DnsError::CommandFailed(
            "DoH requires a local forwarder; use --protocol dot or systemd-resolved".to_string(),
        ));
    }
    let contents = fs::read_to_string(RESOLV).map_err(DnsError::SystemResolverWrite)?;
    let iface = match kind {
        ResolvKind::SystemdResolved => Some(default_iface()?),
        _ => None,
    };
    let entry = DnsBackup {
        resolv_contents: contents,
        iface: iface.clone(),
        kind: Some(kind.to_u8()),
    };
    backup::update(|state| {
        if state.dns.is_none() {
            state.dns = Some(entry);
        }
    })?;
    let written = write_config(kind, provider, protocol, iface.as_deref());
    let outcome = match written {
        Ok(()) => {
            tokio::time::sleep(SETTLE).await;
            system_lookup(provider.dot_host, provider.name)
                .await
                .map(|_| ())
        }
        Err(err) => Err(err),
    };
    if let Err(err) = outcome {
        if let Err(undo) = restore() {
            tracing::debug!("{undo:?}");
            eprintln!("[!] rollback failed: {undo}");
        }
        return Err(err);
    }
    Ok(())
}

fn classify(err: &ResolveError, label: &str) -> DnsError {
    tracing::debug!("{err:?}");
    match err.kind() {
        ResolveErrorKind::Timeout | ResolveErrorKind::NoConnections => {
            DnsError::ProviderUnreachable(label.to_string())
        }
        ResolveErrorKind::NoRecordsFound { .. } => DnsError::InvalidResponse,
        _ => DnsError::TlsHandshake(label.to_string()),
    }
}

fn system_resolver() -> Result<TokioAsyncResolver, DnsError> {
    TokioAsyncResolver::tokio_from_system_conf().map_err(|err| {
        tracing::debug!("{err:?}");
        DnsError::CommandFailed("system resolver setup".to_string())
    })
}

async fn system_lookup(name: &str, label: &str) -> Result<Vec<IpAddr>, DnsError> {
    let resolver = system_resolver()?;
    check_with(&resolver, name, label).await
}

fn default_iface() -> Result<String, DnsError> {
    let text = fs::read_to_string("/proc/net/route").map_err(DnsError::SystemResolverWrite)?;
    parse_default_iface(&text)
        .filter(|name| is_valid_name(name))
        .ok_or_else(|| DnsError::CommandFailed("default route lookup".to_string()))
}

fn resolvectl(args: &[&str]) -> Result<(), DnsError> {
    let output = Command::new("resolvectl").args(args).output().map_err(|err| {
        tracing::debug!("{err:?}");
        DnsError::CommandFailed("resolvectl".to_string())
    })?;
    if output.status.success() {
        return Ok(());
    }
    tracing::debug!("{}", String::from_utf8_lossy(&output.stderr));
    let verb = args.first().copied().unwrap_or("");
    Err(DnsError::CommandFailed(format!("resolvectl {verb}")))
}

fn push_resolved(iface: &str, provider: &Provider, protocol: Protocol) -> Result<(), DnsError> {
    let mut dns = vec!["dns".to_string(), iface.to_string()];
    for ip in provider.ips {
        dns.push(match protocol {
            Protocol::Dot => format!("{ip}#{}", provider.dot_host),
            Protocol::Doh => ip.to_string(),
        });
    }
    let refs: Vec<&str> = dns.iter().map(String::as_str).collect();
    resolvectl(&refs)?;
    resolvectl(&["domain", iface, "~."])?;
    let mode = match protocol {
        Protocol::Dot => "yes",
        Protocol::Doh => "no",
    };
    resolvectl(&["dnsovertls", iface, mode])
}

fn nm_dns_mode() -> String {
    let mut files = vec![PathBuf::from("/etc/NetworkManager/NetworkManager.conf")];
    if let Ok(entries) = fs::read_dir("/etc/NetworkManager/conf.d") {
        let mut extra: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("conf"))
            .collect();
        extra.sort();
        files.extend(extra);
    }
    let mut mode = "default".to_string();
    for file in files {
        if let Some(value) = fs::read_to_string(&file)
            .ok()
            .as_deref()
            .and_then(parse_dns_key)
        {
            mode = value;
        }
    }
    mode
}

fn write_resolv(kind: ResolvKind, contents: &str) -> Result<(), DnsError> {
    match kind {
        ResolvKind::PlainFile => {
            if let Err(err) = fs::write(RESOLV_TMP, contents) {
                let _ = fs::remove_file(RESOLV_TMP);
                return Err(DnsError::SystemResolverWrite(err));
            }
            match fs::rename(RESOLV_TMP, RESOLV) {
                Ok(()) => Ok(()),
                Err(err)
                    if err.raw_os_error() == Some(libc::EBUSY)
                        || err.raw_os_error() == Some(libc::EXDEV) =>
                {
                    let _ = fs::remove_file(RESOLV_TMP);
                    write_in_place(RESOLV, contents)
                }
                Err(err) => {
                    let _ = fs::remove_file(RESOLV_TMP);
                    Err(DnsError::SystemResolverWrite(err))
                }
            }
        }
        ResolvKind::NetworkManager | ResolvKind::OtherSymlink => write_in_place(RESOLV, contents),
        ResolvKind::SystemdResolved => {
            Err(DnsError::CommandFailed("resolv.conf update".to_string()))
        }
    }
}

fn write_in_place(path: &str, contents: &str) -> Result<(), DnsError> {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, contents).map_err(DnsError::SystemResolverWrite)?;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o644));
    Ok(())
}

fn write_config(
    kind: ResolvKind,
    provider: &Provider,
    protocol: Protocol,
    iface: Option<&str>,
) -> Result<(), DnsError> {
    if let (ResolvKind::SystemdResolved, Some(iface)) = (kind, iface) {
        eprintln!("[!] /etc/resolv.conf is managed by systemd-resolved, using resolvectl on {iface}");
        if protocol == Protocol::Doh {
            eprintln!(
                "[!] systemd-resolved has no DoH support, queries to {} are sent unencrypted",
                provider.name
            );
        }
        return push_resolved(iface, provider, protocol);
    }
    if kind == ResolvKind::NetworkManager && nm_dns_mode() != "none" {
        eprintln!("[!] NetworkManager may overwrite resolv.conf on reconnect, set dns=none in NetworkManager.conf");
    }
    eprintln!(
        "[!] resolv.conf cannot carry {}, queries to {} are sent unencrypted",
        protocol.label(),
        provider.name
    );
    write_resolv(kind, &render_resolv(provider.ips))
}