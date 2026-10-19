# taskmgr-rust

A simple Linux task manager that shows what is *actually* running.

- Resolves processes hidden behind wrappers (Flatpak/bwrap, Wine/Proton,
  Steam pressure-vessel, AppImage, NixOS `-wrapped` binaries) and groups them
  by application.
- Shows every disk, temperature sensor and GPU separately, with readable
  names.

Pages:

- **Processes**: applications, background and system processes, grouped by
  application with their window titles (KDE Plasma), heat-mapped CPU, memory
  and disk columns, search, sorting, end task.
- **Performance**: one graph per CPU, memory, disk, network adapter and GPU,
  plus every temperature sensor.
- **System**: a summary of the machine.

Never run it as root. CPU power needs read access to RAPL, see
`docs/energy.md`.

## Without Nix (AppImage)

```sh
nix build .#appimage   # result/taskmgr-x86_64.AppImage, about 55 MB
```

The AppImage keeps only what the app loads (see `packaging/appimage.sh`):
standard Qt controls with Breeze colours, software rendering, no KDE
frameworks. It follows the desktop's light or dark preference.

## Install (Nix)

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
cargo run -p taskmgr-core --example windows    # window list from KWin
nix build                                      # package with desktop file
nix build .#probes.x86_64-linux.native         # static core binaries (data collection)
```

Layout:

- `crates/taskmgr-core`: sampling of `/proc` and `/sys` (CPU, memory, disks,
  network, GPUs, sensors), identity resolver, KWin window list. No root, no
  global state.
- `crates/taskmgr`: Qt Quick / Kirigami UI, bridged with cxx-qt.

License: MIT (see `LICENSE`).
