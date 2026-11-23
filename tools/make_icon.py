"""Draws Formula's application icon (azulene drawn in white on a blue tile) and saves it as a
PNG for the window, an ICO for Windows and a 1024-pixel PNG for the Mac app icon (macOS icons
leave a margin around the tile).

    python3 app/tools/make_icon.py
"""
import math
import os

from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
APP = os.path.dirname(HERE)
S = 1024  # drawn large, then scaled down smoothly


def draw(pad=40, radius=200):
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    # Blue tile, a little lighter at the top.
    tile = Image.new("RGBA", (S, S))
    for y in range(S):
        f = y / S
        tile.paste((int(56 - 24 * f), int(124 - 30 * f), int(228 - 30 * f), 255), (0, y, S, y + 1))
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle([pad, pad, S - pad, S - pad], radius=radius, fill=255)
    img.paste(tile, (0, 0), mask)
    # Azulene: a seven-membered ring fused to a five-membered ring along a vertical bond,
    # with five double bonds drawn as shorter inner lines (as in a structure drawing).
    def ring(n, cx, start, step):
        r = 1 / (2 * math.sin(math.pi / n))  # circumradius for bonds of length 1
        return [(cx + r * math.cos(start + step * k), r * math.sin(start + step * k)) for k in range(n)]

    a7 = 1 / (2 * math.tan(math.pi / 7))
    a5 = 1 / (2 * math.tan(math.pi / 5))
    # Seven-ring: from the upper shared atom, anticlockwise; five-ring: from the upper shared
    # atom, clockwise (y up here, flipped below).
    seven = ring(7, -a7, math.pi / 7, 2 * math.pi / 7)
    five = ring(5, a5, 4 * math.pi / 5, -2 * math.pi / 5)
    c7, c5 = (-a7, 0.0), (a5, 0.0)
    bonds = []  # (start, end, ring centre or None)
    for k in range(7):
        bonds.append((seven[k], seven[(k + 1) % 7], c7 if k in (1, 3, 5) else None))
    for k in range(4):  # the shared bond (five[4]-five[0]) is already in the seven-ring
        bonds.append((five[k], five[k + 1], c5 if k in (0, 2) else None))

    xs = [p[0] for p in seven + five]
    ys = [p[1] for p in seven + five]
    scale = 0.80 * (S - 2 * pad) / (max(xs) - min(xs))
    ox = S / 2 - scale * (max(xs) + min(xs)) / 2
    oy = S / 2 + scale * (max(ys) + min(ys)) / 2

    def px(p):
        return (ox + scale * p[0], oy - scale * p[1])

    w = 0.145 * scale

    def stroke(a, b):
        a, b = px(a), px(b)
        d.line([a, b], fill="white", width=round(w))
        for q in (a, b):
            d.ellipse([q[0] - w / 2, q[1] - w / 2, q[0] + w / 2, q[1] + w / 2], fill="white")

    for a, b, centre in bonds:
        stroke(a, b)
        if centre:
            # Inner line: moved towards the ring centre and trimmed at both ends.
            mx, my = (a[0] + b[0]) / 2, (a[1] + b[1]) / 2
            nx, ny = centre[0] - mx, centre[1] - my
            nl = math.hypot(nx, ny)
            off = 0.27
            sx, sy = nx / nl * off, ny / nl * off
            t = 0.20
            ia = (a[0] + (b[0] - a[0]) * t + sx, a[1] + (b[1] - a[1]) * t + sy)
            ib = (b[0] + (a[0] - b[0]) * t + sx, b[1] + (a[1] - b[1]) * t + sy)
            stroke(ia, ib)
    return img


def main():
    big = draw()
    png = big.resize((256, 256), Image.LANCZOS)
    os.makedirs(os.path.join(APP, "assets"), exist_ok=True)
    png.save(os.path.join(APP, "assets", "icon.png"))
    sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    big.resize((256, 256), Image.LANCZOS).save(os.path.join(APP, "installer", "windows", "formula.ico"), sizes=sizes)
    os.makedirs(os.path.join(APP, "installer", "macos"), exist_ok=True)
    draw(pad=100, radius=185).save(os.path.join(APP, "installer", "macos", "icon-1024.png"))
    print("saved assets/icon.png, installer/windows/formula.ico and installer/macos/icon-1024.png")


if __name__ == "__main__":
    main()
