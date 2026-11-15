use std::future::Future;

use anyhow::Context;
use clap::{ColorChoice, Parser, Subcommand};
#[cfg(target_os = "linux")]
use clap::ValueEnum;

use privcrab::error::{AppError, ProbeError};
#[cfg(target_os = "linux")]
use privcrab::error::MacError;
#[cfg(target_os = "linux")]
use privcrab::modules::dns::providers::{Provider, PROVIDERS};
#[cfg(target_os = "linux")]
use privcrab::modules::dns::{self, Protocol};
#[cfg(target_os = "linux")]
use privcrab::modules::hostname;
use privcrab::modules::leak;
#[cfg(target_os = "linux")]
use privcrab::modules::mac::{oui, spoof};
#[cfg(target_os = "linux")]
use privcrab::modules::probe::{self, Mode};
#[cfg(target_os = "linux")]
use privcrab::util::privilege::Need;
use privcrab::style::{bold_crab, bold_purple, bold_turquoise, green, info, note, ok, turquoise, warn};

#[derive(Parser)]
#[command(
    name = "privcrab",
    version,
    about = "Network privacy hardening toolkit",
    color = ColorChoice::Always,
    long_about = "privcrab is a Linux-first CLI for network privacy hardening.\n\nSpoof or restore MAC addresses, switch the system resolver to DoH/DoT,\nrandomize hostnames, detect IPv4/IPv6/DNS leaks, and monitor probe requests.\n\nRun 'privcrab <command> --help' for details on a specific command."
)]
pub struct Cli {
    #[arg(long, global = true, help = "Print debug output")]
    pub verbose: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[cfg(target_os = "linux")]
    #[command(about = "Change or restore the hardware address of an interface")]
    Mac {
        #[command(subcommand)]
        action: MacAction,
    },
    #[cfg(target_os = "linux")]
    #[command(about = "Switch the system resolver to a DoH or DoT provider")]
    Dns {
        #[command(subcommand)]
        action: DnsAction,
    },
    #[command(about = "Check which addresses are visible from outside")]
    Leak {
        #[command(subcommand)]
        action: LeakAction,
    },
    #[cfg(target_os = "linux")]
    #[command(about = "Randomize or restore the system hostname")]
    Hostname {
        #[command(subcommand)]
        action: HostnameAction,
    },
    #[cfg(target_os = "linux")]
    #[command(about = "Replace the MAC address whenever a probe request is seen")]
    Probe {
        #[command(subcommand)]
        action: ProbeAction,
    },
}

#[cfg(target_os = "linux")]
#[derive(Subcommand)]
pub enum MacAction {
    #[command(about = "Set a new MAC address")]
    Spoof {
        #[arg(long)]
        iface: String,
        #[arg(long, conflicts_with = "mac", help = "Use a prefix of this vendor")]
        vendor: Option<String>,
        #[arg(long, help = "Exact address to set, for example 02:aa:bb:cc:dd:ee")]
        mac: Option<String>,
    },
    #[command(about = "Put the saved MAC address back")]
    Restore {
        #[arg(long)]
        iface: String,
    },
    #[command(about = "List vendors available for --vendor")]
    ListVendors,
}

#[cfg(target_os = "linux")]
#[derive(Subcommand)]
pub enum DnsAction {
    #[command(about = "Verify a provider and make it the system resolver")]
    Set {
        #[arg(long, value_enum)]
        provider: ProviderArg,
        #[arg(long, value_enum)]
        protocol: ProtocolArg,
    },
    #[command(about = "Put the saved resolver configuration back")]
    Restore,
    #[command(about = "Resolve a name through the current system resolver")]
    Test,
}

#[derive(Subcommand)]
pub enum LeakAction {
    #[command(
        about = "Show IPv4, IPv6 and DNS resolver addresses seen from outside",
        long_about = "Show IPv4, IPv6 and DNS resolver addresses seen from outside.\n\nThe test uses the live system resolver, so results include any change made by 'privcrab dns set'."
    )]
    Test {
        #[arg(long, help = "Wait for every endpoint and print each result")]
        full: bool,
    },
}

#[cfg(target_os = "linux")]
#[derive(Subcommand)]
pub enum HostnameAction {
    #[command(about = "Set a random hostname")]
    Randomize,
    #[command(about = "Put the saved hostname back")]
    Restore,
}

