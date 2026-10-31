#!/usr/bin/env python3
"""Slice a transparent sprite sheet into per-frame PNGs using alpha projection.

Finds horizontal bands (rows of sprites) separated by fully-transparent rows,
then within each band finds frames separated by fully-transparent columns. No
need to know the grid size. Writes <out>/<sheet>_r<row>_f<frame>.png and an
animated GIF per row for quick preview.

Usage: python slice_sheet.py SHEET.png OUTDIR [--maxwidth 2400]
"""
from __future__ import annotations
import argparse
from pathlib import Path
from PIL import Image

Image.MAX_IMAGE_PIXELS = None


def bands(projection, gap=1, min_run=2):
    """Return (start,end) spans where projection>0, merging gaps < `gap`."""
    spans, start = [], None
    zero = 0
    for i, v in enumerate(projection):
        if v > 0:
            if start is None:
                start = i
            zero = 0
        else:
            if start is not None:
                zero += 1
                if zero >= gap:
                    end = i - zero + 1
                    if end - start >= min_run:
                        spans.append((start, end))
                    start = None
    if start is not None and len(projection) - start >= min_run:
        spans.append((start, len(projection)))
    return spans


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("sheet")
    ap.add_argument("outdir")
    ap.add_argument("--maxwidth", type=int, default=2400)
    a = ap.parse_args()

    im = Image.open(a.sheet).convert("RGBA")
    if im.width > a.maxwidth:                 # downscale: ASCII needs <100 cols
        s = a.maxwidth / im.width
        im = im.resize((a.maxwidth, round(im.height * s)), Image.LANCZOS)
    alpha = im.split()[-1]
    apx = alpha.load()
    W, H = im.size

    row_proj = [sum(1 for x in range(W) if apx[x, y] > 24) for y in range(H)]
    row_spans = bands(row_proj, gap=2, min_run=4)

    out = Path(a.outdir); out.mkdir(parents=True, exist_ok=True)
    stem = Path(a.sheet).stem
    total = 0
    for r, (y0, y1) in enumerate(row_spans):
        col_proj = [sum(1 for y in range(y0, y1) if apx[x, y] > 24) for x in range(W)]
        col_spans = bands(col_proj, gap=2, min_run=4)
        frames = []
        for f, (x0, x1) in enumerate(col_spans):
            crop = im.crop((x0, y0, x1, y1))
            crop.save(out / f"{stem}_r{r:02d}_f{f:02d}.png")
            frames.append(crop)
            total += 1
        # quick GIF preview of the row (uniform canvas)
        if len(frames) > 1:
            cw = max(c.width for c in frames); ch = max(c.height for c in frames)
            canv = []
            for c in frames:
                bg = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
                bg.alpha_composite(c, ((cw - c.width) // 2, ch - c.height))
                canv.append(bg.convert("P", palette=Image.ADAPTIVE))
            canv[0].save(out / f"{stem}_row{r:02d}.gif", save_all=True,
                         append_images=canv[1:], duration=110, loop=0, disposal=2)
        print(f"row {r:02d}: y[{y0}:{y1}]  {len(col_spans)} frames")
    print(f"{total} frames -> {out}")


if __name__ == "__main__":
    main()
