#!/usr/bin/env python3
"""Slice each source sheet into its best side-profile animation row and emit one
normalized animated GIF per animal into animals/.  Picks the row with the widest
average frame (the side/locomotion view) so quadrupeds read well.
"""
from __future__ import annotations
import os
from pathlib import Path
from PIL import Image
from slice_sheet import bands

Image.MAX_IMAGE_PIXELS = None
HERE = Path(__file__).resolve().parent
OUT = HERE / "animals"
OUT.mkdir(exist_ok=True)

# (glob, label, animation-row-preference)  prefer_row: None=auto widest
SHEETS = [
    ("dl/wild/Wild Animals/Fox/Fox_Walk.png",    "fox",    None),
    ("dl/wild/Wild Animals/Wolf/Wolf_Walk.png",  "wolf",   None),
    ("dl/wild/Wild Animals/Bear/Bear_Walk.png",  "bear",   None),
    ("dl/wild/Wild Animals/Deer/Deer_Walk.png",  "deer",   None),
    ("dl/wild/Wild Animals/Boar/Boar_Walk.png",  "boar",   None),
    ("dl/wild/Wild Animals/Rabbit/Rabbit_Hop.png","rabbit", None),
    ("dl/farm/cow_walk.png",      "cow",     None),
    ("dl/farm/pig_walk.png",      "pig",     None),
    ("dl/farm/sheep_walk.png",    "sheep",   None),
    ("dl/farm/llama_walk_0.png",  "llama",   None),
    ("dl/farm/chicken_walk.png",  "chicken", None),
    ("dl/farm/bird_1_brown.png",  "bird",    None),
    ("dl/farm/bird_2_eagle.png",  "eagle",   None),
    ("dl/farm/bird_3_sparrow.png","sparrow", None),
    ("dl/farm/bird_3_robin.png",  "robin",   None),
]


def slice_rows(path, maxwidth=1600):
    im = Image.open(path).convert("RGBA")
    if im.width > maxwidth:
        s = maxwidth / im.width
        im = im.resize((maxwidth, round(im.height * s)), Image.LANCZOS)
    apx = im.split()[-1].load()
    W, H = im.size
    rproj = [sum(1 for x in range(W) if apx[x, y] > 24) for y in range(H)]
    rows = bands(rproj, gap=2, min_run=3)
    result = []
    for (y0, y1) in rows:
        cproj = [sum(1 for y in range(y0, y1) if apx[x, y] > 24) for x in range(W)]
        cols = bands(cproj, gap=2, min_run=2)
        if len(cols) >= 2:
            avg_w = sum(b - a for a, b in cols) / len(cols)
            result.append((y0, y1, cols, avg_w))
    return im, result


def build(path, label, prefer):
    im, rows = slice_rows(path)
    if not rows:
        print(f"  ! {label}: no rows"); return None
    if prefer is not None and prefer < len(rows):
        y0, y1, cols, _ = rows[prefer]
    else:
        y0, y1, cols, _ = max(rows, key=lambda r: r[3])   # widest = side view
    frames = [im.crop((x0, y0, x1, y1)) for (x0, x1) in cols]
    cw = max(f.width for f in frames); ch = max(f.height for f in frames)
    canv = []
    for f in frames:
        bg = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
        bg.alpha_composite(f, ((cw - f.width) // 2, ch - f.height))   # bottom-align
        canv.append(bg)
    # scale up small sprites so ASCII has detail to work with
    scale = max(1, 220 // cw)
    if scale > 1:
        canv = [c.resize((cw * scale, ch * scale), Image.NEAREST) for c in canv]
    pal = [c.convert("P", palette=Image.ADAPTIVE, colors=255) for c in canv]
    gif = OUT / f"{label}.gif"
    pal[0].save(gif, save_all=True, append_images=pal[1:], duration=120,
                loop=0, disposal=2, transparency=0, optimize=False)
    print(f"  + {label:9s} {len(frames)} frames  cell {cw}x{ch} x{scale}")
    return gif


if __name__ == "__main__":
    os.chdir(HERE)
    n = 0
    for g, label, prefer in SHEETS:
        if not Path(g).exists():
            print(f"  ! missing {g}"); continue
        if build(g, label, prefer):
            n += 1
    print(f"{n} animal GIFs -> {OUT}")
