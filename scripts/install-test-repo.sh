#!/bin/sh
# Install (or update) Tessera from the test repository unpacked next to this
# script, and the driver extensions named on the command line. Run it from
# the folder where tessera-repo.tar.gz was extracted.
#
#   sh install-test-repo.sh                    Tessera only
#   sh install-test-repo.sh SafeSign SafeNet   Tessera and those drivers
#   sh install-test-repo.sh all                Tessera and every driver
set -eu
here=$(cd "$(dirname "$0")" && pwd)
flatpak remote-add --user --if-not-exists --no-gpg-verify tessera-test "file://$here/repo"
flatpak remote-modify --user --url="file://$here/repo" tessera-test
flatpak update --user --appstream tessera-test >/dev/null 2>&1 || true
flatpak install --user --or-update -y tessera-test io.github.pedrohqb.Tessera

available=$(flatpak remote-ls --user --columns=application tessera-test 2>/dev/null |
    sed -n 's/^io\.github\.pedrohqb\.Tessera\.Driver\.//p' | sort -u)
wanted="$*"
[ "$wanted" = all ] && wanted=$available
for name in $wanted; do
    flatpak install --user --or-update -y tessera-test "io.github.pedrohqb.Tessera.Driver.$name"
done

echo
echo "Installed. Open Tessera from the menu, or run: flatpak run io.github.pedrohqb.Tessera"
if [ -z "$wanted" ]; then
    echo "Drivers in this repository (install with: sh $0 NAME...):"
    for name in $available; do echo "  $name"; done
fi
echo "After installing a driver, quit Tessera from its menu and open it again."
