{
  description = "taskmgr-rust — wrapper-aware Linux task manager";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  # AppImage runtime only (static ELF that mounts the image).
  inputs.nix-appimage.url = "github:ralismark/nix-appimage";

  outputs = { self, nixpkgs, nix-appimage }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (s: f nixpkgs.legacyPackages.${s});

      # Runtime QML modules and Qt plugins. The Breeze Qt Quick style draws
      # the same on every desktop; no platform theme from the desktop.
      runtimeQt = pkgs: with pkgs.kdePackages; [
        qtbase
        qtdeclarative
        qtwayland
        qtsvg
        kirigami
        qqc2-breeze-style
        kguiaddons
        breeze-icons
        kiconthemes
      ];

    in {
      packages = forAll (pkgs: {
        # For distributions without Nix, see packaging/appimage.sh.
        appimage = pkgs.runCommand "taskmgr-appimage" {
          nativeBuildInputs = [ pkgs.patchelf pkgs.squashfsTools pkgs.glibc.bin pkgs.binutils ];
          APP = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/.taskmgr-wrapped";
          HELPER = "${self.probes.${pkgs.stdenv.hostPlatform.system}.native}/bin/taskmgr-helper";
          QTBASE = pkgs.kdePackages.qtbase;
          QTDECL = pkgs.kdePackages.qtdeclarative;
          QTWAYLAND = pkgs.kdePackages.qtwayland;
          QTSVG = pkgs.kdePackages.qtsvg;
          KIRIGAMI = pkgs.kdePackages.kirigami.unwrapped or pkgs.kdePackages.kirigami;
          BREEZESTYLE = pkgs.kdePackages.qqc2-breeze-style;
          GUIADDONS = pkgs.kdePackages.kguiaddons;
          # Without the 25 MB library of every Breeze icon: the few the
          # style asks for are copied as files.
          ICONTHEMES = pkgs.kdePackages.kiconthemes.overrideAttrs (o: {
            cmakeFlags = (o.cmakeFlags or [ ]) ++ [ "-DUSE_BreezeIcons=OFF" ];
          });
          BREEZE = pkgs.kdePackages.breeze-icons;
          EXTRA = "${pkgs.xcb-util-cursor}/lib/libxcb-cursor.so.0";
          RUNTIME = nix-appimage.packages.${pkgs.stdenv.hostPlatform.system}.appimage-runtimes.appimage-type2-runtime;
          SRC = self;
        } "bash ${./packaging/appimage.sh}";
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "taskmgr";
          version = "0.1.3";
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "taskmgr" "-p" "taskmgr-core" "--bins" ];
          cargoTestFlags = [ "--workspace" ];
          env.CXXFLAGS = "-Wno-sfinae-incomplete";

          nativeBuildInputs = [ pkgs.pkg-config pkgs.kdePackages.wrapQtAppsHook pkgs.kdePackages.qttools ];
          buildInputs = runtimeQt pkgs;
          # The desktop's own Kirigami may be built against another Qt.
          qtWrapperArgs = [ "--unset" "QML2_IMPORT_PATH" "--unset" "QML_IMPORT_PATH" ];

          preBuild = ''
            export QMAKE="${pkgs.qt6.env "taskmgr-qt" (with pkgs.qt6; [ qtdeclarative qtwayland qtsvg ])}/bin/qmake"
          '';

          postInstall = ''
            install -Dm644 packaging/fr.rsman.taskmgr-rust.desktop $out/share/applications/fr.rsman.taskmgr-rust.desktop
            install -Dm644 packaging/fr.rsman.taskmgr-rust.svg $out/share/icons/hicolor/scalable/apps/fr.rsman.taskmgr-rust.svg
          '';

          meta = {
            description = "Linux task manager that resolves processes through Flatpak, Wine, Steam and AppImage";
            mainProgram = "taskmgr";
            platforms = pkgs.lib.platforms.linux;
          };
        };
      });


      # Static core binaries without Qt, to collect data on other machines.
      probes = forAll (pkgs:
        let
          probe = p: p.pkgsStatic.rustPlatform.buildRustPackage {
            pname = "taskmgr-probe";
            version = "0.1.3";
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
          # GCC 16 warns inside Qt's own headers.
          CXXFLAGS = "-Wno-sfinae-incomplete";
          shellHook = ''
            # cxx-qt needs one qmake prefix holding every linked module; set
            # here because qtbase's setup hook overrides QMAKE.
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
