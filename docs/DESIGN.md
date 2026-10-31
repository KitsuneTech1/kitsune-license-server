# Kitsune License Server — Design Notes

Captured during brainstorming, 2026-06-15.

## Locked decisions

- **Validation model:** Online activation (phone-home). Enables revocation, rate-limiting, seat enforcement, install tracking.
- **Storage:** SQLite, single node.
- **Crypto:**
  - HTTPS/TLS in transit.
  - License keys stored hashed at rest.
  - Activation responses are Ed25519-signed tokens; client verifies with an embedded public key.
- **Web UI:** Admin dashboard (products, generate/revoke keys, seat limits, activations + devices).
- **Embeddable page:** view-only — shows an end user their activated devices + info. No activate/deactivate controls.
- **Client:** the protected program installs via terminal after key entry → a Rust CLI activator prompts for the key, activates online, verifies the signed token, gates install.
- **Mascot:** animated ASCII animal during activation. Terminal theme stays plain/default.

## Open questions

- [ ] Which animal? (fox / cat / bunny / owl / penguin / dog — or other). Mockups in `mockups/animal-picker.html`.
- [ ] Art fidelity tier:
  - Tier 1 — hand-crafted animated ASCII (portable, blocky).
  - Tier 2 — image→colored ASCII from a reference (detailed, runs anywhere). **Recommended.**
  - Tier 3 — true sixel/GIF rendering with ASCII fallback (coolest, most work).
  - Recommendation: Tier 2 + sixel upgrade when supported, ASCII fallback otherwise.
- [ ] Reference image: provide a fox image/GIF, or source a CC fox.

## API sketch (to refine)

- `POST /activate` — `{ key, device_fingerprint, meta }` → `{ signed_token }` (or error: invalid/revoked/seat-limit).
- `POST /validate` — periodic re-check; returns current status + refreshed token.
- `POST /heartbeat` — updates last-seen for a device.
- Admin API (behind Authentik): products CRUD, key generation, revoke key/device, list activations.

## Mascot generator (`tools/asciigen/`) — 2026-06-15

Self-contained Pillow pipeline (no chafa/jp2a needed). Fans out
`sources/ × style-presets` into animated ASCII candidates; writes per-candidate
plain-text frames (`gallery_v2/frames/<id>/frame_NNN.txt`), one-file FF-separated
copies, and an animated `picker.html`. Presets: `fine, classic, bold, blocks,
edges, dither`. Auto-detects background polarity (so dark-on-light and
light-on-dark both render the *animal* in glyphs, background = spaces), crops to
content, centers + bottom-aligns frames so the critter animates in place.

Run: `python tools/asciigen/gen.py --out gallery_v2`

### Finding: source quality is the bottleneck, not the pipeline
- **Muybridge / Zoopraxography GIFs** (public domain, downloaded to `sources/`)
  look bad as ASCII: the "Descriptive Zoopraxography" ones are multi-figure
  discs on a curved arc; the rest are grainy 1880s photos. → speckly, unclear.
- **Clean source = beautiful ASCII.** Running the pipeline on Moo's own fox logo
  (`kitsunetech-site/public/logo.png`, RGBA) produced a crisp, recognizable fox.
  Bold/high-contrast/transparent subjects ASCII-ize well; photos don't.
- **Static, though.** The logo is one frame — gorgeous but not animated.

### Roster + ranking (2026-06-15, route A: clean animated sources)
Tooling added: `slice_sheet.py` (alpha-projection frame slicer), `build_animals.py`
(picks widest/side row per sheet → one normalized GIF per animal in `animals/`),
`render_contact.py` (renders each ASCII candidate's mid-frame to a monospace PNG
contact sheet in `review/` so output can be eyeballed and ranked).

Sources: **Wild Animals** (CC0: fox, wolf, bear, deer, boar, rabbit), **LPC farm**
(CC-BY: cow, pig, sheep, llama, chicken), **LPC birds** (CC-BY: bird, eagle,
sparrow, robin — note: 15 LPC birds collapse to ~3 shapes in mono ASCII since
variants differ only by color), **fox-warrior** (CC-BY, anthropomorphic).
Muybridge GIFs abandoned (grainy/multi-figure → noise).

Ranked keepers (best→worst), in `ranked_out/picker.html`:
deer, sheep, cow, llama, pig, boar, bear, wolf, fox, bird, eagle, foxwarrior.
Dropped as terrible: chicken (2 frames → two triangles), sparrow, robin (tiny
blobs), rabbit (blobby).

**Count reality:** ~12 solid distinct animated animals from clean CC0/CC-BY packs.
Hitting a full 25 *distinct* needs more species packs (cat, dog, penguin, dino,
snake, horse, bat, frog, sea creatures). Pending Moo's call on whether to source them.

### Route: generate our own images (2026-06-15)
Best route — stop scavenging sprites, gen clean source ourselves:
- `gen_images.py` drives the **local ComfyUI** (`:8188`, SDXL via SpriteForge's
  `comfy_client.py`) with an ASCII-tuned prompt (bold flat vector mascot, thick
  outline, **strict side profile**, plain bg) + rembg cutout → `gen_src/`. ~5s/img
  fast (Lightning). Run with pixi python (`~/.pixi/bin/python`).
- **Key insight:** gen mascots have internal shading → luminance ASCII = noise.
  ASCII the **alpha shape as a solid silhouette** instead → crisp, readable.
- `cow_anim.py`: silhouette + auto-found eye/mouth/tail anchors + procedural overlay
  (eye blink, grass sprig nibbled at the mouth, tail flick, shimmering ground).
  Output: `cow_out/frames/cow-graze/`, `cow_out/cow-graze.html`.
- Animation decision: **procedural over a gen still** (chosen over local/cloud I2V —
  ComfyUI has Hunyuan/LTXV/Grok/Kling nodes if we ever want true motion).
- TODO: option for a bolder SOLID silhouette (vs current soft-ramp halftone);
  generalize `cow_anim` to any animal; pick final mascot; wire frames into Rust CLI.

### Earlier open decision (mascot art route)
- **A — clean animated source:** source CC0 pixel/vector fox (or other) sprite
  animations → straight through the pipeline → beautiful *and* moving.
- **B — animate the logo:** ASCII the fox logo + procedural intro (typewriter
  draw-on reveal, idle bob, blink, circuit-dot pulse). Most on-brand; animation
  is an effect, not the animal galloping.
- **C — curated Muybridge:** keep these but crop to single subject + denoise.
  Quick, vintage-rotoscope vibe, lower ceiling.

## Tooling

See README "Planned stack". Image→ASCII generators considered: `chafa`, `artem`,
`ascii-image-converter` — superseded by the self-contained `tools/asciigen` pipeline.
