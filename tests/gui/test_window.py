#!/usr/bin/env python3
"""Drive Tessera's window through the accessibility tree (AT-SPI) and check
that its buttons do what they say, on a made-up home folder.

    xvfb-run -a dbus-run-session -- python3 tests/gui/test_window.py [TESSERA]

TESSERA defaults to target/debug/tessera. Needs at-spi2-core, python3-pyatspi,
libnss3-tools (certutil) and p11-kit-modules. Exits 1 on the first failure.

What it checks:
- Native tab: Firefox's Set up writes p11-kit-proxy.so into the profile and
  the row turns Ready; Okular and Papers have no button, wait for Firefox
  ("Set up Firefox first") and turn Ready with it; PJeOffice Pro's Set up writes p11-kit's
  path for this distribution into list.a3.
- Flatpak tab: the Firefox Flatpak's step 2 registers p11-kit-client.so in
  its profile.
"""
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import pyatspi

REPO = Path(__file__).resolve().parents[2]
TESSERA = Path(sys.argv[1] if len(sys.argv) > 1 else REPO / "target/debug/tessera").resolve()
CLIENT = "/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so"


APP = None


def dump(root, depth=0, out=None):
    """The accessibility tree, for a failure's report."""
    out = [] if out is None else out
    try:
        state = "" if showing(root) else " (hidden)"
        out.append(f"{'  ' * depth}{root.getRoleName()} {root.name!r} {root.description!r}{state}")
        for i in range(root.childCount):
            dump(root[i], depth + 1, out)
    except Exception:  # noqa: BLE001
        pass
    return out


def fail(msg):
    print(f"FAIL: {msg}", flush=True)
    if APP is not None and os.environ.get("TESSERA_GUI_DUMP"):
        print("\n".join(dump(APP)), flush=True)
    sys.exit(1)


def ok(msg):
    print(f"ok: {msg}", flush=True)


def wait(what, check, timeout=20):
    end = time.time() + timeout
    while time.time() < end:
        try:
            value = check()
        except Exception:  # noqa: BLE001 - the tree changes under our feet
            value = None
        if value:
            return value
        time.sleep(0.3)
    fail(f"timed out waiting for {what}")


def nodes(root):
    """Every accessible under root (depth first)."""
    stack = [root]
    while stack:
        node = stack.pop()
        if node is None:
            continue
        yield node
        try:
            stack.extend(node[i] for i in range(node.childCount - 1, -1, -1))
        except Exception:  # noqa: BLE001
            pass


def showing(node):
    return node.getState().contains(pyatspi.STATE_SHOWING)


def labels(row):
    """The texts of a row's labels (title, then subtitle)."""
    out = []
    for n in nodes(row):
        try:
            if n.getRoleName() == "label" and n.name:
                out.append(n.name)
        except Exception:  # noqa: BLE001
            continue
    return out


def subtitle(row):
    texts = labels(row)
    return texts[1] if len(texts) > 1 else ""


def find(root, role=None, name=None, sub=None, only_showing=True):
    """The first accessible with this role and name (and, for a row, a
    subtitle starting with `sub`)."""
    for n in nodes(root):
        try:
            if role is not None and n.getRoleName() != role:
                continue
            if name is not None and n.name != name:
                continue
            if sub is not None and not subtitle(n).startswith(sub):
                continue
            if only_showing and not showing(n):
                continue
            return n
        except Exception:  # noqa: BLE001
            continue
    return None


def press(node):
    action = node.queryAction()
    for i in range(action.nActions):
        if action.getName(i) in ("click", "press", "activate", "toggle"):
            action.doAction(i)
            return
    action.doAction(0)


def focused(node):
    try:
        return node.getState().contains(pyatspi.STATE_FOCUSED)
    except Exception:  # noqa: BLE001
        return False


def activate_by_keyboard(node, presses=80):
    """Tab through the window until `node` has the focus, then press Space:
    for widgets AT-SPI offers no action for (an expander row's header).
    Needs xdotool."""
    subprocess.run(
        ["xdotool", "search", "--name", "^Tessera$", "windowfocus", "--sync"],
        check=False,
        stderr=subprocess.DEVNULL,
    )
    for _ in range(presses):
        if focused(node):
            subprocess.run(["xdotool", "key", "space"], check=True)
            return
        subprocess.run(["xdotool", "key", "Tab"], check=True)
        time.sleep(0.15)
    fail(f"could not reach {node.name!r} with the keyboard")


