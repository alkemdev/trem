#!/usr/bin/env python3
"""
Download John Sankey’s Well-Tempered Clavier MIDI (mirrored on jsbach.net).

Index page:
  http://www.jsbach.net/midi/midi_johnsankey.html

Archives:
  - Book I (BWV 846–869): one .mid per prelude+fugue pair (24 files).
  - Book II (BWV 870–881): **only the first 12 pairs** on this mirror (12 files).

John Sankey’s site (https://johnsankey.ca/) states that anyone may copy and
distribute his materials as long as the permission notice is distributed with
copies. This repo ships `PERMISSION-JOHN-SANKEY.txt` next to the extracted files.

Usage:
  python3 scripts/fetch_sankey_wtc_midi.py
  python3 scripts/fetch_sankey_wtc_midi.py --dry-run
"""

from __future__ import annotations

import argparse
import io
import sys
import urllib.request
import zipfile
from pathlib import Path

INDEX = "http://www.jsbach.net/midi/midi_johnsankey.html"
ZIPS = (
    (
        "846-869.zip",
        "http://www.jsbach.net/midi/sankey/846-869.zip",
        "WTC Book I (24 prelude+fugue pairs)",
    ),
    (
        "870-881.zip",
        "http://www.jsbach.net/midi/sankey/870-881.zip",
        "WTC Book II, pairs 1–12 only",
    ),
)

PERMISSION_TEXT = """John Sankey — redistribution notice (abridged; full site: https://johnsankey.ca/)

The author’s site states that anyone may copy, link to, or distribute anything
on his site, provided this notice of permission to further copy is distributed
with all copies, and that the material remains free for all (no collection
copyright or use restrictions). Credit is appreciated.

MIDI source index (Dave Lampson / jsbach.net mirror):
  http://www.jsbach.net/midi/midi_johnsankey.html

These files were extracted from:
  - 846-869.zip (Book I)
  - 870-881.zip (Book II, partial)

(Wikipedia is explicitly excluded from this permission on the author’s site;
that exception does not affect normal redistribution elsewhere.)
"""


def download(url: str, timeout: float = 60.0) -> bytes:
    req = urllib.request.Request(
        url,
        headers={
            "User-Agent": "trem-assets-fetch/1.0 (John Sankey WTC MIDI; open source)"
        },
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return resp.read()


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Fetch Sankey WTC zips into assets/midi/wtc/sankey/"
    )
    ap.add_argument(
        "--out-dir",
        type=Path,
        default=Path("assets/midi/wtc/sankey"),
        help="output directory (default: assets/midi/wtc/sankey)",
    )
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    out: Path = args.out_dir
    if not args.dry_run:
        out.mkdir(parents=True, exist_ok=True)
        (out / "PERMISSION-JOHN-SANKEY.txt").write_text(
            PERMISSION_TEXT, encoding="utf-8"
        )
        print(f"wrote {out / 'PERMISSION-JOHN-SANKEY.txt'}")

    for arcname, url, desc in ZIPS:
        print(f"# {desc}")
        print(f"  {url}")
        if args.dry_run:
            continue
        data = download(url)
        with zipfile.ZipFile(io.BytesIO(data)) as zf:
            for info in zf.infolist():
                if info.is_dir():
                    continue
                name = Path(info.filename).name
                if not name.lower().endswith(".mid"):
                    continue
                dest = out / name
                dest.write_bytes(zf.read(info.filename))
                print(f"  wrote {dest} ({info.file_size} bytes)")

    if args.dry_run:
        print("(dry run — no files written)")
        print(f"would extract *.mid from {len(ZIPS)} zip(s) -> {out}")
        print(f"index: {INDEX}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
