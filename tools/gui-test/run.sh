#!/bin/bash
# Run Tessera on a virtual display (Xvfb) with its own D-Bus session, a fake
# tray host and test drivers, for screenshots and clicks with xdotool.
#
#   tools/gui-test/run.sh [tessera arguments]     (as a normal user, not root:
#                                                  p11-kit skips user config for root)
# Then, in another terminal:  . /tmp/tessera-gui/env.sh; import -window root shot.png
#
# Needs: Xvfb, xdotool, ImageMagick (import), dbus-run-session, python3-gi,
# softhsm2 (for the test driver), a debug build in target/debug/tessera.
set -eu
repo=$(cd "$(dirname "$0")/../.." && pwd)
H=${TESSERA_GUI_DIR:-/tmp/tessera-gui}
mkdir -p "$H/run" "$H/modules" "$H/drivers/softhsm" "$H/hsm"
chmod 700 "$H/run"
if [ ! -f "$H/hsm/conf" ]; then
    printf 'directories.tokendir = %s/hsm\nobjectstore.backend = file\n' "$H" > "$H/hsm/conf"
    SOFTHSM2_CONF=$H/hsm/conf softhsm2-util --init-token --free --label Test --pin 1234 --so-pin 1234 >/dev/null
fi
cat > "$H/drivers/softhsm/driver.toml" <<EOT
name = "SoftHSM (test)"
vendor = "OpenDNSSEC"
description = "Software token for tests"
module = "/usr/lib/softhsm/libsofthsm2.so"
[env]
SOFTHSM2_CONF = "$H/hsm/conf"
EOT
cat > "$H/env.sh" <<EOT
export DISPLAY=:99 XDG_RUNTIME_DIR=$H/run GDK_BACKEND=x11 GSK_RENDERER=cairo NO_AT_BRIDGE=1 \\
  TESSERA_DRIVERS_PATH=$H/drivers TESSERA_HOST_MODULES=$H/modules LANG=\${LANG:-pt_BR.UTF-8}
[ -f $H/bus ] && export DBUS_SESSION_BUS_ADDRESS=\$(cat $H/bus) || true
EOT
. "$H/env.sh"
Xvfb :99 -screen 0 1100x1300x24 >/dev/null 2>&1 &
sleep 1
exec dbus-run-session -- bash -c "
  $(command -v python3.12 || echo python3) '$repo/tools/gui-test/fake-watcher.py' '$H/watcher.log' &
  sleep 0.5; echo \$DBUS_SESSION_BUS_ADDRESS > '$H/bus'
  '$repo/target/debug/tessera' $* > '$H/tessera.log' 2>&1"
