#!/usr/bin/env python3
"""Download each vendor driver listed in the Flatpak manifest, check that its
sha256 matches the pinned one, and write its size into the manifest.

A mismatch means the vendor changed the file at the same URL: nothing is
written, and the script fails, so the new file can be reviewed before the pin
is updated by hand.

Usage: scripts/pin-drivers.py [--check]
  --check   only verify; fail if a size in the manifest is wrong
"""
import hashlib
import re
import sys
import urllib.request
from pathlib import Path

MANIFEST = Path(__file__).resolve().parent.parent / "flatpak" / "io.github.pedrohqb.Tessera.yml"
BLOCK = re.compile(r"(# BEGIN DRIVERS\n)(.*?)(\s*# END DRIVERS)", re.S)
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


def main():
    check = "--check" in sys.argv
    text = MANIFEST.read_text()
    block = BLOCK.search(text)
    if not block:
        sys.exit("DRIVERS block not found in the manifest")
    body, failed = block.group(2), False

    def update(m):
        nonlocal failed
        sha, size = fetch(m["url"])
        if sha != m["sha"]:
            print(f"{m['name']}: SHA256 MISMATCH\n  pinned {m['sha']}\n  got    {sha}", file=sys.stderr)
            failed = True
            return m.group(0)
        status = "ok" if int(m["size"]) == size else f"size {m['size']} -> {size}"
        print(f"{m['name']}: sha256 ok, {size} bytes ({status})")
        if check and int(m["size"]) != size:
            failed = True
        return m.group(1) + str(size)

    new_body = ENTRY.sub(update, body)
    if failed:
        sys.exit(1)
    if not check:
        MANIFEST.write_text(text[: block.start(2)] + new_body + text[block.end(2):])


if __name__ == "__main__":
    main()
