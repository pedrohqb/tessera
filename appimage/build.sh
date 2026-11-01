#!/bin/bash
# Build Tessera-x86_64.AppImage with linuxdeploy and its GTK plugin.
#
#   appimage/build.sh            (on Ubuntu 24.04 or newer, as in CI)
#
# Needs: cargo, libgtk-4-dev, libadwaita-1-dev, libpcsclite-dev,
# libarchive-tools (bsdtar), p11-kit, p11-kit-modules, curl, file.
# The AppImage runs on systems with glibc 2.39 or newer (Ubuntu 24.04,
# Fedora 40, Debian 13 and later).
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"
work=${APPIMAGE_WORK:-$repo/target/appimage}
tools=$work/tools
appdir=$work/AppDir
mkdir -p "$tools"
rm -rf "$appdir"

# linuxdeploy and its GTK plugin (downloaded once).
if [ ! -x "$tools/linuxdeploy" ]; then
    curl -fsSL -o "$tools/linuxdeploy" \
        https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
    curl -fsSL -o "$tools/linuxdeploy-plugin-gtk.sh" \
        https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh
    chmod +x "$tools/linuxdeploy" "$tools/linuxdeploy-plugin-gtk.sh"
fi

cargo build --release --locked

# Tessera's own files: the driver descriptions, the unpacking script and
# the pins (from the Flatpak manifest, the single source of the pins).
share=$appdir/usr/share/tessera
mkdir -p "$share"
install -m755 flatpak/apply_extra.sh "$share/apply_extra.sh"
python3 appimage/drivers-list.py flatpak/io.github.pedrohqb.Tessera.yml > "$share/drivers.list"
for d in flatpak/drivers/*/; do
    install -Dm644 "$d/driver.toml" "$share/drivers/$(basename "$d")/driver.toml"
done
install -Dm644 data/io.github.pedrohqb.Tessera.metainfo.xml \
    "$appdir/usr/share/metainfo/io.github.pedrohqb.Tessera.appdata.xml"
install -Dm644 data/icons/io.github.pedrohqb.Tessera-symbolic.svg \
    "$appdir/usr/share/icons/hicolor/symbolic/apps/io.github.pedrohqb.Tessera-symbolic.svg"

# p11-kit's server helper and client module, used when the system lacks them.
helper=
for c in /usr/libexec/p11-kit/p11-kit-server /usr/lib/x86_64-linux-gnu/p11-kit/p11-kit-server; do
    if [ -x "$c" ]; then helper=$c; break; fi
done
[ -n "$helper" ] || { echo "p11-kit-server not found (install p11-kit)" >&2; exit 1; }
client=/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so

export DEPLOY_GTK_VERSION=4
# The vendor drivers talk to the system's pcscd: its client library must be
# the system's, not a copy.
export NO_STRIP=1
cd "$work"
"$tools/linuxdeploy" --appdir "$appdir" \
    --executable "$repo/target/release/tessera" \
    --executable "$(command -v bsdtar)" \
    --executable "$helper" \
    --library "$client" \
    --desktop-file "$repo/data/io.github.pedrohqb.Tessera.desktop" \
    --icon-file "$repo/data/icons/io.github.pedrohqb.Tessera.svg" \
    --custom-apprun "$repo/appimage/AppRun" \
    --exclude-library 'libpcsclite*' \
    --plugin gtk \
    --output appimage
mv -f Tessera*-x86_64.AppImage "$repo/Tessera-x86_64.AppImage"
echo "Built $repo/Tessera-x86_64.AppImage"
