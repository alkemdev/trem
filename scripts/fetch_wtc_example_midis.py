#!/usr/bin/env python3
"""
Semi-automated refresh of bundled Well-Tempered Clavier example MIDIs.

Runs, in order:
  1. scripts/fetch_sankey_wtc_midi.py  — complete Book I + partial Book II (jsbach.net mirror)
  2. scripts/fetch_mutopia_wtc_midi.py — extra individual movements from Mutopia (partial)

From the repository root:

  python3 scripts/fetch_wtc_example_midis.py
  python3 scripts/fetch_wtc_example_midis.py --dry-run

For per-source flags (e.g. custom --out-dir), run the individual scripts instead.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STEPS = (
    ROOT / "scripts" / "fetch_sankey_wtc_midi.py",
    ROOT / "scripts" / "fetch_mutopia_wtc_midi.py",
)


def main() -> int:
    ap = argparse.ArgumentParser(description="Fetch Sankey + Mutopia WTC example MIDIs")
    ap.add_argument(
        "--dry-run", action="store_true", help="pass --dry-run to both fetch scripts"
    )
    args = ap.parse_args()
    extra = ["--dry-run"] if args.dry_run else []

    for script in STEPS:
        print(f"\n==> {script.name}\n")
        r = subprocess.run([sys.executable, str(script), *extra], cwd=str(ROOT))
        if r.returncode != 0:
            return r.returncode
    print("\nDone. See assets/midi/wtc/README.md for layout and licensing.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
