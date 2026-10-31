#!/usr/bin/env python3
"""Chill cow: standing still, with an EYE, munching a mouthful of grass.

Clean solid silhouette of the gen'd cow (no head motion). Overlays: a visible eye
that blinks, a grass sprig in the mouth that gets chewed shorter then refreshes, a
shimmering grass ground, and a small tail flick. Portable plain ASCII.
"""
from __future__ import annotations
import argparse, os
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent
GROUND = ("v.,v\\,.v,.\\v.,.v,\\.v,.v\\,.v,.v\\,.v,.\\v,.v,\\.v,.v.,\\v.,v,.v"
          "\\,.v,.v,\\.v,.v\\,.v,.v\\")


def silhouette(png, cols=70):
    a = Image.open(png).convert("RGBA").split()[-1]
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
            line.append("#" if v >= 0.55 else ("+" if v >= 0.30 else " "))
        grid.append(line)
    return grid


def clean_strays(grid):
    """Drop thin 1-row horizontal slivers (the gen cow's stray back-line)."""
    h, w = len(grid), len(grid[0])
    for r in range(1, h - 1):
        for c in range(w):
            if grid[r][c] != " " and grid[r - 1][c] == " " and grid[r + 1][c] == " ":
                # isolated vertically; keep only if part of a tall run nearby
                grid[r][c] = " "
    return grid


def anchors(grid):
    h, w = len(grid), len(grid[0])
    band = range(int(h * 0.28), int(h * 0.55))
    mouth = None
    for c in range(w):
        for r in band:
            if grid[r][c] != " ":
                mouth = (r, c); break
        if mouth:
            break
    eye = (mouth[0] - 4, mouth[1] + 9) if mouth else (4, 10)
    tail = None
    for c in range(w - 1, -1, -1):
        for r in range(int(h * 0.25), int(h * 0.55)):
            if grid[r][c] != " ":
                tail = (r, c); break
        if tail:
            break
    ground = max(r for r in range(h) for c in range(w) if grid[r][c] != " ")
    return mouth, eye, tail, ground


def put(grid, r, c, ch):
    if 0 <= r < len(grid) and 0 <= c < len(grid[0]):
        grid[r][c] = ch


def carve_eye(grid, er, ec, glyph):
    # clear a little socket so the eye reads, then place the glyph
    for dr in (-1, 0, 1):
        for dc in (-1, 0, 1, 2):
            put(grid, er + dr, ec + dc, " ")
    put(grid, er, ec, glyph)
    put(grid, er, ec + 1, glyph if glyph == "-" else " ")


def draw_right(grid, r, col_end, s):
    """Draw string `s` ending at (r, col_end), going left. Spaces transparent."""
    start = col_end - len(s) + 1
    for i, ch in enumerate(s):
        if ch != " ":
            put(grid, r, start + i, ch)


# A bushy mouthful of grass sticking out of the mouth, animated: it waves, gets
# chewed shorter, then a fresh mouthful (lots of movement). Two rows tall.
CLUMP = [
    (r"\\|//,", r" \|/, "),   # full, leaning right
    (r",\\|//", r" ,\|/ "),   # waves left
    (r" \\|/",  r"  \|, "),   # chewed a bit
    (r"  \|,",  r"   \| "),   # shorter
    (r"   \,",  r"    | "),   # almost gone
    (r"\\|/,",  r" ,\|/ "),   # fresh mouthful
    (r"/,\\|/", r" \|/, "),   # waves right
    (r" \\|//", r"  \|, "),   # chew
]
TAILS = ["\\", "|", "/", "|", "\\", "|", "/", "|"]
# jaw: vertical nudge of the lower lip to read as chewing
JAW = [0, 1, 0, 1, 1, 0, 1, 0]


def build(src, cols=70):
    base = clean_strays(silhouette(src, cols))
    mouth, _eye, tail, ground = anchors(base)
    PADL, PADR, PADB = 9, 4, 2
    w0 = len(base[0])
    frames = []
    for fi in range(len(CLUMP)):
        g = [[" "] * PADL + row[:] + [" "] * PADR for row in base]
        g += [[" "] * (PADL + w0 + PADR) for _ in range(PADB)]
        mr, mc = mouth[0], mouth[1] + PADL
        tr, tc = tail[0], tail[1] + PADL
        gr = ground + 1

        # bushy grass out of the mouth (two rows), ending just left of the muzzle
        top, bot = CLUMP[fi]
        draw_right(g, mr - 1, mc - 1, top)
        draw_right(g, mr, mc - 1, bot)
        # jaw chew: nudge the lower lip down a touch on alternate frames
        if JAW[fi]:
            put(g, mr + 1, mc - 1, ",")

        # flicking tail
        tg = TAILS[fi]
        toff = {"\\": (0, 1), "|": (0, 1), "/": (-1, 1)}[tg]
        put(g, tr + toff[0], tc + toff[1], tg)

        # lush, waving grass ground (taller tufts that shimmer)
        base_g = list(GROUND)
        for i in range(fi % 4, len(base_g), 3):
            if base_g[i] in ".,":
                base_g[i] = "v"
        gline = " " + "".join(base_g)
        for i, ch in enumerate(gline[: len(g[0])]):
            if ch != " ":
                put(g, gr, i, ch)
        # a few blades standing up above the ground line, swaying
        for i in range((fi * 2) % 5, len(g[0]) - 1, 6):
            put(g, gr - 1, i, "\\|/"[(i + fi) % 3])
        frames.append("\n".join("".join(r).rstrip() for r in g))
    return frames


def main():
    os.chdir(HERE)
    ap = argparse.ArgumentParser()
    ap.add_argument("--src", default="gencow2_src/cowA.png")
    ap.add_argument("--cols", type=int, default=70)
    ap.add_argument("--ruler", action="store_true")
    a = ap.parse_args()
    if a.ruler:
        g = clean_strays(silhouette(a.src, a.cols))
        m, e, t, gr = anchors(g)
        print("mouth", m, "eye", e, "tail", t, "ground", gr)
        w = len(g[0])
        print("    " + "".join(str(i % 10) for i in range(w)))
        for r, line in enumerate(g):
            print(f"{r:3d} {''.join(line).rstrip()}")
        return
    frames = build(a.src, a.cols)
    W = max(len(l) for f in frames for l in f.splitlines())
    H = max(len(f.splitlines()) for f in frames)
    norm = []
    for f in frames:
        ls = [l.ljust(W) for l in f.splitlines()]
        while len(ls) < H:
            ls.append(" " * W)
        norm.append("\n".join(ls))
    d = Path("cow_out/frames/cow-munch")
    d.mkdir(parents=True, exist_ok=True)
    for i, f in enumerate(norm):
        (d / f"frame_{i:03d}.txt").write_text(f + "\n", encoding="utf-8")
    from gen import write_picker
    write_picker(Path("cow_out/cow-munch.html"),
                 [("cow-munch", "Cow munching grass (eyes + chew)", 260, norm)])
    print(f"{len(norm)} frames -> {d}")


if __name__ == "__main__":
    main()
