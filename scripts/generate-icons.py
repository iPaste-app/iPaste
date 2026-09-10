"""Regenerate desktop icons from assets/branding. Requires Pillow: py -m pip install Pillow."""

from pathlib import Path
import struct

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
ICONS = ROOT / "src-tauri" / "icons"


def load_artwork(name: str) -> Image.Image:
    artwork = Image.open(ROOT / "assets" / "branding" / name).convert("RGBA")
    # Remove faint extraction debris while preserving the opaque artwork and antialiasing.
    alpha = artwork.getchannel("A").point(lambda value: 0 if value <= 32 else value)
    artwork.putalpha(alpha)
    return artwork.crop(alpha.getbbox())


def fit_icon(artwork: Image.Image, size: int, coverage: float) -> Image.Image:
    fitted = artwork.copy()
    extent = round(size * coverage)
    fitted.thumbnail((extent, extent), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (size, size))
    canvas.alpha_composite(fitted, ((size - fitted.width) // 2, (size - fitted.height) // 2))
    return canvas


def main() -> None:
    app = load_artwork("app-source.png")
    tray = load_artwork("tray-source.png")
    # Keep the filenames and dimensions consumed by the existing desktop configuration.
    for path in sorted(ICONS.glob("*.png")):
        with Image.open(path) as existing:
            size = existing.width
        is_tray = path.name.startswith("tray-icon")
        fit_icon(tray if is_tray else app, size, 0.94 if is_tray else 0.90).save(path)

    master = fit_icon(app, 1024, 0.90)
    master.save(ICONS / "icon.png")
    master.save(ICONS / "icon.ico", sizes=[(n, n) for n in (16, 20, 24, 32, 40, 48, 64, 128, 256)])
    # Tauri decodes the first ICO entry for native window title bars. Pillow writes
    # the smallest first; reorder directory entries without changing payload offsets.
    ico_path = ICONS / "icon.ico"
    data = ico_path.read_bytes()
    count = struct.unpack_from("<H", data, 4)[0]
    entries = [data[6 + i * 16 : 6 + (i + 1) * 16] for i in range(count)]
    entries.sort(key=lambda entry: (entry[0] or 256) * (entry[1] or 256), reverse=True)
    ico_path.write_bytes(data[:6] + b"".join(entries) + data[6 + count * 16 :])
    master.save(ICONS / "icon.icns")
    print("Generated desktop PNG, ICO, ICNS and tray icons.")


if __name__ == "__main__":
    main()
