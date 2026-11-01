#!/usr/bin/env python3
"""Print the vendor drivers pinned in the Flatpak manifest, one per line:
"filename url sha256". The AppImage downloads these on its first start."""
import re
import sys
from pathlib import Path

manifest = Path(sys.argv[1]).read_text()
block = re.search(r"# BEGIN DRIVERS\n(.*?)# END DRIVERS", manifest, re.S).group(1)
entry = re.compile(r"filename: (\S+).*?url: (\S+)\s+sha256: ([0-9a-f]{64})", re.S)
found = entry.findall(block)
if not found:
    sys.exit("no extra-data entries found")
for name, url, sha in found:
    print(name, url, sha)
