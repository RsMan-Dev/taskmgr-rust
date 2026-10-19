#!/usr/bin/env bash
# Builds a self-contained AppImage holding only what the app loads: the
# binary, the shared libraries it resolves to (glibc included, started
# through the bundled loader), a few Qt plugins and the QML modules it
# imports. Called by the flake (`nix build .#appimage`), which sets:
#   APP       unwrapped taskmgr binary
#   HELPER    static taskmgr-helper
#   QTBASE QTDECL QTWAYLAND QTSVG KIRIGAMI BREEZE   package prefixes
#   EXTRA     libraries opened at run time by plugins (xcb-cursor)
#   RUNTIME   AppImage type2 runtime
#   SRC       source tree (desktop file, icon)
#   out       output directory
set -euo pipefail

dir=$PWD/AppDir
lib=$dir/usr/lib
mkdir -p "$dir/usr/bin" "$lib" "$dir/usr/plugins" "$dir/usr/qml"

install -m755 "$APP" "$dir/usr/bin/taskmgr"
install -m755 "$HELPER" "$dir/usr/bin/taskmgr-helper"

copy_tree() { # <from> <to>: dereferenced copy, writable
    mkdir -p "$(dirname "$2")"
    cp -rL "$1" "$2"
    chmod -R u+w "$2"
}

# Qt moves plugins between modules across versions: take each one from
# whichever module has it, and stop if a required one is missing.
plugin() { # <path under plugins/> [optional]
    local found=
    for m in "$QTBASE" "$QTWAYLAND" "$QTSVG"; do
        local src=$m/lib/qt-6/plugins/$1
        [ -e "$src" ] || continue
        found=1
        if [ -d "$src" ]; then
            mkdir -p "$dir/usr/plugins/$1"
            cp -rL "$src/." "$dir/usr/plugins/$1/"
        else
            install -Dm755 "$src" "$dir/usr/plugins/$1"
        fi
    done
    [ -n "$found" ] || [ "${2:-}" = optional ] || { echo "missing Qt plugin $1" >&2; exit 1; }
}
plugin platforms/libqwayland.so
plugin platforms/libqxcb.so
plugin platforms/libqoffscreen.so
plugin wayland-shell-integration
plugin wayland-decoration-client
plugin platformthemes/libqxdgdesktopportal.so
plugin platforminputcontexts/libcomposeplatforminputcontextplugin.so optional
plugin platforminputcontexts/libibusplatforminputcontextplugin.so optional
plugin imageformats/libqsvg.so
plugin imageformats/libqjpeg.so
plugin iconengines/libqsvgicon.so
chmod -R u+w "$dir/usr/plugins"

qml=$QTDECL/lib/qt-6/qml
for m in QtQml QtCore QtQuick/Window QtQuick/Layouts QtQuick/Templates QtQuick/Shapes \
         QtQuick/Controls/impl QtQuick/Controls/Basic QtQuick/Controls/Fusion; do
    copy_tree "$qml/$m" "$dir/usr/qml/$m"
done
# Module roots without their optional submodules (styles, effects, ...).
for m in QtQuick QtQuick/Controls; do
    find "$qml/$m" -maxdepth 1 -type f -exec cp -L {} "$dir/usr/qml/$m/" \;
done
chmod -R u+w "$dir/usr/qml"
copy_tree "$KIRIGAMI/lib/qt-6/qml/org/kde/kirigami" "$dir/usr/qml/org/kde/kirigami"
# Kirigami takes its colours from styles/<Qt Quick style>/Theme.qml.
rm -rf "$dir/usr/qml/org/kde/kirigami/styles/Material"
cp -r "$dir/usr/qml/org/kde/kirigami/styles/org.kde.desktop" "$dir/usr/qml/org/kde/kirigami/styles/Fusion"
# Information messages are framed with this colour, white there.
sed -i 's/activeTextColor: palette.highlightedText/activeTextColor: palette.highlight/' \
    "$dir/usr/qml/org/kde/kirigami/styles/Fusion/Theme.qml"
find "$dir/usr/qml" \( -name '*.qmltypes' -o -name '*.debug' \) -delete

