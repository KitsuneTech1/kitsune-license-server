#!/usr/bin/env python3
"""Hand-crafted animated ASCII cow eating grass, with horns.

Coherent side-view cow facing LEFT, standing, with a sprig of grass in its mouth
and big curved horns. Animation loop: chews (jaw + grass sprig wiggle), blinks,
tail swishes, ground grass shimmers. Portable plain ASCII (Tier 1).
"""
from __future__ import annotations
import os
from pathlib import Path

HERE = Path(__file__).resolve().parent

# Base art, facing left. {e}=eye, {m}=grass sprig in mouth, {t}=tail tuft.
# Horns are the converging \/ pair at top. Body to the right, four legs, ground.
BASE = r"""
            ^         ^
            |\       /|
            | \     / |
             \ \   / /         _______________
              \ \ / /         /               \
               \ V /         /                 \____
          ______\_/_________/                       \
         /       {e}                 @         @      |
  {m}|        \                              @       |{t}
         \      \____                               _|
          \         |______________________________|
                     |    |              |    |
                     |_   |_             _|   _|
   .,.,.,.,.,.,.,.,.,.,.,.,.,.,.,.,.,.,.,.,.,.,.,.,.,.,.,
"""


def variant(eye, mouth, tail, ground_shift):
    art = BASE.format(e=eye, m=mouth, t=tail)
    lines = art.strip("\n").split("\n")
    # shimmer the ground line (last line): raise a few blades
    gl = list(lines[-1])
    for i in range(ground_shift % 3, len(gl), 5):
        if gl[i] == ".":
            gl[i] = "v"
        elif gl[i] == ",":
            gl[i] = "\\"
    lines[-1] = "".join(gl)
    return "\n".join(lines)


FRAMES = [
    variant("O", r"\\|//~", "(", 0),   # eye open, full sprig, tail left
    variant("O", r" \|/~", " )", 1),   # chew: sprig shorter, tail right
    variant("-", r"  |/~", "(", 2),    # blink
    variant("O", r"   /~", " )", 0),   # sprig nearly eaten, tail right
]


def normalize(frames):
    w = max(len(l) for f in frames for l in f.splitlines())
    h = max(len(f.splitlines()) for f in frames)
    out = []
    for f in frames:
        lines = [l.ljust(w) for l in f.splitlines()]
        while len(lines) < h:
            lines.append(" " * w)
        out.append("\n".join(lines))
    return out


def main():
    os.chdir(HERE)
    frames = normalize(FRAMES)
    d = Path("cow_out/frames/cow-eating")
    d.mkdir(parents=True, exist_ok=True)
    for i, f in enumerate(frames):
        (d / f"frame_{i:03d}.txt").write_text(f + "\n", encoding="utf-8")
    Path("cow_out/cow-eating.txt").write_text("\f".join(frames) + "\n", encoding="utf-8")
    print(f"{len(frames)} frames -> {d}\n")
    print(frames[0])


if __name__ == "__main__":
    main()
