::: {align="center"}
`<img src="photos/privcrab.png" alt="PrivCrab logo" width="150">`{=html}

# PrivCrab

### Linux-first Network Privacy Toolkit

[![Tests](https://github.com/tc4dy/privcrab/actions/workflows/test.yml/badge.svg)](https://github.com/tc4dy/privcrab/actions/workflows/test.yml)
![Rust](https://img.shields.io/badge/rust-1.74%2B-blue.svg) ![License:
MIT](https://img.shields.io/badge/license-MIT-purple.svg)
![Network](https://img.shields.io/badge/Network-Privacy-turquoise)

**Take control. Leave no trace.**

A Linux-first command-line utility for hardening your network privacy.
PrivCrab provides granular control over hardware addresses, DNS queries,
system hostnames, and your IPv4/IPv6 leak surface.
:::

Your network footprint can reveal more than you intend: hardware
identifiers in MAC addresses, plaintext DNS queries, predictable
hostnames, and unencrypted traffic paths. PrivCrab lets you inspect what
the outside world can see, then spoof or restore selected identifiers.

PrivCrab operates locally. It has **no telemetry or analytics** and
makes no network calls except when you explicitly run a leak test or
verify a DNS provider.

::: {align="center"}
`<img src="photos/privcrab_banner.png" alt="PrivCrab banner" width="100%">`{=html}
:::

------------------------------------------------------------------------

## Table of Contents

-   [Features](#features)
-   [Why Linux-First?](#why-linux-first)
-   [Installation](#installation)
    -   [System Dependencies](#1-install-system-dependencies)
    -   [Quick Install
        Script](#2-quick-install-script-build-from-source)
    -   [Pre-compiled Binaries](#3-pre-compiled-binaries)
    -   [Manual Build](#4-manual-build)
-   [Privilege Model](#privilege-model)
-   [Usage](#usage)
    -   [Leak Test](#1-leak-test)
    -   [MAC Spoofing](#2-mac-spoofing)
    -   [DNS over HTTPS / TLS](#3-dns-over-https--tls)
    -   [Hostname Randomization](#4-hostname-randomization)
    -   [Probe Monitoring](#5-probe-monitoring-experimental)
-   [Privacy and Safety Notes](#privacy-and-safety-notes)

------------------------------------------------------------------------

## Features

PrivCrab is designed to address specific network-privacy issues without
bloating the binary.

-   **MAC Address Spoofing:** Change the hardware address of a network
    interface. Generate random locally administered addresses or choose
    vendor OUIs (Apple, Intel, Cisco, and others). PrivCrab saves the
    original MAC address in a state file so it can be restored later.
-   **Encrypted DNS (DoH / DoT):** Verify a DNS provider using an
    encrypted query, then configure `/etc/resolv.conf` or
    `systemd-resolved`. An automatic rollback mechanism restores the
    previous DNS configuration if applying the new settings breaks
    connectivity.
-   **Hostname Randomization:** Generate and apply a plausible hostname
    (for example, `quiet-node-a3f2`) to the kernel and `/etc/hostname`.
    PrivCrab warns if `/etc/hosts` may need a manual update to prevent
    `sudo` resolution delays.
-   **Leak Detection:** Query external endpoints to determine your
    public IPv4 address, IPv6 reachability, and the DNS resolver visible
    externally. PrivCrab also checks whether your public IP falls within
    a known datacenter range, which can provide a hint about whether
    your VPN is routing traffic.
-   **Probe Monitoring:** Put a wireless adapter into monitor mode,
    capture Wi-Fi probe requests, and rotate the MAC address when a
    probe request is captured. Modern OS-level MAC randomization is
    generally preferred over this method.
-   **Crash-Safe State Management:** Save backups to
    `~/.local/share/privcrab/state.json` or, when run as root,
    `/var/lib/privcrab/state.json`. If the state file is corrupted,
    PrivCrab backs it up as `.bak` rather than panicking or discarding
    the saved MAC address or hostname.

------------------------------------------------------------------------

## Why Linux-First?

PrivCrab is built specifically for Linux. This is a deliberate design
choice rather than a technical limitation.

On Windows, even a carefully designed privacy tool operates within an
ecosystem that includes operating-system telemetry, background data
collection, hardware-level tracking, and cloud-linked user profiles.
Running a network privacy tool on Windows can feel like locking the
front door while leaving the back wall exposed.

Linux provides direct control over the networking stack. It allows
PrivCrab to interact with network interfaces through `ioctl` and
`netlink`, modify system resolvers, and use the minimum required
privileges instead of relying on full administrative access for every
operation.

If you care about digital sovereignty, your network layer should run on
an operating system you control. PrivCrab is designed with that
principle in mind.

------------------------------------------------------------------------

## Installation

PrivCrab is written in Rust and requires `libpcap` for raw packet
capture in the `probe` module. Depending on your Linux distribution,
install the system dependencies before compiling.

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

The installer automatically runs `setcap cap_net_admin,cap_net_raw+ep`
on the binary. This allows MAC and probe commands to use the assigned
capabilities without requiring `sudo` for those commands.

### 3. Pre-compiled Binaries

If you do not want to build from source or install the Rust toolchain,
download the pre-compiled binary from the [GitHub Releases
page](https://github.com/tc4dy/privcrab/releases).

1.  **Download the binary.**\
    Open the Releases page and download the latest `privcrab` binary.

2.  **Make it executable.**

    ``` bash
    chmod +x privcrab
    ```

3.  **Run it.**

    You can test it immediately without root:

    ``` bash
    ./privcrab leak test
    ```

4.  **Optional: Move it to a system path.**

    To run it from anywhere, move it to `/usr/local/bin` and grant the
    required capabilities:

    ``` bash
    sudo mv privcrab /usr/local/bin/
    sudo setcap cap_net_admin,cap_net_raw+ep /usr/local/bin/privcrab
    ```

    You can now run `privcrab` from any directory.

### 4. Manual Build

If you have Rust 1.74 or later installed and prefer to build manually:

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

PrivCrab uses Linux capabilities instead of running as a full-root
daemon.

  -----------------------------------------------------------------------
  Command                 Required privilege      Reason
  ----------------------- ----------------------- -----------------------
  `leak test`             None                    Makes outbound HTTP/DNS
                                                  requests.

  `mac spoof` /           `CAP_NET_ADMIN`         Requires permission to
  `mac restore`                                   modify interface
                                                  hardware addresses
                                                  through `ioctl`.

  `probe start`           `CAP_NET_ADMIN`,        Requires raw sockets
                          `CAP_NET_RAW`           (`pcap`) and interface
                                                  mode switching.

  `dns set` /             Root                    Writes to
  `dns restore`                                   `/etc/resolv.conf` or
                                                  calls `resolvectl`.

  `hostname randomize`    Root                    Calls `sethostname` and
                                                  writes to
                                                  `/etc/hostname`.
  -----------------------------------------------------------------------

If you do not assign capabilities with `setcap`, run the relevant
privileged commands with `sudo`.

------------------------------------------------------------------------

## Usage

Running `privcrab` without arguments prints the help menu. Commands
follow a consistent structure.

### Example Output

#### Leak test

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

#### MAC spoofing

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

Check what the outside world can see before and after applying changes.

``` bash
privcrab leak test
```

Use `--full` to wait for every endpoint and print each individual
result. If IPv6 is not reachable, PrivCrab reports a negative status. If
your IPv4 address falls within a known datacenter range, it indicates
that you may be behind a VPN.

### 2. MAC Spoofing

Spoof a network interface's hardware address. By default, PrivCrab
generates a random locally administered address.

``` bash
sudo privcrab mac spoof --iface wlan0
```

Use a specific vendor prefix or an exact MAC address:

``` bash
sudo privcrab mac spoof --iface wlan0 --vendor Intel
sudo privcrab mac spoof --iface wlan0 --mac 02:aa:bb:cc:dd:ee
```

Restore the original MAC address from the saved state:

``` bash
sudo privcrab mac restore --iface wlan0
```

#### MAC options

  -----------------------------------------------------------------------
  Flag                                Description
  ----------------------------------- -----------------------------------
  `--iface IFACE`                     Interface to modify (for example,
                                      `wlan0` or `enp3s0`).

  `--vendor VENDOR`                   Use a prefix from a known vendor.
                                      Run `privcrab mac list-vendors` to
                                      see supported names.

  `--mac MAC`                         Set an exact MAC address (for
                                      example, `02:aa:bb:cc:dd:ee`). The
                                      address must be unicast and
                                      non-zero.
  -----------------------------------------------------------------------

### 3. DNS over HTTPS / TLS

Verify and switch the system resolver to a privacy-focused provider.

``` bash
sudo privcrab dns set --provider cloudflare --protocol dot
```

#### DNS options

  -----------------------------------------------------------------------
  Flag                                Description
  ----------------------------------- -----------------------------------
  `--provider`                        Supported providers: `cloudflare`,
                                      `quad9`, or `nextdns`.

  `--protocol`                        `doh` (requires `systemd-resolved`)
                                      or `dot`.
  -----------------------------------------------------------------------

Restore the previous resolver configuration:

``` bash
sudo privcrab dns restore
```

Test whether the current resolver is responding correctly:

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

Put a wireless interface into monitor mode and rotate its MAC address
whenever a probe request is captured. This can help you test how often
your device broadcasts its presence.

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

## Privacy and Safety Notes

-   **MAC randomization:** Changing your MAC address may temporarily
    disconnect your Wi-Fi. Modern systems (including NetworkManager) can
    already randomize MAC addresses while scanning. PrivCrab is useful
    when you need a specific vendor OUI or a persistent change.
-   **DNS encryption:** DoH is not supported by a plain
    `/etc/resolv.conf` configuration. PrivCrab refuses to apply DoH
    unless `systemd-resolved` is present. DoT is supported natively by
    `systemd-resolved` on modern Linux distributions.
-   **Hostname limits:** Changing the hostname does not update
    `/etc/hosts`. You may experience delays with `sudo` if the old
    hostname is no longer resolvable. PrivCrab prints a warning if it
    detects this situation.
-   **Probe mode:** Probe-request monitoring is experimental and may not
    work with every Wi-Fi adapter. Native OS-level MAC randomization is
    often more effective.
-   **State safety:** PrivCrab stores backups in
    `~/.local/share/privcrab/state.json`. When run as root, it uses
    `/var/lib/privcrab/state.json`. If a state file is corrupted, it is
    renamed to `.bak` to help prevent data loss.

------------------------------------------------------------------------

::: {align="center"}
**PrivCrab --- Take control. Leave no trace.**

[GitHub Repository](https://github.com/tc4dy/privcrab) ·
[Releases](https://github.com/tc4dy/privcrab/releases) · [Report an
Issue](https://github.com/tc4dy/privcrab/issues)
:::
