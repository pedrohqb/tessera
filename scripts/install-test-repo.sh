#!/bin/sh
# Install (or update) Tessera from the test repository unpacked next to this
# script, and the extensions (drivers and web signers) named on the command
# line. Run it from the folder where tessera-repo.tar.gz was extracted.
#
#   sh install-test-repo.sh                    Tessera only
#   sh install-test-repo.sh SafeSign WebPKI    Tessera and those extensions
#   sh install-test-repo.sh all                Tessera and every extension
set -eu
here=$(cd "$(dirname "$0")" && pwd)
flatpak remote-add --user --if-not-exists --no-gpg-verify tessera-test "file://$here/repo"
flatpak remote-modify --user --url="file://$here/repo" tessera-test
flatpak update --user --appstream tessera-test >/dev/null 2>&1 || true
flatpak install --user --or-update -y tessera-test io.github.pedrohqb.Tessera

refs=$(flatpak remote-ls --user --columns=application tessera-test 2>/dev/null |
    grep -E '^io\.github\.pedrohqb\.Tessera\.(Driver|Signer)\.' | sort -u)
available=$(echo "$refs" | sed 's/^io\.github\.pedrohqb\.Tessera\.[A-Za-z]*\.//')
wanted="$*"
[ "$wanted" = all ] && wanted=$available
for name in $wanted; do
    ref=$(echo "$refs" | grep -E "\.(Driver|Signer)\.$name\$" | head -n1)
    [ -n "$ref" ] || { echo "Not in this repository: $name"; exit 1; }
    flatpak install --user --or-update -y tessera-test "$ref"
done

echo
echo "Installed. Open Tessera from the menu, or run: flatpak run io.github.pedrohqb.Tessera"
if [ -z "$wanted" ]; then
    echo "Extensions in this repository (install with: sh $0 NAME...):"
    for name in $available; do echo "  $name"; done
fi
echo "After installing an extension with Tessera open, use Restart Tessera in its menu."
