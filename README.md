<div align="center">
  <img src="photos/privcrab.png" alt="PrivCrab Logo" width="340">

  <h1>PrivCrab</h1>
  <h3>Linux-first Network Privacy Toolkit</h3>

[![Tests](https://github.com/tc4dy/privcrab/actions/workflows/test.yml/badge.svg)](https://github.com/tc4dy/privcrab/actions/workflows/test.yml)
![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg) ![License:
MIT](https://img.shields.io/badge/license-MIT-purple.svg)
![Network](https://img.shields.io/badge/Network-Privacy-turquoise)

**Take control. Leave no trace.**

</div>

**PrivCrab** is a Linux-first command-line utility for hardening your
network privacy. It provides granular control over hardware addresses,
DNS queries, system hostnames, and your IPv4/IPv6 leak surface.

Your network footprint often leaks more than you intend: hardware
identifiers in MAC addresses, plaintext DNS queries, predictable
hostnames, and unencrypted traffic paths. PrivCrab allows you to inspect
what the outside world sees and spoof or restore these identifiers.

It operates entirely locally. There is no telemetry or analytics, and it
makes no network calls beyond those required when you explicitly run a
leak test or verify a DNS provider.

<div align="center">
  <img src="photos/privcrab_banner.png" alt="PrivCrab Banner" width="100%">
</div>

------------------------------------------------------------------------

## Table of Contents

- [Features](#features)
- [Why Linux-First?](#why-linux-first)
- [Installation](#installation)
  - [1. Install System Dependencies](#1-install-system-dependencies)
  - [2. Quick Install Script](#2-quick-install-script-build-from-source)
  - [3. Pre-compiled Binaries](#3-pre-compiled-binaries)
  - [4. Manual Build](#4-manual-build)
- [Privilege Model](#privilege-model)
- [Usage](#usage)
  - [Example Output](#example-output)
  - [1. Leak Test](#1-leak-test)
  - [2. MAC Spoofing](#2-mac-spoofing)
  - [3. DNS over HTTPS / TLS](#3-dns-over-https--tls)
  - [4. Hostname Randomization](#4-hostname-randomization)
  - [5. Probe Monitoring](#5-probe-monitoring-experimental)
- [Privacy and Safety Notes](#privacy-and-safety-notes)

------------------------------------------------------------------------

## Features

PrivCrab is designed to handle specific network footprint issues without
bloating the binary.

- **Hardware Address Spoofing:** Changes the hardware address (MAC) of
  any interface. You can generate completely random locally administered
  addresses or choose specific vendor OUIs (Apple, Intel, Cisco, etc.)
  to blend in. PrivCrab automatically saves the original MAC to a state
  file, ensuring you can always restore it.
- **Encrypted DNS (DoH / DoT):** Verifies a DNS provider using an
  encrypted query, then modifies `/etc/resolv.conf` or
  `systemd-resolved`. It includes an automatic rollback mechanism: if
  applying the new DNS settings breaks your connectivity, it restores
  your previous configuration.
- **Hostname Randomization:** Generates a plausible hostname (e.g.,
  `quiet-node-a3f2`), applies it to the kernel, and updates
  `/etc/hostname`. It warns you if `/etc/hosts` needs manual updating to
  prevent `sudo` resolution delays.
- **Leak Detection:** Queries multiple external endpoints to determine
  your public IPv4, IPv6, and the exact IP address your DNS resolver is
  using. It checks if your public IP falls within a known datacenter
  range, giving you a hint if your VPN is actually routing traffic.
- **Probe Monitoring:** Switches a wireless adapter into monitor mode,
  captures Wi-Fi probe requests, and rotates the MAC address on the fly.
  (Note: Modern OS-level MAC randomization is generally preferred over
  this method.)
- **Crash-Safe State Management:** Saves backups to
  `~/.local/share/privcrab/state.json` (or `/var/lib/privcrab/` if run
  as root). If the state file is corrupted, it safely backs it up as
  `.bak` rather than panicking or losing your original MAC/hostname.

------------------------------------------------------------------------

## Why Linux-First?

PrivCrab is built specifically for Linux. This is not a technical
limitation, but a deliberate design choice.

On Windows, even the most meticulously crafted privacy tool is
fundamentally compromised. The operating system itself is deeply
integrated with telemetry, background data collection, hardware-level
tracking, and cloud-tied user profiles. Running a network privacy tool
on Windows is akin to locking the front door while the entire back wall
is missing.

Linux gives you true kernel-level control. It allows PrivCrab to
directly interact with network interfaces via `ioctl` and `netlink`,
modify system resolvers without fighting a locked-down registry, and
operate with the absolute minimum required privileges rather than full
administrative access.

If you care about actual digital sovereignty, your network layer must
run on an OS you control. PrivCrab is designed for that reality.

------------------------------------------------------------------------

## Installation

PrivCrab is written in Rust and requires `libpcap` for raw packet
capture (used in the `probe` module). Depending on your distribution,
you may need to install system dependencies before compiling.

### 1. Install System Dependencies

**Fedora / RHEL / CentOS**

``` bash
sudo dnf install libpcap-devel gcc
```

**Debian / Ubuntu / Kali**

``` bash
sudo apt update
sudo apt install libpcap-dev build-essential
```

**Arch Linux / Manjaro**

``` bash
sudo pacman -S libpcap base-devel
```

### 2. Quick Install Script (Build from Source)

The provided script handles dependency checks, Rust toolchain
installation (if missing), compilation, and capability assignment.

``` bash
git clone https://github.com/tc4dy/privcrab.git
cd privcrab
chmod +x install.sh
sudo ./install.sh
```

**Note:** The installer will automatically run
`setcap cap_net_admin,cap_net_raw+ep` on the binary so you do not need
`sudo` for MAC/probe commands.

### 3. Pre-compiled Binaries

If you do not want to build from source or install the Rust toolchain,
you can download the pre-compiled binary directly from the [GitHub
Releases page](https://github.com/tc4dy/privcrab/releases).

1.  **Download the binary:** Go to the Releases page and download the
    latest `privcrab` binary.

2.  **Make it executable:**

    ``` bash
    chmod +x privcrab
    ```

3.  **Run it:** You can test it immediately without root.

    ``` bash
    ./privcrab leak test
    ```

4.  **Optional — move to a system path:** To run it from anywhere, move
    it to `/usr/local/bin` and grant the necessary capabilities.

    ``` bash
    sudo mv privcrab /usr/local/bin/
    sudo setcap cap_net_admin,cap_net_raw+ep /usr/local/bin/privcrab
    ```

    You can now run `privcrab` from any directory.

### 4. Manual Build

If you have Rust 1.74+ installed and prefer to build manually:

``` bash
# Build the binary
cargo build --release --locked

# Install to /usr/local/bin
sudo cp target/release/privcrab /usr/local/bin/

# Grant capabilities (required for MAC and probe)
sudo setcap cap_net_admin,cap_net_raw+ep /usr/local/bin/privcrab
```

------------------------------------------------------------------------

## Privilege Model

PrivCrab uses Linux Capabilities instead of running as a full root
daemon.

| Command                     | Required Privilege             | Reason                                                        |
|-----------------------------|--------------------------------|---------------------------------------------------------------|
| `leak test`                 | None                           | Only makes outbound HTTP/DNS requests.                        |
| `mac spoof` / `mac restore` | `CAP_NET_ADMIN`                | Requires ioctl access to modify interface hardware addresses. |
| `probe start`               | `CAP_NET_ADMIN`, `CAP_NET_RAW` | Requires raw sockets (`pcap`) and interface mode switching.   |
| `dns set` / `dns restore`   | Root                           | Writes to `/etc/resolv.conf` or calls `resolvectl`.           |
| `hostname randomize`        | Root                           | Calls `sethostname` and writes to `/etc/hostname`.            |

If you skip `setcap`, you must run all privileged commands with `sudo`.

------------------------------------------------------------------------

## Usage

Running `privcrab` without arguments prints the help menu. Every command
follows a predictable structure.

### Example Output

**Checking for leaks**

``` text
$ privcrab leak test

──────────────────────────────────────────────
   privcrab
   Network privacy hardening toolkit
──────────────────────────────────────────────
   v0.1.0 | github.com/tc4dy

[+] ipv4: 93.88.156.41
[-] ipv6 not reachable
[*] dns resolver seen from outside: 195.123.245.19
[-] ipv4 address is not inside a known datacenter range
```

**Spoofing a MAC address**

``` text
$ sudo privcrab mac spoof --iface wlan0 --vendor Intel

──────────────────────────────────────────────
   privcrab
   Network privacy hardening toolkit
──────────────────────────────────────────────
   v0.1.0 | github.com/tc4dy

[+] wlan0 is now 3c:a9:f4:aa:bb:cc
[*] action completed
```

### 1. Leak Test

Check what the outside world sees before and after applying changes.

``` bash
privcrab leak test
```

Use `--full` to wait for every endpoint and print each individual
result. If IPv6 is not reachable, it prints a negative status. If your
IPv4 falls into a known datacenter range, it hints that you are behind a
VPN.

### 2. MAC Spoofing

Spoof the hardware address. By default, generates a random locally
administered address.

``` bash
sudo privcrab mac spoof --iface wlan0
```

Use a specific vendor prefix or an exact MAC address:

``` bash
sudo privcrab mac spoof --iface wlan0 --vendor Intel
sudo privcrab mac spoof --iface wlan0 --mac 02:aa:bb:cc:dd:ee
```

Restore the original MAC from the saved state:

``` bash
sudo privcrab mac restore --iface wlan0
```

#### MAC Options

| Flag              | Description                                                                               |
|-------------------|-------------------------------------------------------------------------------------------|
| `--iface IFACE`   | Interface to modify (e.g., `wlan0`, `enp3s0`).                                            |
| `--vendor VENDOR` | Use a prefix from a known vendor. Run `privcrab mac list-vendors` to see supported names. |
| `--mac MAC`       | Set an exact MAC address (e.g., `02:aa:bb:cc:dd:ee`). Must be unicast and non-zero.       |

### 3. DNS Over HTTPS / TLS

Verify and switch the system resolver to a privacy-focused provider.

``` bash
sudo privcrab dns set --provider cloudflare --protocol dot
```

#### DNS Options

| Flag         | Description                                   |
|--------------|-----------------------------------------------|
| `--provider` | `cloudflare`, `quad9`, or `nextdns`.          |
| `--protocol` | `doh` (requires `systemd-resolved`) or `dot`. |

Restore the previous resolver configuration:

``` bash
sudo privcrab dns restore
```

Test if the current resolver is answering correctly:

``` bash
privcrab dns test
```

### 4. Hostname Randomization

Generate a random hostname and apply it to the kernel and
`/etc/hostname`.

``` bash
sudo privcrab hostname randomize
```

Restore the original hostname:

``` bash
sudo privcrab hostname restore
```

### 5. Probe Monitoring (Experimental)

Enter monitor mode on a wireless interface and rotate the MAC address
whenever a probe request is captured. This is useful for testing how
often your device is broadcasting its presence.

``` bash
sudo privcrab probe start --iface wlan0
```

Stop monitoring and restore the original MAC address:

``` bash
sudo privcrab probe stop --iface wlan0
```

Check the current mode of an interface:

``` bash
privcrab probe status --iface wlan0
```

------------------------------------------------------------------------

## Privacy Notes

> **MAC Randomization:** Changing your MAC address may temporarily
> disconnect your Wi-Fi. Modern systems (NetworkManager, Windows)
> already randomize MACs on scan by default. PrivCrab is useful when you
> need a specific vendor OUI or a persistent change.

> **DNS Encryption:** DoH is not supported by `/etc/resolv.conf`.
> PrivCrab will refuse to apply DoH unless `systemd-resolved` is
> present. DoT is supported natively by `systemd-resolved` on modern
> Linux distributions.

> **Hostname Limits:** Changing the hostname does not update
> `/etc/hosts`. You may experience delays with `sudo` if the old
> hostname is no longer resolvable. PrivCrab prints a warning if this is
> detected.

> **Probe Mode:** Monitoring probe requests is experimental and may not
> work on all Wi-Fi adapters. It is often more effective to enable
> native OS-level MAC randomization.

> **State Safety:** PrivCrab saves backups in
> `~/.local/share/privcrab/state.json`. If run as root, it uses
> `/var/lib/privcrab/state.json`. If a state file is corrupted, it is
> renamed to `.bak` to prevent data loss.

------------------------------------------------------------------------

<div align="center">

**PrivCrab — Take control. Leave no trace.**

[GitHub Repository](https://github.com/tc4dy/privcrab) ·
[Releases](https://github.com/tc4dy/privcrab/releases) · [Report an
Issue](https://github.com/tc4dy/privcrab/issues)

</div>
