#!/usr/bin/env python3
"""
asciigen — turn animal GIFs/images into animated ASCII candidates.

Self-contained: only needs Pillow. No chafa/jp2a/ffmpeg required.

Fans out  sources x style-presets  into a batch of distinct animated ASCII
candidates, then writes:
  - out/frames/<candidate>/frame_000.txt ...   (portable plain-text frames; what the
                                                Rust activator will actually play)
  - out/<candidate>.txt                          (all frames in one file, FF-separated)
  - out/picker.html                              (open in a browser; every candidate
                                                  animates side-by-side so you can pick)

Usage:
  python gen.py                       # uses ./sources -> ./out, all presets
  python gen.py --width 90 --max-frames 16
  python gen.py --presets fine,classic,edges
  python gen.py --sources sources --out out

Pick a winner, then we wire its frames into the activator.
"""

from __future__ import annotations

import argparse
import html
import shutil
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageEnhance, ImageFilter, ImageOps, ImageSequence

# ----------------------------------------------------------------------------
# Character ramps (dark -> light, i.e. dense ink -> empty).
# On a black terminal a *denser* glyph reads as brighter, so by default we map
# brighter pixels to denser glyphs (invert=True handled per-preset).
# ----------------------------------------------------------------------------
RAMP_LONG = (
    "$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/\\|()1{}[]?-_+~<>i!lI;:,\"^`'. "
)
RAMP_CLASSIC = "@%#*+=-:. "
RAMP_BOLD = "@#+. "
RAMP_BLOCKS = "█▓▒░ "        # not strictly 7-bit ASCII but ubiquitous + portable
RAMP_DITHER = "# "           # 1-bit, paired with PIL dithering


@dataclass(frozen=True)
class Preset:
    name: str
    ramp: str
    width: int
    contrast: float = 1.0
    brightness: float = 1.0
    edges: bool = False          # run FIND_EDGES first (line-art look)
    dither: bool = False         # 1-bit Floyd-Steinberg, map to RAMP_DITHER
    autocontrast: bool = True


PRESETS: dict[str, Preset] = {
    # detailed, soft gradients
    "fine":    Preset("fine",    RAMP_LONG,    92, contrast=1.15),
    # the familiar @%#*+=-:. terminal look
    "classic": Preset("classic", RAMP_CLASSIC, 74, contrast=1.25),
    # high-contrast, fewer glyphs, very readable small
    "bold":    Preset("bold",    RAMP_BOLD,    58, contrast=1.6, brightness=1.05),
    # shaded block fill (heavier, poster-like)
    "blocks":  Preset("blocks",  RAMP_BLOCKS,  72, contrast=1.2),
    # outline / line-art via edge detection
    "edges":   Preset("edges",   RAMP_CLASSIC, 80, edges=True, contrast=1.4,
                      autocontrast=True),
    # 1-bit stippled
    "dither":  Preset("dither",  RAMP_DITHER,  84, dither=True),
}

CELL_ASPECT = 0.5  # monospace cell is ~2x tall as wide


INK_THRESHOLD = 0.14   # below this = treated as background (space)


