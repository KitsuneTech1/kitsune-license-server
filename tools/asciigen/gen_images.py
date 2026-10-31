#!/usr/bin/env python3
"""Generate our own animal images via the local ComfyUI (SDXL) and cut them out,
tuned to ASCII-ize cleanly: bold flat vector mascots, thick outlines, side profile,
plain background -> crisp silhouettes.

Run with SpriteForge's pixi python (has rembg + websocket):
  C:\\Users\\Owner\\.pixi\\bin\\python.exe gen_images.py cow 4
  C:\\Users\\Owner\\.pixi\\bin\\python.exe gen_images.py "horned highland cow" 6 --quality
"""
from __future__ import annotations
import os, sys, time
from pathlib import Path

sys.path.insert(0, os.environ.get("SPRITE_FORGE_PATH", "sprite-forge"))
import comfy_client  # noqa: E402

# ASCII likes bold, flat, high-contrast, single clean silhouette, side profile.
STYLE = ("strict left-facing side profile, full body lateral view seen exactly "
         "from the side, flat 2d vector mascot illustration, bold thick black "
         "outline, simple clean shapes, flat solid colors, all four legs visible "
         "in profile, centered, plain solid white background, sticker art, high "
         "contrast, no gradient, no shading, no text")
NEG = ("front view, facing camera, three-quarter view, frontal, looking at viewer, "
       "photograph, realistic, 3d render, soft gradient shading, blurry, lowres, "
       "text, words, watermark, signature, busy background, multiple animals, "
       "sprite sheet, extra limbs, deformed, cropped, frame, border")

HERE = Path(__file__).resolve().parent
OUT = HERE / "gen_src"
OUT.mkdir(exist_ok=True)


def cutout(png_bytes):
    from rembg import remove
    from PIL import Image
    import io
    cut = remove(png_bytes)
    img = Image.open(io.BytesIO(cut)).convert("RGBA")
    bbox = img.split()[-1].getbbox()
    if bbox:
        img = img.crop(bbox)
    return img


def main():
    if len(sys.argv) < 2:
        print("usage: gen_images.py SUBJECT [COUNT] [--quality]"); return 1
    subject = sys.argv[1]
    count = int(sys.argv[2]) if len(sys.argv) > 2 and sys.argv[2].isdigit() else 4
    fast = "--quality" not in sys.argv
    slug = "".join(c if c.isalnum() else "-" for c in subject).strip("-").lower()

    print(f"comfy reachable: {comfy_client.reachable()}  subject={subject!r} "
          f"count={count} mode={'fast' if fast else 'quality'}")
    pos = f"{STYLE}, a {subject}"
    saved = []
    for i in range(count):
        seed = 1000 + i * 7
        t0 = time.time()
        cb = lambda v, m: print(f"  [{i}] step {v}/{m}", end="\r")
        png = comfy_client.generate(pos, NEG, width=1024, height=1024, seed=seed,
                                    fast=fast, on_progress=cb)
        img = cutout(png)
        p = OUT / f"{slug}_{i}.png"
        img.save(p)
        saved.append(p)
        print(f"  [{i}] saved {p.name}  {img.size}  {time.time()-t0:.0f}s")
    print(f"{len(saved)} images -> {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
