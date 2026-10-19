{
  description = "taskmgr-rust — wrapper-aware Linux task manager";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (s: f nixpkgs.legacyPackages.${s});

      # QML modules and Qt plugins needed at runtime: the native KDE look
      # (Breeze style, platform theme, icons) and the chart library.
      runtimeQt = pkgs: with pkgs.kdePackages; [
        qtbase
        qtdeclarative
        qtwayland
        qtsvg
        kirigami
        qqc2-desktop-style
        kquickcharts
        breeze
        breeze-icons
        plasma-integration
        kiconthemes
      ];
    in {
      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            rust-analyzer
            pkg-config
          ] ++ runtimeQt pkgs;
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          # Debug C++ builds (cxx-qt) otherwise warn on every file.
          hardeningDisable = [ "fortify" ];
          # Replace (not extend) the session paths: the desktop's own plugins
          # may be built against a different Qt and fail to load.
          shellHook = ''
            # cxx-qt finds Qt modules through qmake: it needs a single prefix
            # holding every module we link (Qml and Quick live outside
            # qtbase). Set here because qtbase's setup hook overrides QMAKE.
            export QMAKE="${pkgs.qt6.env "taskmgr-qt" (with pkgs.qt6; [ qtdeclarative qtwayland qtsvg ])}/bin/qmake"
            export QT_PLUGIN_PATH="${pkgs.lib.makeSearchPath "lib/qt-6/plugins" (runtimeQt pkgs)}"
            export QML_IMPORT_PATH="${pkgs.lib.makeSearchPath "lib/qt-6/qml" (runtimeQt pkgs)}"
            export QML2_IMPORT_PATH="$QML_IMPORT_PATH"
            export XDG_DATA_DIRS="${pkgs.lib.makeSearchPath "share" (runtimeQt pkgs)}:$XDG_DATA_DIRS"
          '';
        };
      });
    };
}
