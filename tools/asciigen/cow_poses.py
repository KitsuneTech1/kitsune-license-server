#!/usr/bin/env python3
"""Generate a sequence of cow POSE keyframes for animation (head up -> down ->
eating -> up -> looking). Same seed across poses to keep the cow's identity as
consistent as possible. Cutout + save to gen_poses/.

Run with pixi python:
  C:\\Users\\Owner\\.pixi\\bin\\python.exe cow_poses.py [SEED]
"""
from __future__ import annotations
import os, sys, io, time
from pathlib import Path

sys.path.insert(0, os.environ.get("SPRITE_FORGE_PATH", "sprite-forge"))
import comfy_client  # noqa: E402

STYLE = ("strict left-facing side profile, full body lateral view seen exactly "
         "from the side, flat 2d vector mascot illustration, bold thick black "
         "outline, simple clean shapes, flat solid colors, all four legs visible "
         "in profile, a cow with horns, centered, plain solid white background, "
         "sticker art, high contrast, no gradient, no shading, no text")
NEG = ("front view, facing camera, three-quarter view, frontal, looking at viewer, "
       "photograph, realistic, 3d render, soft gradient shading, blurry, lowres, "
       "text, words, watermark, busy background, multiple animals, sprite sheet, "
       "extra limbs, deformed, cropped, frame, border")

# ordered animation keyframes
POSES = [
    ("01_up",      "standing still with its head raised up, alert"),
    ("02_half",    "lowering its head halfway down toward the ground"),
    ("03_down",    "head reaching all the way down to the ground"),
    ("04_eat",     "head down eating grass from the ground, grazing, mouth at the grass"),
    ("05_lift",    "raising its head back up from the ground, chewing"),
    ("06_look",    "head raised and turned slightly, looking around curiously"),
]

HERE = Path(__file__).resolve().parent
OUT = HERE / "gen_poses"


def cutout(png_bytes):
    from rembg import remove
    from PIL import Image
    img = Image.open(io.BytesIO(remove(png_bytes))).convert("RGBA")
    bbox = img.split()[-1].getbbox()
    return img.crop(bbox) if bbox else img


def main():
    seed = int(sys.argv[1]) if len(sys.argv) > 1 and sys.argv[1].isdigit() else 4242
    OUT.mkdir(exist_ok=True)
    print(f"comfy: {comfy_client.reachable()}  seed={seed}")
    for name, pose in POSES:
        pos = f"{STYLE}, the cow is {pose}"
        t0 = time.time()
        cb = lambda v, m: print(f"  {name} {v}/{m}", end="\r")
        png = comfy_client.generate(pos, NEG, width=1024, height=1024, seed=seed,
                                    fast=True, on_progress=cb)
        img = cutout(png)
        p = OUT / f"{name}.png"
        img.save(p)
        print(f"  {name:10s} {img.size}  {time.time()-t0:.0f}s")
    print(f"-> {OUT}")


if __name__ == "__main__":
    main()
