#!/bin/sh
# Copies the Adwaita symbolic icons the window uses (every "…-symbolic" name
# in src/) into the app's hicolor theme.
#
# GTK 4 looks for an icon only in the desktop's theme, the themes it
# inherits from and hicolor; it no longer falls back to Adwaita. On Plasma
# the theme is Breeze, which lacks some of these names (web-browser-symbolic,
# application-x-addon-symbolic…), and GTK drew a "missing image" in their
# place. A theme that has the icon still wins, so on Plasma the window keeps
# Breeze's icons wherever Breeze has them.
#
# Run from the source folder, in the build (the SDK's /usr has Adwaita).
set -eu
dest=/app/share/icons/hicolor/scalable/apps
mkdir -p "$dest"
for name in $(grep -rohE '"[a-z0-9-]+-symbolic"' src | tr -d '"' | sort -u); do
    src=$(find /usr/share/icons/Adwaita -name "$name.svg" | head -n 1)
    if [ -z "$src" ]; then
        echo "fallback-icons: $name not found in Adwaita" >&2
        exit 1
    fi
    install -m644 "$src" "$dest/$name.svg"
done
