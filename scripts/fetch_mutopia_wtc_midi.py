#!/usr/bin/env python3
"""
Download Well-Tempered Clavier–range MIDI files from the Mutopia Project.

Mutopia hosts Bach under https://www.mutopiaproject.org/ftp/BachJS/ .  This script
walks BWV846–BWV893 (Book I + II) and recursively discovers *.mid links.

Note: Mutopia does *not* host a complete MIDI set for the WTC; you typically get
on the order of a few dozen files, not all 96 movements.  See assets/midi/wtc/README.md.

License: Bach’s music is public domain; Mutopia files are marked Public Domain on
the project site.  Redistribution is intended; still, cite Mutopia as the source.

Usage:
  python3 scripts/fetch_mutopia_wtc_midi.py
  python3 scripts/fetch_mutopia_wtc_midi.py --dry-run
"""

from __future__ import annotations

import argparse
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from collections import deque
from pathlib import Path

BASE = "https://www.mutopiaproject.org/ftp/BachJS/"
BWV_FIRST = 846
BWV_LAST = 893

# Drop obvious non–WTC piano MIDIs that sometimes appear under the same BWV folder.
SKIP_URL_SUBSTR = (
    "guitar-duo",
    "spiritus_domini",
)


def fetch(url: str, timeout: float = 45.0) -> str:
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "trem-assets-fetch/1.0 (Mutopia WTC MIDI; open source)"},
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return resp.read().decode("utf-8", "replace")


def dir_hrefs(html: str, parent: str) -> list[str]:
    out: list[str] = []
    for m in re.finditer(r'href="([^"]+)"', html):
        href = m.group(1)
        if href in ("../",) or href.startswith("?"):
            continue
        if not href.endswith("/"):
            continue
        out.append(urllib.parse.urljoin(parent, href))
    return out


def mid_hrefs(html: str, parent: str) -> list[str]:
    out: list[str] = []
    for m in re.finditer(r'href="([^"]+\.mid)"', html, flags=re.I):
        href = m.group(1)
        out.append(urllib.parse.urljoin(parent, href))
    return out


def discover_mid_urls() -> list[str]:
    found: set[str] = set()
    for n in range(BWV_FIRST, BWV_LAST + 1):
        root = f"{BASE}BWV{n}/"
        try:
            html = fetch(root)
        except urllib.error.HTTPError as e:
            if e.code == 404:
                continue
            raise
        except urllib.error.URLError:
            continue

        q: deque[str] = deque([root])
        seen_dirs: set[str] = {root}

        while q:
            durl = q.popleft()
            try:
                h = fetch(durl)
            except Exception:
                continue
            for m in mid_hrefs(h, durl):
                if any(s in m for s in SKIP_URL_SUBSTR):
                    continue
                found.add(m)
            for child in dir_hrefs(h, durl):
                if f"/BWV{n}/" not in child:
                    continue
                if child not in seen_dirs:
                    seen_dirs.add(child)
                    q.append(child)

    return sorted(found)


def stable_name(mid_url: str) -> str:
    parts = urllib.parse.urlparse(mid_url).path.strip("/").split("/")
    # .../BachJS/BWV846/wtk1-prelude1/wtk1-prelude1.mid
    try:
        js = parts.index("BachJS")
        tail = parts[js + 1 :]
    except ValueError:
        tail = parts[-4:]
    if len(tail) < 2:
        return Path(mid_url).name
    bwv = tail[0]
    rest = "__".join(tail[1:])
    return f"{bwv}__{rest}".replace("/", "_")


def download(url: str, dest: Path, dry_run: bool) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dry_run:
        print(f"would fetch {url} -> {dest}")
        return
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "trem-assets-fetch/1.0 (Mutopia WTC MIDI; open source)"},
    )
    with urllib.request.urlopen(req, timeout=60) as resp:
        data = resp.read()
    dest.write_bytes(data)
    print(f"wrote {dest} ({len(data)} bytes)")


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Fetch Mutopia WTC-range MIDI into assets/midi/wtc/mutopia/"
    )
    ap.add_argument(
        "--out-dir",
        type=Path,
        default=Path("assets/midi/wtc/mutopia"),
        help="output directory (default: assets/midi/wtc/mutopia)",
    )
    ap.add_argument(
        "--dry-run", action="store_true", help="print planned downloads only"
    )
    args = ap.parse_args()

    urls = discover_mid_urls()
    if not urls:
        print("no MIDI URLs discovered (network error?)", file=sys.stderr)
        return 1

    print(f"discovered {len(urls)} MIDI file(s)")
    for url in urls:
        name = stable_name(url)
        if not name.lower().endswith(".mid"):
            name += ".mid"
        download(url, args.out_dir / name, args.dry_run)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