def make_home(home):
    # Native Firefox: one profile with its certificate database, as Firefox
    # makes it the first time (which Okular and Papers read).
    ff = home / ".mozilla/firefox"
    (ff / "abc.default-release").mkdir(parents=True)
    (ff / "profiles.ini").write_text("[Profile0]\nPath=abc.default-release\nIsRelative=1\n")
    subprocess.run(
        ["certutil", "-N", "-d", f"sql:{ff / 'abc.default-release'}", "--empty-password"],
        check=True,
    )
    # Native PJeOffice Pro's settings, as it writes them.
    (home / ".pjeoffice-pro").mkdir()
    (home / ".pjeoffice-pro/pjeoffice-pro.config").write_text(
        "#Salvo em 2026-11-01\nlist.a3=\nlist.a1=\n"
    )
    # The Firefox Flatpak's profile.
    fff = home / ".var/app/org.mozilla.firefox/.mozilla/firefox"
    (fff / "flat.default-release").mkdir(parents=True)
    (fff / "profiles.ini").write_text("[Profile0]\nPath=flat.default-release\nIsRelative=1\n")


def expected_pjeoffice_library():
    text = Path("/etc/os-release").read_text()
    ids = []
    for line in text.splitlines():
        if line.startswith(("ID=", "ID_LIKE=")):
            ids += line.split("=", 1)[1].strip('"').lower().split()
    if {"debian", "ubuntu"} & set(ids):
        return "/usr/lib/x86_64-linux-gnu/p11-kit-proxy.so"
    if {"fedora", "rhel", "centos", "suse", "opensuse", "mageia"} & set(ids):
        return "/usr/lib64/p11-kit-proxy.so"
    return "/usr/lib/p11-kit-proxy.so"


OLD_P11KIT_BANNER = "On this system signing does not work (old p11-kit); see the project's documentation"


def old_p11kit_host():
    """The same rule as native.rs (old_p11kit_by_release), on this host."""
    text = Path("/etc/os-release").read_text()
    ids, codenames = [], []
    for line in text.splitlines():
        key, _, value = line.partition("=")
        value = value.strip('"').lower()
        if key in ("ID", "ID_LIKE"):
            ids += value.split()
        if key in ("VERSION_CODENAME", "UBUNTU_CODENAME", "DEBIAN_CODENAME"):
            codenames.append(value)
    old = {"bullseye", "bookworm", "trixie", "focal", "jammy", "noble"}
    return bool({"debian", "ubuntu"} & set(ids)) and bool(old & set(codenames))