#[cfg(target_os = "linux")]
#[derive(Subcommand)]
pub enum ProbeAction {
    #[command(about = "Enter monitor mode and replace the MAC on every probe request")]
    Start {
        #[arg(long)]
        iface: String,
    },
    #[command(about = "Leave monitor mode and restore the saved MAC")]
    Stop {
        #[arg(long)]
        iface: String,
    },
    #[command(about = "Show the current mode of an interface")]
    Status {
        #[arg(long)]
        iface: String,
    },
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, ValueEnum)]
pub enum ProviderArg {
    Cloudflare,
    Quad9,
    Nextdns,
}

#[cfg(target_os = "linux")]
impl ProviderArg {
    fn get(self) -> &'static Provider {
        match self {
            ProviderArg::Cloudflare => &PROVIDERS[0],
            ProviderArg::Quad9 => &PROVIDERS[1],
            ProviderArg::Nextdns => &PROVIDERS[2],
        }
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, ValueEnum)]
pub enum ProtocolArg {
    Doh,
    Dot,
}

#[cfg(target_os = "linux")]
impl ProtocolArg {
    fn get(self) -> Protocol {
        match self {
            ProtocolArg::Doh => Protocol::Doh,
            ProtocolArg::Dot => Protocol::Dot,
        }
    }
}

#[cfg(target_os = "linux")]
impl Command {
    pub fn need(&self) -> Need {
        match self {
            Command::Leak { .. } => Need::None,
            Command::Mac { action } => match action {
                MacAction::ListVendors => Need::None,
                MacAction::Spoof { .. } | MacAction::Restore { .. } => Need::NetAdmin,
            },
            Command::Dns { action } => match action {
                DnsAction::Test => Need::None,
                DnsAction::Set { .. } | DnsAction::Restore => Need::Root,
            },
            Command::Hostname { .. } => Need::Root,
            Command::Probe { action } => match action {
                ProbeAction::Status { .. } => Need::None,
                ProbeAction::Start { .. } => Need::NetAdminRaw,
                ProbeAction::Stop { .. } => Need::NetAdmin,
            },
        }
    }
}

#[cfg(target_os = "linux")]
pub fn ensure_privilege(command: &Command) {
    command.need().require();
}

#[cfg(not(target_os = "linux"))]
pub fn ensure_privilege(_command: &Command) {}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        #[cfg(target_os = "linux")]
        Command::Mac { action } => run_mac(action),
        #[cfg(target_os = "linux")]
        Command::Dns { action } => run_dns(action),
        Command::Leak { action } => run_leak(action),
        #[cfg(target_os = "linux")]
        Command::Hostname { action } => run_hostname(action),
        #[cfg(target_os = "linux")]
        Command::Probe { action } => run_probe(action),
    }
}

pub fn report(err: &anyhow::Error) {
    let mut chain = err.chain();
    let verb = chain.next().map(|part| part.to_string()).unwrap_or_default();
    let reason = chain.next().map(|part| part.to_string()).unwrap_or_default();
    match err.downcast_ref::<AppError>() {
        Some(AppError::Probe(ProbeError::MonitorModeFailed(iface))) => {
            eprintln!(
                "{}",
                warn(&format!(
                    "monitor mode failed on {} — adapter may not support it",
                    bold_turquoise(iface)
                ))
            );
            eprintln!(
                "{}",
                info("check: iw list | grep -A 10 \"Supported interface modes\"")
            );
        }
        _ => eprintln!(
            "{}",
            warn(&format!("{} failed: {}", bold_crab(&verb), reason))
        ),
    }
    tracing::debug!("{err:#}");
}

fn wrap<T, E: Into<AppError>>(verb: &'static str, result: Result<T, E>) -> anyhow::Result<T> {
    result.map_err(|err| anyhow::Error::new(err.into()).context(verb))
}

fn block_on<F: Future>(future: F) -> anyhow::Result<F::Output> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("runtime")?;
    Ok(runtime.block_on(future))
}

#[cfg(target_os = "linux")]
fn block_on_multi<F: Future>(future: F) -> anyhow::Result<F::Output> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("runtime")?;
    Ok(runtime.block_on(future))
}

fn run_leak(action: LeakAction) -> anyhow::Result<()> {
    let LeakAction::Test { full } = action;
    wrap("leak test", block_on(leak::test(full))?)
}

