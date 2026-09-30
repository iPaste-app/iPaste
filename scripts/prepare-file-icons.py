"""Extract the approved duotone artwork into transparent, consistently sized WebP assets.

Requires Pillow. Uses the original generated sheet, retaining its white internal symbols.
"""
from collections import deque
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "generated-images/file-icons-2026-09-23/set-06-duotone.png"
OUTPUT = ROOT / "public/file-icons"
NAMES = ["document", "pdf", "spreadsheet", "presentation", "archive", "image", "audio", "video", "code", "generic"]


def silhouette(image):
    width, height = image.size
    pixels = image.load()
    # These duotone pages have colored (including blue-grey) silhouettes. Exclude
    # neutral studio shadows rather than cutting their white background into blobs.
    candidates = {(x, y) for y in range(height) for x in range(width)
                  if max(pixels[x, y]) - min(pixels[x, y]) > 18 or min(pixels[x, y]) < 110
                  or (y < height / 2 and min(pixels[x, y]) < 235)}
    largest = set()
    while candidates:
        origin = candidates.pop()
        component = {origin}
        queue = deque([origin])
        while queue:
            x, y = queue.popleft()
            for neighbor in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
                if neighbor in candidates:
                    candidates.remove(neighbor)
                    component.add(neighbor)
                    queue.append(neighbor)
        if len(component) > len(largest):
            largest = component
    mask = Image.new("L", image.size)
    alpha = mask.load()
    for pixel in largest:
        alpha[pixel] = 255
    # Only erase the connected exterior: white PDF lettering and pictograms stay opaque.
    ImageDraw.floodfill(mask, (0, 0), 128)
    return mask.point(lambda value: 0 if value == 128 else 255).filter(ImageFilter.GaussianBlur(0.5))


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    sheet = Image.open(SOURCE).convert("RGB")
    preview = Image.new("RGB", (1500, 820), "#e7ecf2")
    draw = ImageDraw.Draw(preview)
    label_font = ImageFont.truetype("C:/Windows/Fonts/segoeui.ttf", 22)
    for index, name in enumerate(NAMES):
        col, row = index % 5, index // 5
        cell = sheet.crop((round(col * sheet.width / 5), round(row * sheet.height / 2),
                           round((col + 1) * sheet.width / 5), round((row + 1) * sheet.height / 2)))
        mask = silhouette(cell)
        bounds = mask.getbbox()
        if not bounds:
            raise ValueError(f"No artwork found for {name}")
        rgba = cell.convert("RGBA")
        rgba.putalpha(mask)
        rgba = rgba.crop(bounds)
        rgba.thumbnail((232, 292), Image.Resampling.LANCZOS)
        icon = Image.new("RGBA", (256, 320))
        icon.alpha_composite(rgba, ((256 - rgba.width) // 2, (320 - rgba.height) // 2))
        icon.save(OUTPUT / f"{name}.webp", lossless=True, method=6)
        preview.paste(icon, (col * 300 + 22, row * 410 + 20), icon)
        draw.text((col * 300 + 150, row * 410 + 368), name, fill="#263346", font=label_font, anchor="mm")
    preview.save(SOURCE.parent / "duotone-transparent-preview.png")
    print(f"Prepared {len(NAMES)} transparent 256x320 icons in {OUTPUT}")


if __name__ == "__main__":
    main()
