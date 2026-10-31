#!/usr/bin/env python3
"""Grazing animation by a smooth whole-front BEND (not a detached head chunk).

Cow faces left. Each column is shifted down by a weight that is ~1 at the nose
and eases to 0 around the shoulder, so the neck curves and the front of the body
dips *with* the head as one cohesive motion. Rear + hind legs stay planted. Then
silhouette each frame. Smaller, subtler, no neck seam.
"""
from __future__ import annotations
import os
from pathlib import Path
from PIL import Image
import numpy as np

HERE = Path(__file__).resolve().parent

HEAD_FULL = 0.26   # x < this : full bend (head drops as a unit)
BEND_END = 0.60    # x > this : no bend (rear stays planted)
AMP = 0.20         # max downward drop as a fraction of height (subtle)
TUCK = 0.5         # how much the head also pulls back toward the body


def smoothstep(t):
    t = np.clip(t, 0, 1)
    return t * t * (3 - 2 * t)


def weight(W):
    x = np.arange(W) / W
    w = np.ones(W)
    ramp = (x - HEAD_FULL) / (BEND_END - HEAD_FULL)
    w = np.where(x <= HEAD_FULL, 1.0, 1.0 - smoothstep(ramp))
    w = np.where(x >= BEND_END, 0.0, w)
    return w


def bend(img, amount):
    """amount in [0,1]; bend the front down by amount*AMP*H."""
    arr = np.array(img.convert("RGBA"))
    H, W = arr.shape[:2]
    w = weight(W)
    out = np.zeros_like(arr)
    maxdy = AMP * H
    for x in range(W):
        dy = int(round(amount * maxdy * w[x]))
        dx = int(round(amount * maxdy * w[x] * TUCK))  # also tuck head toward body
        sx = x - dx
        if sx < 0 or sx >= W:
            continue
        col = arr[:, sx, :]
        if dy > 0:
            out[dy:, x, :] = col[:H - dy, :]
        else:
            out[:, x, :] = col
    return Image.fromarray(out, "RGBA")


def silhouette_grid(img, cols=64):
    a = img.split()[-1]
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
            # bold SOLID silhouette: one fill glyph, light edge, else empty
            if v >= 0.55:
                ch = "#"
            elif v >= 0.30:
                ch = "+"
            else:
                ch = " "
            line.append(ch)
        grid.append(line)
    return grid


SEQ = [0.0, 0.35, 0.7, 1.0, 1.0, 0.7, 0.35, 0.0]
EATING = {3, 4}
GROUND = ("v.,v\\,.v,.\\v.,.v,\\.v,.v\\,.v,.v\\,.v,.\\v,.v,\\.v,.v.,\\v.,v,.v"
          "\\,.v,.v,\\.v,.v\\,.v,.v\\")


def main():
    os.chdir(HERE)
    src = "gencow2_src/cowA.png"
    base = Image.open(src).convert("RGBA")
    # pad bottom so the dipped head has room
    W, H = base.size
    pad = int(AMP * H) + 8
    canvas = Image.new("RGBA", (W, H + pad), (0, 0, 0, 0))
    canvas.paste(base, (0, 0))

    grids = [silhouette_grid(bend(canvas, a), 64) for a in SEQ]
    Wc = max(len(g[0]) for g in grids)
    Hc = max(len(g) for g in grids)
    PADL, PADR, PADB = 5, 4, 1
    frames = []
    for fi, g in enumerate(grids):
        gh, gw = len(g), len(g[0])
        top = Hc - gh
        left = (Wc - gw) // 2
        cv = [[" "] * (PADL + Wc + PADR) for _ in range(top)]
        for row in g:
            cv.append([" "] * (PADL + left) + row + [" "] * (PADR + (Wc - gw - left)))
        for _ in range(PADB):
            cv.append([" "] * (PADL + Wc + PADR))
        # ground grass
        gl = list(GROUND)
        for i in range(fi % 3, len(gl), 4):
            if gl[i] == ".":
                gl[i] = "v"
        gline = " " + "".join(gl)
        for i, c in enumerate(gline[:len(cv[0])]):
            if c != " ":
                cv[-1][i] = c
        # grass tuft at the lowered nose
        if fi in EATING:
            nose = None
            for rr in range(len(cv) - PADB - 1, -1, -1):
                for cc in range(len(cv[0])):
                    if cv[rr][cc] != " ":
                        nose = (rr, cc); break
                if nose:
                    break
            if nose:
                for k, ch in enumerate("\\|/"):
                    cc = nose[1] - 3 + k
                    if 0 <= cc < len(cv[0]):
                        cv[nose[0]][cc] = ch
        frames.append("\n".join("".join(r).rstrip() for r in cv))

    d = Path("cow_out/frames/cow-bend")
    d.mkdir(parents=True, exist_ok=True)
    for i, f in enumerate(frames):
        (d / f"frame_{i:03d}.txt").write_text(f + "\n", encoding="utf-8")
    from gen import write_picker
    write_picker(Path("cow_out/cow-bend.html"),
                 [("cow-bend", "Gen cow grazing (smooth front bend)", 200, frames)])
    print(f"{len(frames)} frames -> {d}")


if __name__ == "__main__":
    main()
