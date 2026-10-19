# taskmgr-rust

A simple Linux task manager that shows what is *actually* running.

- Resolves processes hidden behind wrappers (Flatpak/bwrap, Wine/Proton,
  Steam pressure-vessel, AppImage, NixOS `-wrapped` binaries, the dynamic
  loader) and groups them by application.
- Shows every disk, temperature sensor and GPU separately, with readable
  names.

Pages:

- **Processes**: applications, background and system processes, grouped by
  application with their window titles (KDE Plasma), heat-mapped columns,
  end task (SIGTERM) and kill (SIGKILL).
- **Performance**: one entry per CPU, memory, disk (with SMART health),
  network adapter, GPU and battery, power use of everything that reports
  it, sensors.
- **Details**, **Startup apps**, **Users**, **Services** (with their journal).
- **Connections**: TCP, UDP and named Unix sockets grouped by application,
  with who can reach each one (this computer, local network, everyone).
- **Crashes**: crashed processes from the systemd journal, with the stack
  trace when systemd-coredump recorded one.
- **Benchmark**: CPU, memory and disk scores compared with the previous
  run, and a stability test for overclocking that counts miscalculations.
- **System**: a summary of the machine, and a searchable hardware tree in
  the spirit of dmidecode (firmware, board, processor and caches, memory
  modules, graphics, disks and partitions, network, displays, audio, USB,
  PCI), each component with its live readings and their min and max.

Never run it as root. Optional data needs the administrator password once
(per-process network, CPU voltage) or a udev rule (CPU power, see
`docs/energy.md`).

## Compatibility

The AppImage carries its own glibc and Qt, so it runs on any x86_64 Linux
from about 2018 on, glibc or musl. Tested: Debian 12, Ubuntu 18.04, 20.04
and 24.04, Fedora 41, Arch, openSUSE Leap 15.6, Alpine 3.20, NixOS (KDE
Plasma and GNOME, Wayland and X11).

Each source of data is optional; without it the matching view says so:

| Feature | Needs |
|---|---|
| Grouping by application | cgroups (v1 or v2); else the process tree |
| Window titles | KDE Plasma (KWin) |
| Services, Crashes, journals | systemd |
| Disk health | UDisks2 |
| Power profile | power-profiles-daemon or tuned-ppd |
| NVIDIA GPU details | the NVIDIA driver (NVML) |

## AppImage

```sh
nix build .#appimage   # result/taskmgr-x86_64.AppImage, about 55 MB
```

Only what the app loads is kept (see `packaging/appimage.sh`): standard Qt
controls with Breeze colours, software rendering, no KDE frameworks.

## Build from source

Rust 1.88 or newer (rustup where the distribution ships an older one), Qt
6.5 or newer and Kirigami 6. Tested with the commands below; Fedora also
needs lld, its default linker fails on the Qt objects.

```sh
# Arch
pacman -S rust base-devel qt6-base qt6-declarative qt6-tools qt6-wayland qt6-svg kirigami qqc2-desktop-style
# Fedora
dnf install cargo rust gcc-c++ lld qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qttools-devel qt6-linguist qt6-qtwayland qt6-qtsvg kf6-kirigami kf6-qqc2-desktop-style
# Debian 13 (its rustc is too old)
apt install rustup g++ pkg-config qt6-base-dev qt6-declarative-dev qt6-tools-dev qt6-l10n-tools qt6-wayland qml6-module-org-kde-kirigami qml6-module-org-kde-desktop qml6-module-qtquick-controls qml6-module-qtquick-layouts qml6-module-qtqml-workerscript qml6-module-qtcore
rustup default stable

cargo build --release
```

## Nix

```sh
nix run github:<owner>/taskmgr-rust
```

In a NixOS flake, follow your own nixpkgs so the app uses the same Qt and
KDE Frameworks as your desktop:

```nix
inputs.taskmgr.url = "github:<owner>/taskmgr-rust";
inputs.taskmgr.inputs.nixpkgs.follows = "nixpkgs";
# environment.systemPackages = [ inputs.taskmgr.packages.${pkgs.system}.default ];
```

## Development

```sh
nix develop          # Rust toolchain, Qt 6, Kirigami
cargo run -p taskmgr
cargo run -p taskmgr-core --example snapshot   # raw measurements as JSON
cargo run -p taskmgr-core --example processes  # processes grouped by application
cargo run -p taskmgr-core --example crashes    # crashes found in the journal
nix build .#probes.x86_64-linux.native         # static core binaries (data collection)
```

Layout:

- `crates/taskmgr-core`: everything measured or resolved, from `/proc`,
  `/sys`, cgroups, D-Bus and the journal. No Qt, no root, no global state;
  tested against fixture directories.
- `crates/taskmgr`: Qt Quick / Kirigami interface bridged with cxx-qt
  (`src/monitor` exposes the core to QML as JSON).

License: MIT (see `LICENSE`).