def _ink_grid(frame: Image.Image, p: Preset):
    """Return (cols, rows, ink[y][x]) where ink in [0,1]: 1 = full subject, 0 = bg.

    Auto-detects background polarity from the border so a dark-animal-on-white
    plate and a white-silhouette-on-black plate both render the *animal* in
    glyphs and leave the background empty.
    """
    rgba = frame.convert("RGBA")
    alpha = rgba.split()[-1]
    gray = rgba.convert("L")

    if p.edges:
        gray = gray.filter(ImageFilter.FIND_EDGES)
    if p.brightness != 1.0:
        gray = ImageEnhance.Brightness(gray).enhance(p.brightness)
    if p.contrast != 1.0:
        gray = ImageEnhance.Contrast(gray).enhance(p.contrast)
    if p.autocontrast:
        gray = ImageOps.autocontrast(gray, cutoff=2)

    w, h = gray.size
    cols = p.width
    rows = max(1, round(cols * (h / w) * CELL_ASPECT))
    gray = gray.resize((cols, rows), Image.LANCZOS)
    alpha = alpha.resize((cols, rows), Image.LANCZOS)
    gpx, apx = gray.load(), alpha.load()

    # background luminance = mean of the 1px border (corners are ~always bg)
    border = []
    for x in range(cols):
        border += [gpx[x, 0], gpx[x, rows - 1]]
    for y in range(rows):
        border += [gpx[0, y], gpx[cols - 1, y]]
    bg = sum(border) / len(border)
    subject_is_dark = bg >= 128  # bright border => the animal is the dark part

    ink = [[0.0] * cols for _ in range(rows)]
    for y in range(rows):
        for x in range(cols):
            if apx[x, y] < 32:          # genuine transparency = background
                continue
            lum = gpx[x, y]
            if subject_is_dark:
                v = (bg - lum) / bg if bg > 0 else 0.0
            else:
                v = (lum - bg) / (255 - bg) if bg < 255 else 0.0
            v = 0.0 if v < 0 else (1.0 if v > 1 else v)
            ink[y][x] = v ** 0.85       # gamma: fill the silhouette a touch
    return cols, rows, ink


def _crop_ink(cols, rows, ink):
    """Crop the ink grid to its content bounding box (threshold INK_THRESHOLD)."""
    xs = [x for y in range(rows) for x in range(cols) if ink[y][x] > INK_THRESHOLD]
    ys = [y for y in range(rows) for x in range(cols) if ink[y][x] > INK_THRESHOLD]
    if not xs:
        return None
    x0, x1, y0, y1 = min(xs), max(xs), min(ys), max(ys)
    grid = [row[x0:x1 + 1] for row in ink[y0:y1 + 1]]
    return grid


def frame_to_ascii(frame: Image.Image, p: Preset) -> list[str]:
    """Convert a single PIL frame to a list of text rows under preset `p`."""
    cols, rows, ink = _ink_grid(frame, p)
    grid = _crop_ink(cols, rows, ink)
    if grid is None:
        return []
    h, w = len(grid), len(grid[0])

    if p.dither:
        from PIL import Image as _I
        im = _I.new("L", (w, h))
        im.putdata([int(round(grid[y][x] * 255)) for y in range(h) for x in range(w)])
        bw = im.convert("1").load()     # Floyd-Steinberg
        return [
            "".join("#" if bw[x, y] else " " for x in range(w)).rstrip()
            for y in range(h)
        ]

    ramp = p.ramp                       # dense -> empty
    n = len(ramp) - 1
    out_rows = []
    for y in range(h):
        chars = []
        for x in range(w):
            v = grid[y][x]
            if v < INK_THRESHOLD:
                chars.append(" ")
            else:
                chars.append(ramp[int(round((1.0 - v) * n))])
        out_rows.append("".join(chars).rstrip())
    return out_rows


def load_frames(path: Path, max_frames: int):
    img = Image.open(path)
    frames = list(ImageSequence.Iterator(img))
    if len(frames) <= 1:
        return [frames[0].copy()] if frames else []
    if len(frames) > max_frames:
        step = len(frames) / max_frames
        frames = [frames[int(i * step)] for i in range(max_frames)]
    return [f.copy() for f in frames]


def pad_frames(frames_rows: list[list[str]]) -> list[str]:
    """Pad every frame to identical size; center horizontally, bottom-align
    vertically so the critter animates in place with its feet grounded."""
    width = max((max((len(r) for r in fr), default=0) for fr in frames_rows), default=0)
    height = max((len(fr) for fr in frames_rows), default=0)
    out = []
    for fr in frames_rows:
        rows = []
        for r in fr:
            pad = width - len(r)
            left = pad // 2
            rows.append(" " * left + r + " " * (pad - left))
        while len(rows) < height:                  # add blank rows on top
            rows.insert(0, " " * width)
        out.append("\n".join(rows))
    return out


