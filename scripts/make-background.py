#!/usr/bin/env python3
"""Write the live desktop wallpaper."""

import struct
import zlib
from pathlib import Path

WIDTH = 1600
HEIGHT = 900


def pixel(x: int, y: int) -> bytes:
    nx = (x - WIDTH * 0.5) / WIDTH
    ny = (y - HEIGHT * 0.18) / HEIGHT
    glow = max(0.0, 1.0 - (nx * nx * 1.4 + ny * ny * 2.2) * 3.2)
    shade = y / HEIGHT
    red = int(9 + glow * 28 + shade * 4)
    green = int(13 + glow * 70 + shade * 6)
    blue = int(18 + glow * 48 + shade * 8)
    return bytes((min(red, 255), min(green, 255), min(blue, 255)))


def main() -> None:
    rows = bytearray()
    for y in range(HEIGHT):
        rows.append(0)
        for x in range(WIDTH):
            rows.extend(pixel(x, y))
    compressed = zlib.compress(bytes(rows), 9)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", compressed)
    png += chunk(b"IEND", b"")
    target = Path(__file__).resolve().parents[1] / "session" / "background.png"
    target.write_bytes(png)


if __name__ == "__main__":
    main()
