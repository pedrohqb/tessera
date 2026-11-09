#!/usr/bin/env python3
"""Download each vendor file pinned in the extensions' manifests (drivers and
web signers: flatpak/extensions/*/io.github.pedrohqb.Tessera.*.yml, between
BEGIN and END MANAGED EXTRA-DATA), check that its sha256 matches the pinned
one, and write its size into the manifest.

A mismatch means the vendor changed the file at the same URL: nothing is
written, and the script fails, so the new file can be reviewed before the pin
is updated by hand.

Usage: scripts/pin-drivers.py [--check]
  --check   only verify; fail if a size in a manifest is wrong
"""
import hashlib
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFESTS = sorted((ROOT / "flatpak" / "extensions").glob("*/io.github.pedrohqb.Tessera.*.yml"))
BLOCK = re.compile(r"(# BEGIN MANAGED EXTRA-DATA\n)(.*?)(\s*# END MANAGED EXTRA-DATA)", re.S)
ENTRY = re.compile(
    r"(filename: (?P<name>\S+).*?url: (?P<url>\S+)\s+sha256: (?P<sha>[0-9a-f]{64})\s+size: )(?P<size>\d+)",
    re.S,
)


def fetch(url):
    req = urllib.request.Request(url, headers={"User-Agent": "tessera-pin/1"})
    h, size = hashlib.sha256(), 0
    with urllib.request.urlopen(req, timeout=300) as r:
        while chunk := r.read(1 << 20):
            h.update(chunk)
            size += len(chunk)
    return h.hexdigest(), size


def pin(manifest, check):
    text = manifest.read_text()
    block = BLOCK.search(text)
    if not block:
        sys.exit(f"{manifest.name}: MANAGED EXTRA-DATA block not found")
    body, failed = block.group(2), False

    def update(m):
        nonlocal failed
        sha, size = fetch(m["url"])
        if sha != m["sha"]:
            print(f"{m['name']}: SHA256 MISMATCH\n  pinned {m['sha']}\n  got    {sha}", file=sys.stderr)
            failed = True
            return m.group(0)
        status = "ok" if int(m["size"]) == size else f"size {m['size']} -> {size}"
        print(f"{manifest.parent.name}/{m['name']}: sha256 ok, {size} bytes ({status})")
        if check and int(m["size"]) != size:
            failed = True
        return m.group(1) + str(size)

    new_body = ENTRY.sub(update, body)
    if not failed and not check:
        manifest.write_text(text[: block.start(2)] + new_body + text[block.end(2):])
    return failed


def main():
    check = "--check" in sys.argv
    if not MANIFESTS:
        sys.exit("no extension manifests found")
    failed = [m.name for m in MANIFESTS if pin(m, check)]
    if failed:
        sys.exit(f"failed: {', '.join(failed)}")


if __name__ == "__main__":
    main()