def slugify(s: str) -> str:
    return "".join(c if c.isalnum() else "-" for c in s).strip("-").lower()


def main() -> int:
    ap = argparse.ArgumentParser(description="Generate animated ASCII candidates.")
    here = Path(__file__).resolve().parent
    ap.add_argument("--sources", default=str(here / "sources"))
    ap.add_argument("--out", default=str(here / "out"))
    ap.add_argument("--width", type=int, default=None,
                    help="override every preset width")
    ap.add_argument("--max-frames", type=int, default=14)
    ap.add_argument("--presets", default=",".join(PRESETS),
                    help="comma list: " + ",".join(PRESETS))
    args = ap.parse_args()

    src_dir = Path(args.sources)
    out_dir = Path(args.out)
    frames_root = out_dir / "frames"
    if frames_root.exists():
        shutil.rmtree(frames_root)
    frames_root.mkdir(parents=True, exist_ok=True)

    chosen = [PRESETS[n.strip()] for n in args.presets.split(",") if n.strip() in PRESETS]
    if args.width:
        chosen = [Preset(**{**p.__dict__, "width": args.width}) for p in chosen]

    exts = {".gif", ".png", ".jpg", ".jpeg", ".webp", ".bmp"}
    sources = sorted(p for p in src_dir.glob("*") if p.suffix.lower() in exts)
    if not sources:
        print(f"No source images in {src_dir}. Drop some animal GIFs there.")
        return 1

    candidates = []  # (id, label, speed_ms, [frame_strings])
    for sp in sources:
        try:
            raw_frames = load_frames(sp, args.max_frames)
        except Exception as e:
            print(f"  ! skip {sp.name}: {e}")
            continue
        if not raw_frames:
            continue
        for p in chosen:
            rows_per_frame = [frame_to_ascii(f, p) for f in raw_frames]
            rows_per_frame = [r for r in rows_per_frame if r]
            if not rows_per_frame:
                continue
            padded = pad_frames(rows_per_frame)
            cid = f"{slugify(sp.stem)}--{p.name}"
            label = f"{sp.stem} · {p.name}"
            speed = 120 if len(padded) > 6 else 380
            candidates.append((cid, label, speed, padded))

            cdir = frames_root / cid
            cdir.mkdir(parents=True, exist_ok=True)
            for i, fr in enumerate(padded):
                (cdir / f"frame_{i:03d}.txt").write_text(fr + "\n", encoding="utf-8")
            (out_dir / f"{cid}.txt").write_text(
                "\f".join(padded) + "\n", encoding="utf-8")
            print(f"  + {cid:32s} {len(padded):2d} frames  "
                  f"{len(padded[0].splitlines())}x{len(padded[0].splitlines()[0])}")

    if not candidates:
        print("No candidates produced.")
        return 1

    write_picker(out_dir / "picker.html", candidates)
    print(f"\n{len(candidates)} candidates -> {out_dir/'picker.html'}")
    print("Open it in a browser and pick the best, then tell me the candidate id.")
    return 0


