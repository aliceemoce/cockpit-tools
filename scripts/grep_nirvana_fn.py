#!/usr/bin/env python3
import re
from pathlib import Path
t = Path(r"C:\Users\aliceemoce\dev\cockpit-tools\scripts\nirvana_extracted\dist-electron\main-FXxcqbQA.js").read_text(encoding="utf-8", errors="replace")
for name in ["silentSwitch", "switchTokensInDb", "function Zh", "function Kh", "function Lc", "function go", "cleanCursorEnvironment"]:
    m = re.search(rf"{re.escape(name)}[^{{]*\{{", t)
    if m:
        print("===", name, "===")
        print(t[m.start():m.start()+1200].replace("\n", " ")[:1200])
        print()
