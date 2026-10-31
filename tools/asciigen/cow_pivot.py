#!/usr/bin/env python3
"""Articulate ONE gen cow: rotate its head/neck down around the shoulder pivot in
image-space to fake a head-up -> graze -> head-up motion, then silhouette each
angle into an ASCII frame. Consistent because it's a single cow.

  python cow_pivot.py --preview          # dump rotated composites as PNG to tune
  python cow_pivot.py                     # build ASCII frames + picker + preview
"""
from __future__ import annotations
import argparse, os
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent

# Pivot / head-box as fractions of the cutout (tuned for a left-facing cow whose
# head+horns sit top-left and body extends right).
CUT_X = 0.34        # right edge of the head/neck box
HEAD_BOT = 0.58     # bottom of the head/neck box (above the chest/front legs)
PIVOT = (0.30, 0.34)  # shoulder/neck base — rotation center


def rotate_head(img, angle, cut_x=CUT_X, head_bot=HEAD_BOT, pivot=PIVOT):
    """Return a new RGBA with the head box rotated by `angle` deg (negative dips
    the left-pointing nose downward) about the pivot."""
    W, H = img.size
    cx, hb = int(cut_x * W), int(head_bot * H)
    pv = (pivot[0] * W, pivot[1] * H)

    head_crop = img.crop((0, 0, cx, hb))
    head_layer = Image.new("RGBA", img.size, (0, 0, 0, 0))
    head_layer.paste(head_crop, (0, 0))
    head_layer = head_layer.rotate(angle, resample=Image.BICUBIC, center=pv)

    body = img.copy()
    # erase the original head box from the body so it doesn't double up
    clear = Image.new("RGBA", (cx, hb), (0, 0, 0, 0))
    body.paste(clear, (0, 0))

    out = body
    out.alpha_composite(head_layer)
    return out


# animation: angle per frame (head up -> dip -> eat -> lift -> up -> look)
# positive angle dips the left-pointing nose DOWN toward the grass
SEQ = [0, 16, 34, 50, 50, 34, 16, 0, 0]
EATING = {3, 4}   # frames where the nose is at the grass


def silhouette_grid(img, cols=78):
    a = img.split()[-1]
    bbox = a.getbbox()
    if bbox:
        a = a.crop(bbox)
    W, H = a.size
    rows = max(1, round(cols * (H / W) * 0.5))
    px = a.resize((cols, rows), Image.LANCZOS).load()
    ramp = [(0.85, "#"), (0.55, "*"), (0.30, "+"), (0.16, ":")]
    grid = []
    for y in range(rows):
        line = []
        for x in range(cols):
            v = px[x, y] / 255.0
            ch = " "
            for thr, g in ramp:
                if v >= thr:
                    ch = g; break
            line.append(ch)
        grid.append(line)
    return grid


def main():
    os.chdir(HERE)
    ap = argparse.ArgumentParser()
    ap.add_argument("--src", default="gencow2_src/cowA.png")
    ap.add_argument("--preview", action="store_true")
    ap.add_argument("--cols", type=int, default=78)
    a = ap.parse_args()
    img = Image.open(a.src).convert("RGBA")

    if a.preview:
        comps = [rotate_head(img, ang) for ang in [0, 16, 34, 50]]
        cw = max(c.width for c in comps); ch = max(c.height for c in comps)
        sheet = Image.new("RGBA", (cw * 2 + 30, ch * 2 + 30), (18, 18, 18, 255))
        for k, c in enumerate(comps):
            r, cc = divmod(k, 2)
            sheet.alpha_composite(c, (10 + cc * (cw + 10), 10 + r * (ch + 10)))
        sheet.convert("RGB").save("cow_out/_pivot_preview.png")
        print("wrote cow_out/_pivot_preview.png")
        return

    # build frames on a shared canvas (rotate first, then silhouette each, then
    # pad to common size, bottom-aligned)
    comps = [rotate_head(img, ang) for ang in SEQ]
    grids = [silhouette_grid(c, a.cols) for c in comps]
    W = max(len(g[0]) for g in grids)
    Hh = max(len(g) for g in grids)
    PADL, PADR, PADB = 6, 4, 2
    frames = []
    GROUND = ("v.,v\\,.v,.\\v.,.v,\\.v,.v\\,.v,.v\\,.v,.\\v,.v,\\.v,.v.,"
              "\\v.,v,.v\\,.v,.v,\\.v,.v\\,.v,.v\\")
    for fi, g in enumerate(grids):
        # pad each grid to (Hh, W), bottom-align + center
        gh, gw = len(g), len(g[0])
        top = Hh - gh
        left = (W - gw) // 2
        canvas = [[" "] * (PADL + W + PADR) for _ in range(top)]
        for row in g:
            canvas.append([" "] * (PADL + left) + row + [" "] * (PADR + (W - gw - left)))
        for _ in range(PADB):
            canvas.append([" "] * (PADL + W + PADR))
        # ground grass shimmer
        gl = list(GROUND)
        for i in range(fi % 3, len(gl), 4):
            if gl[i] == ".":
                gl[i] = "v"
        gline = " " + "".join(gl)
        gr = len(canvas) - 1
        for i, c in enumerate(gline[:len(canvas[0])]):
            if c != " ":
                canvas[gr][i] = c
        # grass tuft at the nose when eating (nose ~ bottom-left of the silhouette)
        if fi in EATING:
            # find lowest-leftmost filled cell = the lowered nose
            nose = None
            for rr in range(len(canvas) - PADB - 1, -1, -1):
                for cc in range(len(canvas[0])):
                    if canvas[rr][cc] != " ":
                        nose = (rr, cc); break
                if nose:
                    break
            if nose:
                for k, ch in enumerate("\\|/"):
                    cc = nose[1] - 3 + k
                    if 0 <= cc < len(canvas[0]):
                        canvas[nose[0]][cc] = ch
        frames.append("\n".join("".join(r).rstrip() for r in canvas))

    d = Path("cow_out/frames/cow-pivot")
    d.mkdir(parents=True, exist_ok=True)
    for i, f in enumerate(frames):
        (d / f"frame_{i:03d}.txt").write_text(f + "\n", encoding="utf-8")
    from gen import write_picker
    write_picker(Path("cow_out/cow-pivot.html"),
                 [("cow-pivot", "Gen cow grazing (image-space head pivot)", 220, frames)])
    print(f"{len(frames)} frames -> {d}")


if __name__ == "__main__":
    main()
