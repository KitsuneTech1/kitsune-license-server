#!/usr/bin/env python3
"""Render each ASCII candidate's representative frame to a monospace image and
tile them into contact-sheet PNG(s) so the result can actually be eyeballed.

Usage: python render_contact.py FRAMES_DIR OUT_PREFIX [--cols 3] [--fp 14]
"""
from __future__ import annotations
import argparse, math
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

FONT_CANDIDATES = [
    r"C:\Windows\Fonts\consola.ttf",
    r"C:\Windows\Fonts\cour.ttf",
    r"C:\Windows\Fonts\lucon.ttf",
]


def load_font(size):
    for f in FONT_CANDIDATES:
        if Path(f).exists():
            return ImageFont.truetype(f, size)
    return ImageFont.load_default()


def render_text(lines, font, fg=(232, 162, 79), bg=(12, 12, 12), pad=8):
    # monospace cell metrics
    bbox = font.getbbox("M")
    cw = font.getlength("M")
    ch = bbox[3] - bbox[1] + 3
    cols = max((len(l) for l in lines), default=1)
    W = int(cw * cols) + pad * 2
    H = int(ch * len(lines)) + pad * 2
    img = Image.new("RGB", (W, H), bg)
    d = ImageDraw.Draw(img)
    for i, l in enumerate(lines):
        d.text((pad, pad + int(i * ch)), l, font=font, fill=fg)
    return img


def label_bar(width, text, font, h=26, bg=(28, 24, 18), fg=(244, 240, 231)):
    img = Image.new("RGB", (width, h), bg)
    d = ImageDraw.Draw(img)
    d.text((8, 4), text, font=font, fill=fg)
    return img


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("frames_dir")
    ap.add_argument("out_prefix")
    ap.add_argument("--cols", type=int, default=3)
    ap.add_argument("--fp", type=int, default=14)
    ap.add_argument("--per-sheet", type=int, default=12)
    a = ap.parse_args()

    font = load_font(a.fp)
    lbl_font = load_font(16)
    cand_dirs = sorted(p for p in Path(a.frames_dir).iterdir() if p.is_dir())

    tiles = []
    for cd in cand_dirs:
        frames = sorted(cd.glob("frame_*.txt"))
        if not frames:
            continue
        mid = frames[len(frames) // 2]
        lines = mid.read_text(encoding="utf-8").rstrip("\n").split("\n")
        body = render_text(lines, font)
        bar = label_bar(body.width, cd.name, lbl_font)
        tile = Image.new("RGB", (body.width, body.height + bar.height), (12, 12, 12))
        tile.paste(bar, (0, 0))
        tile.paste(body, (0, bar.height))
        tiles.append(tile)

    # uniform tile size
    tw = max(t.width for t in tiles); th = max(t.height for t in tiles)
    gap = 14
    per = a.per_sheet
    sheets = math.ceil(len(tiles) / per)
    out_paths = []
    for s in range(sheets):
        chunk = tiles[s * per:(s + 1) * per]
        cols = a.cols
        rows = math.ceil(len(chunk) / cols)
        W = cols * tw + (cols + 1) * gap
        H = rows * th + (rows + 1) * gap
        sheet = Image.new("RGB", (W, H), (8, 7, 6))
        for i, t in enumerate(chunk):
            r, c = divmod(i, cols)
            x = gap + c * (tw + gap)
            y = gap + r * (th + gap)
            sheet.paste(t, (x, y))
        op = f"{a.out_prefix}_{s+1}.png"
        sheet.save(op)
        out_paths.append(op)
        print(op, sheet.size)
    print(f"{len(tiles)} tiles -> {len(out_paths)} sheet(s)")


if __name__ == "__main__":
    main()