def write_picker(path: Path, candidates) -> None:
    cards = []
    for cid, label, speed, frames in candidates:
        src_pres = "".join(
            f"<pre>{html.escape(fr)}</pre>" for fr in frames
        )
        cards.append(f"""
  <div class="card">
    <div class="cap"><span class="name">{html.escape(label)}</span>
      <span class="id">{html.escape(cid)}</span></div>
    <div class="term">
      <div class="bar"><span class="d r"></span><span class="d y"></span>
        <span class="d g"></span><span class="t">activate</span></div>
      <div class="screen">
        <pre class="stage" data-speed="{speed}"></pre>
        <div class="dim">Enter your license key:</div>
        <div class="key">KTSN-1A2B-3C4D-<span class="cur">_</span></div>
        <div class="src">{src_pres}</div>
      </div>
    </div>
  </div>""")

    doc = f"""<!DOCTYPE html>
<html lang="en"><head><meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>asciigen candidates — pick the best</title>
<style>
  :root {{ font-family:'Segoe UI',system-ui,sans-serif; }}
  * {{ box-sizing:border-box; }}
  body {{ margin:0; background:#16130f; color:#f4f0e7; padding:36px 22px 90px; }}
  header {{ max-width:1320px; margin:0 auto 28px; }}
  h1 {{ font-size:26px; margin:0 0 6px; letter-spacing:-.4px; }}
  h1 .ac {{ color:#c14a28; }}
  header p {{ color:#b8ae9c; max-width:820px; margin:0; }}
  .grid {{ max-width:1320px; margin:0 auto; display:grid;
           grid-template-columns:repeat(auto-fill,minmax(360px,1fr)); gap:22px; }}
  .card {{ background:#1c1812; border:1px solid #322a20; border-radius:14px;
           overflow:hidden; transition:transform .15s,border-color .15s; }}
  .card:hover {{ transform:translateY(-3px); border-color:#c14a28; }}
  .cap {{ padding:13px 16px 4px; display:flex; justify-content:space-between;
          align-items:baseline; gap:10px; }}
  .cap .name {{ font-weight:700; font-size:15px; }}
  .cap .id {{ color:#7d735f; font-size:11px; font-family:monospace;
              user-select:all; }}
  .term {{ font-family:'Cascadia Code','JetBrains Mono','Consolas',monospace;
           background:#0c0c0c; color:#cfcfcf; margin:0 14px 14px;
           border-radius:8px; overflow:hidden; }}
  .bar {{ background:#1e1e1e; color:#9a9a9a; font-size:11px; padding:7px 11px;
          display:flex; gap:7px; align-items:center; }}
  .d {{ width:10px;height:10px;border-radius:50%;display:inline-block; }}
  .d.r{{background:#ff5f56}} .d.y{{background:#ffbd2e}} .d.g{{background:#27c93f}}
  .bar .t {{ margin-left:6px; opacity:.7; }}
  .screen {{ padding:14px; }}
  .stage {{ color:#e8a24f; font-size:10px; line-height:1.0; white-space:pre;
            margin:0 0 10px; min-height:150px; overflow:auto; }}
  .dim {{ color:#6f6f6f; font-size:13px; }}
  .key {{ color:#cccccc; font-size:13px; }}
  .cur {{ animation:cur 1s steps(1) infinite; }}
  @keyframes cur {{ 50%{{opacity:0}} }}
  .src {{ display:none; }}
  footer {{ max-width:1320px; margin:36px auto 0; color:#8c8270; font-size:13px; }}
</style></head>
<body>
<header>
  <h1>asciigen <span class="ac">candidates</span></h1>
  <p>Every tile is a (source × style) animated ASCII candidate for the license
     activator. Hover to highlight; the small monospace text under each name is the
     candidate id — copy it and tell me which one wins.</p>
</header>
<div class="grid">{''.join(cards)}</div>
<footer>{len(candidates)} candidates. Generated by tools/asciigen/gen.py.</footer>
<script>
document.querySelectorAll('.stage').forEach(stage => {{
  const src = stage.parentElement.querySelector('.src');
  const frames = [...src.querySelectorAll('pre')].map(p => p.textContent);
  if(!frames.length) return;
  let i = 0; stage.textContent = frames[0];
  const speed = parseInt(stage.dataset.speed || '300', 10);
  if(frames.length > 1)
    setInterval(() => {{ i = (i+1) % frames.length; stage.textContent = frames[i]; }}, speed);
}});
</script>
</body></html>"""
    path.write_text(doc, encoding="utf-8")


if __name__ == "__main__":
    raise SystemExit(main())