#[cfg(target_os = "linux")]
fn run_mac(action: MacAction) -> anyhow::Result<()> {
    match action {
        MacAction::Spoof { iface, vendor, mac } => {
            wrap("spoof", mac_spoof(&iface, vendor.as_deref(), mac.as_deref()))
        }
        MacAction::Restore { iface } => wrap("restore", mac_restore(&iface)),
        MacAction::ListVendors => {
            println!("{}", bold_crab("vendors available for --vendor:"));
            for name in oui::vendors() {
                println!("  {} {}", turquoise("•"), name);
            }
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
fn mac_spoof(iface: &str, vendor: Option<&str>, mac: Option<&str>) -> Result<(), AppError> {
    let target = match (mac, vendor) {
        (Some(mac), _) => mac.to_string(),
        (None, Some(vendor)) => oui::random_mac_for_vendor(vendor)
            .ok_or_else(|| MacError::InvalidMac(format!("unknown vendor {vendor}")))?,
        (None, None) => oui::random_mac(),
    };
    spoof::spoof(iface, &target)?;
    println!(
        "{}",
        ok(&format!(
            "{} is now {}",
            bold_turquoise(iface),
            green(&target)
        ))
    );
    println!("{}", note("action completed"));
    Ok(())
}

#[cfg(target_os = "linux")]
fn mac_restore(iface: &str) -> Result<(), AppError> {
    if spoof::restore(iface)? {
        println!("{}", ok(&format!("{} restored", bold_turquoise(iface))));
        println!("{}", note("action completed"));
    } else {
        println!("{}", warn("no saved state for mac"));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_dns(action: DnsAction) -> anyhow::Result<()> {
    match action {
        DnsAction::Set { provider, protocol } => {
            let target = provider.get();
            let proto = protocol.get();
            wrap("dns set", block_on(dns::set(target, proto))?)?;
            println!(
                "{}",
                ok(&format!(
                    "dns set to {} over {}",
                    bold_turquoise(target.name),
                    bold_purple(proto.label())
                ))
            );
            println!("{}", note("action completed"));
            Ok(())
        }
        DnsAction::Restore => {
            if wrap("dns restore", dns::restore())? {
                println!("{}", ok("dns restored"));
                println!("{}", note("action completed"));
            } else {
                println!("{}", warn("no saved state for dns"));
            }
            Ok(())
        }
        DnsAction::Test => {
            let found = wrap("dns test", block_on(dns::test())?)?;
            let list: Vec<String> = found
                .iter()
                .map(|ip| green(&ip.to_string()))
                .collect();
            println!(
                "{}",
                ok(&format!("system resolver answered: {}", list.join(", ")))
            );
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
fn run_hostname(action: HostnameAction) -> anyhow::Result<()> {
    match action {
        HostnameAction::Randomize => {
            let name = wrap("randomize", hostname::randomize())?;
            println!(
                "{}",
                ok(&format!("hostname is now {}", bold_purple(&name)))
            );
            println!("{}", note("action completed"));
            Ok(())
        }
        HostnameAction::Restore => {
            if wrap("hostname restore", hostname::restore())? {
                println!("{}", ok("hostname restored"));
                println!("{}", note("action completed"));
            } else {
                println!("{}", warn("no saved state for hostname"));
            }
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
fn run_probe(action: ProbeAction) -> anyhow::Result<()> {
    match action {
        ProbeAction::Start { iface } => {
            eprintln!("{}", warn("probe blocker is experimental"));
            eprintln!(
                "{}",
                info("prefer: NetworkManager wifi.scan-rand-mac-address=yes")
            );
            eprintln!(
                "{}",
                info("       or wpa_supplicant mac_addr=1 preassoc_mac_addr=1")
            );
            wrap("probe start", block_on_multi(probe::block::start(&iface))?)?;
            Ok(())
        }
        ProbeAction::Stop { iface } => {
            if wrap("probe stop", probe::block::stop(&iface))? {
                println!(
                    "{}",
                    ok(&format!("{} is back in managed mode", bold_turquoise(&iface)))
                );
                println!("{}", note("action completed"));
            } else {
                println!(
                    "{}",
                    warn(&format!("{} is not in monitor mode", iface))
                );
            }
            Ok(())
        }
        ProbeAction::Status { iface } => {
            let current = wrap("probe status", probe::mode(&iface))?;
            match current {
                Mode::Monitor => println!(
                    "{}",
                    note(&format!("{}: {}", bold_turquoise(&iface), green("monitor")))
                ),
                Mode::Managed => println!(
                    "{}",
                    note(&format!("{}: {}", bold_turquoise(&iface), turquoise("managed")))
                ),
                Mode::Other => println!(
                    "{}",
                    warn(&format!(
                        "{}: not a wireless interface (monitor mode unavailable)",
                        iface
                    ))
                ),
            }
            Ok(())
        }
    }
}