"""Generate packaging icons from the simple BrowseFork branch mark."""

from pathlib import Path
from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parents[1]
ICONS = ROOT / "src-tauri" / "icons"
SCALE = 4
SIZE = 1024


def point(x, y):
    return (round(x * SCALE), round(y * SCALE))


image = Image.new("RGBA", (SIZE * SCALE, SIZE * SCALE), (0, 0, 0, 0))
draw = ImageDraw.Draw(image)
draw.rounded_rectangle(
    (point(48, 48), point(976, 976)), radius=224 * SCALE, fill="#4559d8"
)

stroke = 88 * SCALE
white = "#ffffff"
draw.line([point(512, 770), point(512, 470)], fill=white, width=stroke)
draw.line(
    [point(512, 500), point(504, 440), point(477, 386), point(429, 347), point(371, 326), point(304, 321)],
    fill=white,
    width=stroke,
    joint="curve",
)
draw.line(
    [point(512, 500), point(520, 440), point(547, 386), point(595, 347), point(653, 326), point(720, 321)],
    fill=white,
    width=stroke,
    joint="curve",
)
for x in (304, 720):
    draw.ellipse((point(x - 44, 277), point(x + 44, 365)), fill=white)
draw.line([point(340, 234), point(247, 321), point(340, 408)], fill=white, width=82 * SCALE, joint="curve")
draw.line([point(684, 234), point(777, 321), point(684, 408)], fill=white, width=82 * SCALE, joint="curve")

image = image.resize((SIZE, SIZE), Image.Resampling.LANCZOS)
ICONS.mkdir(parents=True, exist_ok=True)
image.save(ICONS / "icon.png")
image.save(ICONS / "icon.ico", sizes=[(16, 16), (32, 32), (48, 48), (256, 256)])
image.save(ICONS / "icon.icns", format="ICNS")
