"""Turn the source artwork in art/ into the files the app uses.

  art/logo.(png|webp|jpg)      -> src-tauri/app-icon.png, static/logo.png, static/favicon.png
                                  (then run `npx tauri icon src-tauri/app-icon.png`)
  art/wordmark.(png|webp|jpg)  -> static/wordmark.png  (white background removed, cropped)
  art/hero.(png|webp|jpg)      -> static/hero.jpg      (max 2560 wide)

Usage:  python scripts/process_art.py   (needs Pillow: pip install pillow)
"""

from collections import deque
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
ART = ROOT / "art"
STATIC = ROOT / "static"


def find(name: str):
    for ext in ("png", "webp", "jpg", "jpeg"):
        p = ART / f"{name}.{ext}"
        if p.exists():
            return p
    return None


def logo(src: Path):
    im = Image.open(src).convert("RGBA")
    w, h = im.size
    # Mask the corners so the rounded square has transparent corners.
    s, r = 4, round(w * 0.188)
    m = Image.new("L", (w * s, h * s), 0)
    ImageDraw.Draw(m).rounded_rectangle([0, 0, w * s - 1, h * s - 1], radius=r * s, fill=255)
    im.putalpha(m.resize((w, h), Image.LANCZOS))
    im.resize((1024, 1024), Image.LANCZOS).save(ROOT / "src-tauri" / "app-icon.png")
    im.resize((256, 256), Image.LANCZOS).save(STATIC / "logo.png")
    im.resize((64, 64), Image.LANCZOS).save(STATIC / "favicon.png")
    print("logo -> app-icon.png, logo.png, favicon.png")


def wordmark(src: Path):
    im = Image.open(src).convert("RGBA")
    w, h = im.size
    px = im.load()

    def whiteish(p, t=225):
        return p[0] > t and p[1] > t and p[2] > t

    # Flood-fill the background from the edges so white details inside the
    # letters (like the snowcap) are kept.
    bg = bytearray(w * h)
    q = deque((x, y) for x in range(w) for y in (0, h - 1)) + deque((x, y) for y in range(h) for x in (0, w - 1))
    while q:
        x, y = q.popleft()
        i = y * w + x
        if bg[i] or not whiteish(px[x, y]):
            continue
        bg[i] = 1
        for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
            if 0 <= nx < w and 0 <= ny < h and not bg[ny * w + nx]:
                q.append((nx, ny))

    # Background -> transparent; soften the anti-aliased edge next to it by
    # turning leftover white into transparency ("color to alpha").
    for y in range(h):
        for x in range(w):
            i = y * w + x
            if bg[i]:
                px[x, y] = (0, 0, 0, 0)
                continue
            near_bg = any(
                0 <= x + dx < w and 0 <= y + dy < h and bg[(y + dy) * w + x + dx]
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1), (2, 0), (-2, 0), (0, 2), (0, -2))
            )
            if near_bg:
                r, g, b, _ = px[x, y]
                a = max(255 - r, 255 - g, 255 - b) / 255
                if a < 0.02:
                    px[x, y] = (0, 0, 0, 0)
                else:
                    f = lambda c: max(0, min(255, round((c - (1 - a) * 255) / a)))
                    px[x, y] = (f(r), f(g), f(b), round(a * 255))

    box = im.getchannel("A").point(lambda a: 255 if a > 24 else 0).getbbox()
    if box:
        pad = 8
        im = im.crop((max(0, box[0] - pad), max(0, box[1] - pad), min(w, box[2] + pad), min(h, box[3] + pad)))
    if im.height > 160:
        im = im.resize((round(im.width * 160 / im.height), 160), Image.LANCZOS)
    im.save(STATIC / "wordmark.png")
    print(f"wordmark -> static/wordmark.png {im.size}")


def hero(src: Path):
    im = Image.open(src).convert("RGB")
    if im.width > 2560:
        im = im.resize((2560, round(im.height * 2560 / im.width)), Image.LANCZOS)
    im.save(STATIC / "hero.jpg", quality=86, optimize=True, progressive=True)
    print(f"hero -> static/hero.jpg {im.size}")


if __name__ == "__main__":
    done = False
    for name, fn in (("logo", logo), ("wordmark", wordmark), ("hero", hero)):
        src = find(name)
        if src:
            fn(src)
            done = True
    if not done:
        print("No artwork found in art/ (expected logo.*, wordmark.*, hero.*)")
