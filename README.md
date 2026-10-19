# taskmgr-rust

A simple Linux task manager that shows what is *actually* running.

- Resolves processes hidden behind wrappers (Flatpak/bwrap, Wine/Proton,
  Steam pressure-vessel, AppImage, NixOS `-wrapped` binaries) and groups them
  by application.
- Shows every disk, temperature sensor and GPU separately, with readable
  names.

Status: early prototype. See `docs/` for design notes.

## Development

```sh
nix develop          # Rust toolchain, Qt 6, Kirigami
cargo run -p taskmgr
cargo run -p taskmgr-core --example snapshot   # raw measurements as JSON
cargo run -p taskmgr-core --example processes  # processes grouped by application
```

Layout:

- `crates/taskmgr-core`: sampling of `/proc` and `/sys` (CPU, memory, disks,
  network, GPUs, sensors). No root, no global state.
- `crates/taskmgr`: Qt Quick / Kirigami UI, bridged with cxx-qt.

License: TBD.