def main():
    if not TESSERA.exists():
        fail(f"{TESSERA} not found (cargo build first)")
    tmp = Path(tempfile.mkdtemp(prefix="tessera-gui-"))
    home = tmp / "home"
    run = tmp / "run"
    home.mkdir()
    run.mkdir(mode=0o700)
    make_home(home)
    env = dict(
        os.environ,
        HOME=str(home),
        XDG_CONFIG_HOME=str(home / ".config"),
        XDG_DATA_HOME=str(home / ".local/share"),
        XDG_RUNTIME_DIR=str(run),
        TESSERA_DRIVERS_PATH=str(tmp / "drivers"),
        TESSERA_HOST_MODULES=str(tmp / "modules"),
        LANG="C.UTF-8",
        LC_ALL="C.UTF-8",
        GDK_BACKEND="x11",
        GSK_RENDERER="cairo",
        GTK_A11Y="atspi",
    )
    env.pop("NO_AT_BRIDGE", None)
    log = open(tmp / "tessera.log", "w")
    proc = subprocess.Popen([str(TESSERA)], env=env, stdout=log, stderr=subprocess.STDOUT)
    try:
        desktop = pyatspi.Registry.getDesktop(0)
        app = wait(
            "Tessera on the accessibility bus",
            lambda: next(
                (a for a in desktop if a is not None and a.name.lower() in ("tessera", TESSERA.name)),
                None,
            ),
        )
        global APP
        APP = app
        ok("window on the accessibility bus")

        # --- The banner for systems whose p11-kit is too old ---------------
        if old_p11kit_host():
            wait("the old p11-kit banner", lambda: find(app, role="label", name=OLD_P11KIT_BANNER))
            ok("old p11-kit banner shown on this host")
        else:
            time.sleep(1)
            if find(app, role="label", name=OLD_P11KIT_BANNER):
                fail("old p11-kit banner shown on a host that does not need it")
            ok("no old p11-kit banner on this host")

        # --- Native tab ----------------------------------------------------
        press(wait("the Native tab", lambda: find(app, role="page tab", name="Native")))
        ff_row = wait(
            "the native Firefox row",
            lambda: find(app, role="list item", name="Firefox", sub="Native app"),
        )
        pdf = wait("the Okular and Papers row", lambda: find(app, role="list item", name="Okular and Papers"))
        if not subtitle(pdf).endswith("Set up Firefox first"):
            fail(f"Okular and Papers before Firefox: {subtitle(pdf)!r}")
        ok("Okular and Papers: wait for Firefox")
        if find(pdf, role="push button", name="Set up"):
            fail("Okular and Papers has a Set up button of its own")
        press(wait("Firefox's Set up", lambda: find(ff_row, role="push button", name="Set up")))
        profile = home / ".mozilla/firefox/abc.default-release/pkcs11.txt"
        wait(
            "p11-kit-proxy.so in the native Firefox profile",
            lambda: profile.exists() and "library=p11-kit-proxy.so" in profile.read_text(),
        )
        ok("native Firefox: p11-kit-proxy.so registered")
        wait(
            "the native Firefox row to turn Ready",
            lambda: subtitle(find(app, role="list item", name="Firefox", sub="Native app")).endswith("Ready"),
        )
        wait(
            "Okular and Papers to turn Ready with Firefox",
            lambda: subtitle(find(app, role="list item", name="Okular and Papers")).endswith("Ready"),
        )
        ok("native Firefox, Okular and Papers: Ready")

        pje = wait(
            "the PJeOffice Pro row",
            lambda: find(app, role="list item", name="PJeOffice Pro", sub="Native app"),
        )
        press(wait("PJeOffice's Set up", lambda: find(pje, role="push button", name="Set up")))
        config = home / ".pjeoffice-pro/pjeoffice-pro.config"
        want = f"list.a3={expected_pjeoffice_library()}\n"
        wait(f"{want.strip()} in PJeOffice's settings", lambda: want in config.read_text())
        text = config.read_text()
        if not (text.startswith("#Salvo em 2026-11-01\n") and "list.a1=\n" in text):
            fail(f"PJeOffice's other settings changed: {text!r}")
        wait(
            "the PJeOffice Pro row to turn Ready",
            lambda: subtitle(
                find(app, role="list item", name="PJeOffice Pro", sub="Native app")
            ).endswith("Ready"),
        )
        ok(f"native PJeOffice Pro: {want.strip()}")

        # --- Flatpak tab ---------------------------------------------------
        press(wait("the Flatpak tab", lambda: find(app, role="page tab", name="Flatpak")))
        flat = wait(
            "the Firefox Flatpak row",
            lambda: find(app, role="list item", name="Firefox", sub="org.mozilla.firefox"),
        )
        # Expand the row (AT-SPI offers no action for its header).
        activate_by_keyboard(flat)
        outer = flat.parent
        while outer is not None and not (outer.getRoleName() == "list item" and outer.name == ""):
            outer = outer.parent
        step2 = wait(
            "the Firefox Flatpak's step 2 button",
            lambda: find(outer, role="push button", name="Set up"),
        )
        press(step2)
        fprofile = home / ".var/app/org.mozilla.firefox/.mozilla/firefox/flat.default-release/pkcs11.txt"
        wait(
            "p11-kit-client.so in the Firefox Flatpak's profile",
            lambda: fprofile.exists() and f"library={CLIENT}" in fprofile.read_text(),
        )
        ok("Firefox Flatpak: p11-kit-client.so registered (step 2)")
        print("all GUI checks passed", flush=True)
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
        log.close()
        if os.environ.get("TESSERA_GUI_KEEP"):
            print(f"kept {tmp}")
        else:
            shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()
