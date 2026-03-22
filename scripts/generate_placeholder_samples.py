#!/usr/bin/env python3
"""
Write tiny PCM WAV placeholders under assets/samples/ (no dependencies).

These are original generated waveforms (not recordings) — trivially usable as
smoke-test audio if the project grows file-based sample playback.
"""

from __future__ import annotations

import argparse
import math
import struct
import wave
from pathlib import Path


def write_sine_wav(
    path: Path, *, freq_hz: float, seconds: float, sample_rate: int, amplitude: float
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    n = int(sample_rate * seconds)
    with wave.open(str(path), "w") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(sample_rate)
        frames = bytearray()
        for i in range(n):
            s = amplitude * math.sin(2.0 * math.pi * freq_hz * (i / sample_rate))
            frames.extend(struct.pack("<h", int(max(-1.0, min(1.0, s)) * 32767)))
        w.writeframes(frames)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out-dir", type=Path, default=Path("assets/samples"))
    args = ap.parse_args()
    write_sine_wav(
        args.out_dir / "sine_440_250ms.wav",
        freq_hz=440.0,
        seconds=0.25,
        sample_rate=22050,
        amplitude=0.2,
    )
    print(f"wrote {args.out_dir / 'sine_440_250ms.wav'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
