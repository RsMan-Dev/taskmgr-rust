# taskmgr-rust

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)
![Qt 6.5+](https://img.shields.io/badge/Qt-6.5%2B-41cd52.svg)
![Platform: Linux](https://img.shields.io/badge/platform-Linux-lightgrey.svg)

A Linux task manager in the spirit of the Windows one, that shows what is
*actually* running.

Most Linux task managers list `bwrap`, `wine64-preloader`, `pressure-vessel`
or `.firefox-wrapped` where you expect an application. taskmgr-rust looks
through Flatpak, Wine and Proton, Steam, AppImage, Snap and NixOS wrappers,
names each process after what it really is, and groups processes by
application. It shows every disk, GPU, network adapter, fan and temperature
sensor on its own, never merged.

## Features

**Processes**
- Applications, background and system processes, grouped by application
  with their window titles (KDE Plasma) and icons.
- Identity resolved through Flatpak/bwrap, Wine/Proton, Steam
  pressure-vessel, AppImage, Snap, NixOS `-wrapped` binaries and the dynamic
  loader. Without systemd units (other desktops, OpenRC), processes are
  matched to the desktop entry that starts them, web apps included.
- Heat-mapped columns: CPU, memory, disk, network (per process, optional),
  GPU use and memory for NVIDIA, AMD and Intel, video encoder and decoder,
  energy impact.
- End task, kill, suspend, resume and other signals, for a process or a
  whole application; priority; journal of a process.

**Performance**
- One entry and one graph per CPU, memory, disk, network adapter (Wi-Fi
  name and signal; VPN and virtual adapters on request), GPU, battery and
  fan. Entries can be hidden and reordered.
- CPU per logical processor, per-core frequency, kernel time, NUMA nodes,
  temperature and voltage; memory composition and modules.
- Disk health from SMART with every attribute, and safe removal of USB
  drives that names the processes keeping one busy.
- Power use of each part that reports it (CPU, GPU, battery, sensors).

**More pages**
- **Details**: every process as a flat list or a tree.
- **Startup apps**, **Users**, **Services** (with their journal).
- **Connections**: TCP, UDP and named Unix sockets grouped by application,
  with who can reach each one (this computer, local network, everyone).
- **Crashes**: crashed processes from the systemd journal, with the stack
  trace when systemd-coredump recorded one.
- **Benchmark**: CPU, memory and disk scores compared with previous runs,
  and a stability test for overclocking that counts miscalculations.
- **System**: a summary of the machine and a searchable hardware tree in
  the spirit of dmidecode (firmware, board, processor and caches, memory
  modules, graphics, disks and partitions, network, displays, audio, USB,
  PCI), each component with its live readings and their minimum and
  maximum.

**Everywhere**
- Native look on KDE Plasma (Breeze, system colours and accent), light and
  dark themes, English and French.
- Usable down to phone-sized windows; a long press opens menus on touch
  screens.
- Mini mode, keyboard shortcuts (F1 lists them), copyable values, unit and
  graph settings.

## Permissions

Never run taskmgr as root. Everything works as a normal user; a few extras
ask for the administrator password through polkit, once, when you turn
them on:

| Extra | How |
|---|---|
| Network use per process, CPU voltage estimate | a small helper started with `pkexec` |
| Raising a priority | `pkexec renice` |
| CPU power | see below |

### CPU power

The processor's energy counter (`/sys/class/powercap/intel-rapl:*`, AMD
included) is readable by root only since the PLATYPUS attack
(CVE-2020-8694). The **Allow access…** button on the CPU page opens it to
your user until the next reboot. To open it for good to a group (`wheel`
here), add a udev rule:

```nix
# NixOS, configuration.nix
services.udev.extraRules = ''
  SUBSYSTEM=="powercap", KERNEL=="intel-rapl:*", RUN+="${pkgs.coreutils}/bin/chgrp wheel /sys%p/energy_uj", RUN+="${pkgs.coreutils}/bin/chmod g+r /sys%p/energy_uj"
'';
```

```
# Other distributions, /etc/udev/rules.d/90-rapl.rules
SUBSYSTEM=="powercap", KERNEL=="intel-rapl:*", RUN+="/bin/chgrp wheel /sys%p/energy_uj", RUN+="/bin/chmod g+r /sys%p/energy_uj"
```

It applies at the next boot, or at once with
`sudo udevadm trigger --subsystem-match=powercap`. The trade-off: a program
reading this counter very often can infer what the processor computes, down
to fragments of cryptographic keys. On a single-user desktop the risk is
small; on a shared machine or a server, better do without.

## Compatibility

The AppImage carries its own libraries, so it runs on any x86_64 Linux from
about 2018 on, glibc or musl. Tested on Debian 12 and 13, Ubuntu 18.04,
20.04 and 24.04, Fedora 41, Arch, openSUSE Leap 15.6, Alpine 3.20 and
NixOS, under KDE Plasma and GNOME, Wayland and X11.

Each source of data is optional; without it, the matching view says so:

| Feature | Needs |
|---|---|
| Grouping by application | cgroups (v1 or v2); else desktop entries and the process tree |
| Window titles | KDE Plasma (KWin) |
| Services, crashes, journals | systemd |
| Disk health, safe removal | UDisks2 |
| Wi-Fi details | NetworkManager (signal alone without it) |
| Power profile | power-profiles-daemon or tuned-ppd |
| NVIDIA GPU details | the NVIDIA driver (NVML) |

Flatpak is not a target: its sandbox hides the very processes a task
manager has to see.

## Install

### Nix

```sh
nix run github:RsMan-Dev/taskmgr-rust
```

In a NixOS flake, follow your own nixpkgs so the app uses the same Qt and
KDE Frameworks as your desktop:

```nix
inputs.taskmgr.url = "github:RsMan-Dev/taskmgr-rust";
inputs.taskmgr.inputs.nixpkgs.follows = "nixpkgs";
# environment.systemPackages = [ inputs.taskmgr.packages.${pkgs.system}.default ];
```

### AppImage

```sh
nix build github:RsMan-Dev/taskmgr-rust#appimage   # result/taskmgr-x86_64.AppImage, about 55 MB
```

Only what the app loads is bundled (see `packaging/appimage.sh`): standard
Qt controls with Breeze colours and software rendering, no KDE Frameworks.

### From source

Rust 1.88 or newer (use rustup where the distribution ships an older one),
Qt 6.5 or newer and Kirigami 6. Fedora also needs lld: its default linker
fails on the Qt objects.

```sh
# Arch
pacman -S rust base-devel qt6-base qt6-declarative qt6-tools qt6-wayland qt6-svg kirigami qqc2-desktop-style
# Fedora
dnf install cargo rust gcc-c++ lld qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qttools-devel qt6-linguist qt6-qtwayland qt6-qtsvg kf6-kirigami kf6-qqc2-desktop-style
# Debian 13 (its rustc is too old)
apt install rustup g++ pkg-config qt6-base-dev qt6-declarative-dev qt6-tools-dev qt6-l10n-tools qt6-wayland qml6-module-org-kde-kirigami qml6-module-org-kde-desktop qml6-module-qtquick-controls qml6-module-qtquick-layouts qml6-module-qtqml-workerscript qml6-module-qtcore
rustup default stable

git clone https://github.com/RsMan-Dev/taskmgr-rust
cd taskmgr-rust
cargo build --release
./target/release/taskmgr
```

## Development

```sh
nix develop          # Rust toolchain, Qt 6, Kirigami
cargo run -p taskmgr
cargo test --workspace
cargo run -p taskmgr-core --example snapshot   # raw measurements as JSON
cargo run -p taskmgr-core --example processes  # processes grouped by application
cargo run -p taskmgr-core --example inventory  # hardware tree
```

The interface can render itself without a screen, which is how it is
tested:

```sh
QT_QPA_PLATFORM=offscreen TASKMGR_GRAB=/tmp/shot.png TASKMGR_PAGE=1 ./target/release/taskmgr
```

Layout:

- `crates/taskmgr-core`: everything measured or resolved, from `/proc`,
  `/sys`, cgroups, D-Bus and the journal. No Qt, no root, no `unsafe`;
  tested against fixture directories.
- `crates/taskmgr`: Qt Quick / Kirigami interface bridged with
  [cxx-qt](https://github.com/KDAB/cxx-qt); `src/monitor` hands the core's
  results to QML as JSON.
- `translations/`: Qt Linguist files
  (`lupdate crates/taskmgr/qml -ts translations/taskmgr_fr.ts`).

## License

[MIT](LICENSE). Interface icons from
[Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons)
(MIT, see `crates/taskmgr/icons/LICENSE`).