# Icons Kirigami controls ask for by name; application icons come from the
# host's hicolor theme, which Breeze inherits.
for theme in breeze breeze-dark; do
    t=$BREEZE/share/icons/$theme
    mkdir -p "$dir/usr/share/icons/$theme"
    cp "$t/index.theme" "$dir/usr/share/icons/$theme/"
    for n in dialog-close edit-clear edit-clear-locationbar-ltr edit-clear-locationbar-rtl edit-find \
             search emblem-error emblem-information emblem-success emblem-warning network-disconnect; do
        (cd "$t" && find -L . -name "$n.svg") | while read -r f; do
            install -Dm644 "$t/$f" "$dir/usr/share/icons/$theme/$f"
        done
    done
done

# Shared libraries of every ELF file, resolved from the store.
interp=$(patchelf --print-interpreter "$APP")
cp -L "$interp" "$lib/"
for l in $EXTRA; do cp -L "$l" "$lib/"; done
elves=$(find "$dir/usr/bin/taskmgr" "$dir/usr/plugins" "$dir/usr/qml" "$lib" -name '*.so*' -o -name taskmgr -type f)
for f in $elves; do
    ldd "$f" | awk '/=> \//{print $3}'
done | sort -u | while read -r l; do
    cp -Ln "$l" "$lib/"
done
chmod -R u+w "$lib"
find "$dir" -type f \( -name '*.so*' -o -path '*/bin/*' \) -exec strip --strip-unneeded {} \; 2>/dev/null || true

loader=$(basename "$interp")
cat > "$dir/AppRun" <<EOF
#!/bin/sh
here=\$(dirname "\$(readlink -f "\$0")")
export TASKMGR_PORTABLE=1
export QT_QUICK_BACKEND=\${QT_QUICK_BACKEND:-software}
export QT_PLUGIN_PATH="\$here/usr/plugins"
export QML_IMPORT_PATH="\$here/usr/qml" QML2_IMPORT_PATH="\$here/usr/qml"
export XDG_DATA_DIRS="\$here/usr/share:\${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
unset QT_QPA_PLATFORMTHEME QT_STYLE_OVERRIDE QT_QUICK_CONTROLS_STYLE
[ -e /etc/fonts/fonts.conf ] && export FONTCONFIG_FILE=/etc/fonts/fonts.conf
[ -z "\$LOCALE_ARCHIVE" ] && [ -e /usr/lib/locale/locale-archive ] && export LOCALE_ARCHIVE=/usr/lib/locale/locale-archive
[ -d /usr/share/X11/xkb ] && export XKB_CONFIG_ROOT=/usr/share/X11/xkb
# pkexec runs the helper as root, and root cannot enter the user's FUSE
# mount: hand it a copy outside the image.
if [ -n "\$XDG_RUNTIME_DIR" ] && [ -O "\$XDG_RUNTIME_DIR" ]; then
    helper="\$XDG_RUNTIME_DIR/taskmgr-helper"
    cmp -s "\$here/usr/bin/taskmgr-helper" "\$helper" || install -m755 "\$here/usr/bin/taskmgr-helper" "\$helper"
    export TASKMGR_HELPER="\$helper"
fi
# Host directories last, for libraries opened at run time (NVML).
hostlibs=/usr/lib/x86_64-linux-gnu:/usr/lib64:/usr/lib:/run/opengl-driver/lib
exec "\$here/usr/lib/$loader" --argv0 "\$0" --library-path "\$here/usr/lib:\$hostlibs" "\$here/usr/bin/taskmgr" "\$@"
EOF
chmod 755 "$dir/AppRun"

cp "$SRC/packaging/fr.rsman.taskmgr-rust.desktop" "$dir/"
cp "$SRC/packaging/fr.rsman.taskmgr-rust.svg" "$dir/"
ln -s fr.rsman.taskmgr-rust.svg "$dir/.DirIcon"

mksquashfs "$dir" image.squashfs -root-owned -noappend -comp zstd -Xcompression-level 19 -quiet
mkdir -p "$out"
cat "$RUNTIME" image.squashfs > "$out/taskmgr-x86_64.AppImage"
chmod 755 "$out/taskmgr-x86_64.AppImage"
