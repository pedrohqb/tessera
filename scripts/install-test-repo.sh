#!/bin/sh
# Install (or update) Tessera from the test repository unpacked next to this
# script. Run it from the folder where tessera-repo.tar.gz was extracted.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
flatpak remote-add --user --if-not-exists --no-gpg-verify tessera-test "file://$here/repo"
flatpak remote-modify --user --url="file://$here/repo" tessera-test
flatpak update --user --appstream tessera-test >/dev/null 2>&1 || true
flatpak install --user --or-update -y tessera-test io.github.pedrohqb.Tessera
echo
echo "Installed. Open Tessera from the menu, or run: flatpak run io.github.pedrohqb.Tessera"
