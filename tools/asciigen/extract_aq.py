#!/usr/bin/env python3
"""Extract hand-crafted ASCII art (shape frames) from the asciiquarium perl
source and render a contact sheet of the creatures."""
import re
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
SRC = (HERE / "handcrafted/asciiquarium.pl").read_text(encoding="utf-8", errors="replace")


def sub_text(name):
    m = re.search(r"sub " + re.escape(name) + r"\b.*?(?=\nsub |\Z)", SRC, re.S)
    return m.group(0) if m else ""


def blocks(text):
    """Yield contents of q{...} blocks (handles \\} escapes)."""
    out, i = [], 0
    while True:
        j = text.find("q{", i)
        if j < 0:
            break
        k, buf = j + 2, []
        while k < len(text):
            c = text[k]
            if c == "\\" and k + 1 < len(text):
                buf.append(text[k + 1]); k += 2; continue
            if c == "}":
                break
            buf.append(c); k += 1
        out.append("".join(buf))
        i = k + 1
    return out


def is_shape(b):
    return any(ch in b for ch in "<>()/\\=_'~:.@") and \
        sum(c.isdigit() for c in b) < len(b.replace(" ", "").replace("\n", "")) * 0.5


def first_shape(name):
    for b in blocks(sub_text(name)):
        if is_shape(b):
            return b.replace("?", " ").strip("\n")
    return None


CREATURES = [
    ("little fish", "add_new_fish"),
    ("shark", "add_shark"),
    ("ship", "add_ship"),
    ("whale", "add_whale"),
    ("sea serpent", "add_new_monster"),
    ("big fish", "add_big_fish_1"),
    ("castle", "add_castle"),
]

font = ImageFont.truetype(r"C:\Windows\Fonts\consola.ttf", 15)
lbl = ImageFont.truetype(r"C:\Windows\Fonts\consola.ttf", 16)
cw = font.getlength("M"); bb = font.getbbox("M"); ch = bb[3] - bb[1] + 4

tiles = []
for label, sub in CREATURES:
    art = first_shape(sub)
    if not art:
        print("  ! no art for", sub); continue
    lines = art.split("\n")
    cols = max(len(l) for l in lines)
    img = Image.new("RGB", (int(cw * cols) + 20, int(ch * len(lines)) + 28), (4, 18, 34))
    d = ImageDraw.Draw(img)
    d.text((8, 4), label, font=lbl, fill=(180, 230, 255))
    for i, l in enumerate(lines):
        d.text((8, 24 + int(i * ch)), l, font=font, fill=(120, 230, 200))
    tiles.append(img)
    print(f"  + {label:14s} {len(lines)}x{cols}")

W = max(t.width for t in tiles)
H = sum(t.height for t in tiles) + 10 * len(tiles)
sheet = Image.new("RGB", (W + 20, H + 10), (2, 10, 20)); y = 8
for t in tiles:
    sheet.paste(t, (10, y)); y += t.height + 10
sheet.save(HERE / "cow_out/aquarium_cast.png")
print("wrote cow_out/aquarium_cast.png", sheet.size)
