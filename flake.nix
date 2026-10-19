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
      packages = forAll (pkgs: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "taskmgr";
          version = "0.1.0";
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "taskmgr" "-p" "taskmgr-core" "--bins" ];
          cargoTestFlags = [ "--workspace" ];

          nativeBuildInputs = [ pkgs.pkg-config pkgs.kdePackages.wrapQtAppsHook pkgs.kdePackages.qttools ];
          buildInputs = runtimeQt pkgs;
          # The session's QML paths point at the desktop's own Kirigami, which
          # may be built against another Qt and would shadow ours.
          qtWrapperArgs = [ "--unset" "QML2_IMPORT_PATH" "--unset" "QML_IMPORT_PATH" ];

          preBuild = ''
            export QMAKE="${pkgs.qt6.env "taskmgr-qt" (with pkgs.qt6; [ qtdeclarative qtwayland qtsvg ])}/bin/qmake"
          '';

          postInstall = ''
            install -Dm644 packaging/fr.rsman.taskmgr-rust.desktop $out/share/applications/fr.rsman.taskmgr-rust.desktop
          '';

          meta = {
            description = "Linux task manager that resolves processes through Flatpak, Wine, Steam and AppImage";
            mainProgram = "taskmgr";
            platforms = pkgs.lib.platforms.linux;
          };
        };
      });

      # Static core binaries (examples + network helper), no Qt: runs on any
      # Linux of the same architecture. Used to collect data on servers.
      probes = forAll (pkgs:
        let
          probe = p: p.pkgsStatic.rustPlatform.buildRustPackage {
            pname = "taskmgr-probe";
            version = "0.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "-p" "taskmgr-core" "--examples" "--bins" ];
            doCheck = false;
            postInstall = ''
              find target -path '*/release/examples/*' -maxdepth 4 -type f -executable ! -name '*-*' \
                -exec install -Dm755 -t $out/bin {} \;
            '';
          };
        in {
          native = probe pkgs;
          aarch64 = probe pkgs.pkgsCross.aarch64-multiplatform;
        });

      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            rust-analyzer
            pkg-config
            kdePackages.qttools
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
