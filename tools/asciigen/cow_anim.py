#!/usr/bin/env python3
"""Procedural ASCII animation over a generated cow still.

Renders the gen'd cow cutout as a soft-edged solid SILHOUETTE (clean, unlike the
noisy luminance version), auto-finds the eye / mouth / tail anchors, then overlays
motion: eye blink, a grass sprig at the mouth being nibbled shorter, a flicking
tail, and a shimmering grass ground line. Portable plain ASCII (Tier 1).

  python cow_anim.py                 # build frames + picker + preview
  python cow_anim.py --ruler         # just print the silhouette with a ruler
"""
from __future__ import annotations
import argparse, os
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent
RAMP = [(0.85, "#"), (0.55, "*"), (0.30, "+"), (0.16, ":")]
LEFT, RIGHT, TOP, BOT = 7, 5, 1, 2   # padding (room for sprig / tail / ground)


def silhouette(png_path, cols=72):
    im = Image.open(png_path).convert("RGBA")
    a = im.split()[-1]
    bbox = a.getbbox()
    if bbox:
        a = a.crop(bbox)
    W, H = a.size
    rows = max(1, round(cols * (H / W) * 0.5))
    px = a.resize((cols, rows), Image.LANCZOS).load()
    grid = []
    for y in range(rows):
        line = []
        for x in range(cols):
            v = px[x, y] / 255.0
            ch = " "
            for thr, g in RAMP:
                if v >= thr:
                    ch = g
                    break
            line.append(ch)
        grid.append(line)
    return grid


def filled(grid, r, c):
    return 0 <= r < len(grid) and 0 <= c < len(grid[0]) and grid[r][c] != " "


def anchors(grid):
    h, w = len(grid), len(grid[0])
    # mouth/nose = leftmost filled cell in the lower-head band (left third)
    band = range(int(h * 0.30), int(h * 0.55))
    mouth = None
    for c in range(w):
        for r in band:
            if grid[r][c] != " ":
                mouth = (r, c); break
        if mouth:
            break
    # eye = up and to the right of the mouth, inside the head mass
    eye = (mouth[0] - 4, mouth[1] + 7) if mouth else (5, 8)
    # tail = rightmost filled cell in the mid band
    midband = range(int(h * 0.25), int(h * 0.55))
    tail = None
    for c in range(w - 1, -1, -1):
        for r in midband:
            if grid[r][c] != " ":
                tail = (r, c); break
        if tail:
            break
    # ground = the lowest filled row (feet)
    ground = max(r for r in range(h) for c in range(w) if grid[r][c] != " ")
    return mouth, eye, tail, ground


def pad(grid):
    w = len(grid[0])
    out = [[" "] * (LEFT + w + RIGHT) for _ in range(TOP)]
    for row in grid:
        out.append([" "] * LEFT + list(row) + [" "] * RIGHT)
    for _ in range(BOT):
        out.append([" "] * (LEFT + w + RIGHT))
    return out


def stamp(grid, r, c, s):
    for i, ch in enumerate(s):
        cc = c + i
        if ch != " " and 0 <= r < len(grid) and 0 <= cc < len(grid[0]):
            grid[r][cc] = ch


def build_frames(src, cols=72):
    base = silhouette(src, cols)
    mouth, eye, tail, ground = anchors(base)
    GROUND = "v.,v\\,.v,.\\v.,.v,\\.v,.v\\,.v,.v\\,.v,.\\v,.v,\\.v,.v.,\\v.,v,.v\\,.v,.v,\\.v,.v\\,"

    # per-frame: (sprig string drawn LEFT of mouth, eye glyph, tail glyph, shimmer)
    cycle = [
        (r"\|/~", " ", "\\", 0),   # full sprig, eye open, tail down
        (r" |/~", " ", "|", 1),    # nibble, tail mid
        (r"  /~", "-", "/", 2),    # shorter, BLINK, tail flick up
        (r"   ~", " ", "|", 0),    # almost gone, tail mid
        (r"\|/~", " ", "\\", 1),   # regrown
        (r" \|~", " ", "/", 2),    # chew
    ]
    frames = []
    for sprig, eyeg, tailg, sh in cycle:
        g = pad([row[:] for row in base])
        # padded coords
        er, ec = eye[0] + TOP, eye[1] + LEFT
        mr, mc = mouth[0] + TOP, mouth[1] + LEFT
        tr, tc = tail[0] + TOP, tail[1] + LEFT
        gr = ground + TOP
        # eye: carve a hole (open) or fill (blink)
        if eyeg == " ":
            g[er][ec] = " "
        else:
            g[er][ec] = eyeg
        # grass sprig poking left out of the mouth
        stamp(g, mr, mc - len(sprig), sprig)
        # flicking tail off the rear
        toff = {"\\": (0, 1), "|": (0, 1), "/": (-1, 1)}[tailg]
        stamp(g, tr + toff[0], tc + toff[1], tailg)
        # ground grass with a little shimmer
        gl = list(GROUND)
        for i in range(sh, len(gl), 4):
            if gl[i] == ".":
                gl[i] = "v"
        stamp(g, gr + 1, 1, "".join(gl)[: len(g[0]) - 2])
        frames.append("\n".join("".join(r).rstrip() for r in g))
    return frames


def main():
    os.chdir(HERE)
    ap = argparse.ArgumentParser()
    ap.add_argument("--src", default="gencow2_src/cowA.png")
    ap.add_argument("--cols", type=int, default=72)
    ap.add_argument("--ruler", action="store_true")
    a = ap.parse_args()
    if a.ruler:
        g = silhouette(a.src, a.cols)
        w = len(g[0])
        print("    " + "".join(str((i // 10) % 10) for i in range(w)))
        print("    " + "".join(str(i % 10) for i in range(w)))
        for r, line in enumerate(g):
            print(f"{r:3d} {''.join(line).rstrip()}")
        return
    frames = build_frames(a.src, a.cols)
    # normalize width/height
    W = max(len(l) for f in frames for l in f.splitlines())
    H = max(len(f.splitlines()) for f in frames)
    norm = []
    for f in frames:
        ls = [l.ljust(W) for l in f.splitlines()]
        while len(ls) < H:
            ls.append(" " * W)
        norm.append("\n".join(ls))
    d = Path("cow_out/frames/cow-graze")
    d.mkdir(parents=True, exist_ok=True)
    for i, f in enumerate(norm):
        (d / f"frame_{i:03d}.txt").write_text(f + "\n", encoding="utf-8")
    from gen import write_picker
    write_picker(Path("cow_out/cow-graze.html"),
                 [("cow-graze", "Gen cow grazing (silhouette + procedural)", 260, norm)])
    print(f"{len(norm)} frames -> {d}\n")
    print(norm[0])


if __name__ == "__main__":
    main()
