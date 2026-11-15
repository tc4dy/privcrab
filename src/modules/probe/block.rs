use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use pcap::{Active, Capture};

use super::{check_name, enter_monitor, leave_monitor};
use crate::error::ProbeError;
use crate::modules::mac::oui::random_mac;
use crate::modules::mac::spoof::{restore as restore_mac, spoof};

const RADIOTAP: i32 = 127;
const FILTER: &str = "type mgt subtype probe-req";
const READ_TIMEOUT_MS: i32 = 200;
const RETRY_DELAY: Duration = Duration::from_millis(200);
const MAC_HEADER_SA_OFFSET: usize = 10;

pub fn src_mac(frame: &[u8]) -> Option<[u8; 6]> {
    let radiotap_len = usize::from(u16::from_le_bytes(frame.get(2..4)?.try_into().ok()?));
    let start = radiotap_len.checked_add(MAC_HEADER_SA_OFFSET)?;
    let end = start.checked_add(6)?;
    frame.get(start..end)?.try_into().ok()
}

pub async fn start(iface: &str) -> Result<(), ProbeError> {
    check_name(iface)?;
    enter_monitor(iface)?;
    println!("[*] probe blocker running on {iface}, press Ctrl-C to stop");
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let name = iface.to_string();
    let mut worker = tokio::task::spawn_blocking(move || run_loop(&name, &flag));
    let joined = tokio::select! {
        signal = tokio::signal::ctrl_c() => {
            stop.store(true, Ordering::Relaxed);
            signal.map_err(ProbeError::Signal)?;
            worker.await
        }
        joined = &mut worker => joined,
    };
    if let Err(err) = &joined {
        tracing::debug!("probe loop failed: {err:?}");
        let _ = leave_monitor(iface);
        let _ = restore_mac(iface);
    }
    match joined {
        Ok(result) => {
            if result.is_ok() {
                println!("[*] probe blocker stopped");
            }
            result
        }
        Err(err) => {
            tracing::debug!("{err:?}");
            Err(ProbeError::CaptureOpen(iface.to_string()))
        }
    }
}

pub fn stop(iface: &str) -> Result<bool, ProbeError> {
    check_name(iface)?;
    let left = leave_monitor(iface)?;
    if restore_mac(iface)? {
        println!("[+] original MAC restored on {iface}");
    } else {
        eprintln!("[!] no saved MAC to restore on {iface}");
    }
    Ok(left)
}

fn capture_error<E: std::fmt::Debug>(iface: &str, err: E) -> ProbeError {
    tracing::debug!("{err:?}");
    ProbeError::CaptureOpen(iface.to_string())
}

fn open_capture(iface: &str) -> Result<Capture<Active>, ProbeError> {
    let mut cap = Capture::from_device(iface)
        .map_err(|err| capture_error(iface, err))?
        .promisc(true)
        .immediate_mode(true)
        .timeout(READ_TIMEOUT_MS)
        .open()
        .map_err(|err| capture_error(iface, err))?;
    if cap.get_datalink().0 != RADIOTAP {
        return Err(ProbeError::MonitorModeFailed(iface.to_string()));
    }
    cap.filter(FILTER, true).map_err(|err| {
        tracing::debug!("{err:?}");
        ProbeError::FilterSet(iface.to_string())
    })?;
    Ok(cap)
}

fn run_loop(iface: &str, stop: &AtomicBool) -> Result<(), ProbeError> {
    let mut cap = open_capture(iface)?;
    while !stop.load(Ordering::Relaxed) {
        let hit = match cap.next_packet() {
            Ok(packet) => src_mac(packet.data).is_some(),
            Err(pcap::Error::TimeoutExpired) => false,
            Err(err) => {
                tracing::debug!("{err:?}");
                thread::sleep(RETRY_DELAY);
                cap = open_capture(iface)?;
                false
            }
        };
        if hit {
            if let Err(err) = spoof(iface, &random_mac()) {
                tracing::debug!("spoof failed: {err:?}");
            }
            cap = open_capture(iface)?;
        }
    }
    Ok(())
}