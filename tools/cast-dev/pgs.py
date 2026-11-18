#!/usr/bin/env python3
"""Writes a short PGS (.sup) subtitle stream of drawn boxes, for the Cast test media.

    python3 tools/cast-dev/pgs.py out.sup [--width 1280] [--height 720]

ffmpeg has no PGS encoder, so this writes the segments itself: two captions, each a framed bar
with a row of blocks, shown at 1-5 s and 6-10 s. Nothing in it is text, so it needs no font.
"""

import argparse
import struct


def segment(kind: int, pts: float, body: bytes) -> bytes:
    ticks = int(pts * 90000)
    return b"PG" + struct.pack(">IIBH", ticks, ticks, kind, len(body)) + body


def rle(pixels: list[list[int]]) -> bytes:
    out = bytearray()
    for row in pixels:
        x = 0
        while x < len(row):
            color = row[x]
            n = 1
            while x + n < len(row) and row[x + n] == color and n < 16383:
                n += 1
            if color == 0:
                out += bytes([0, n]) if n < 64 else bytes([0, 0x40 | (n >> 8), n & 0xFF])
            elif n < 3:
                out += bytes([color]) * n
            elif n < 64:
                out += bytes([0, 0x80 | n, color])
            else:
                out += bytes([0, 0xC0 | (n >> 8), n & 0xFF, color])
            x += n
        out += b"\x00\x00"
    return bytes(out)


def caption(width: int, height: int, blocks: int) -> list[list[int]]:
    """A framed bar: 1 is the fill, 2 the frame, 3 the blocks."""
    pixels = [[1] * width for _ in range(height)]
    for y in range(height):
        for x in range(width):
            if x < 4 or y < 4 or x >= width - 4 or y >= height - 4:
                pixels[y][x] = 2
    size = height // 2
    gap = size // 2
    left = (width - blocks * size - (blocks - 1) * gap) // 2
    top = (height - size) // 2
    for b in range(blocks):
        for y in range(top, top + size):
            for x in range(left + b * (size + gap), left + b * (size + gap) + size):
                pixels[y][x] = 3
    return pixels


def display_set(number: int, start: float, end: float, screen: tuple[int, int], blocks: int) -> bytes:
    sw, sh = screen
    w, h = 640, 72
    x, y = (sw - w) // 2, sh - h - 60
    pcs = struct.pack(">HHBHBBBB", sw, sh, 0x10, number * 2, 0x80, 0, 0, 1)
    pcs += struct.pack(">HBBHH", 0, 0, 0, x, y)
    wds = struct.pack(">BBHHHH", 1, 0, x, y, w, h)
    # YCrCb with alpha: 0 clear, 1 dark fill, 2 white frame, 3 yellow blocks.
    pds = struct.pack(">BB", 0, 0)
    for entry in ((0, 16, 128, 128, 0), (1, 32, 128, 128, 200), (2, 235, 128, 128, 255), (3, 210, 146, 16, 255)):
        pds += bytes(entry)
    data = rle(caption(w, h, blocks))
    ods = struct.pack(">HBB", 0, 0, 0xC0) + (len(data) + 4).to_bytes(3, "big") + struct.pack(">HH", w, h) + data
    shown = (
        segment(0x16, start, pcs)
        + segment(0x17, start, wds)
        + segment(0x14, start, pds)
        + segment(0x15, start, ods)
        + segment(0x80, start, b"")
    )
    clear_pcs = struct.pack(">HHBHBBBB", sw, sh, 0x10, number * 2 + 1, 0x00, 0, 0, 0)
    cleared = segment(0x16, end, clear_pcs) + segment(0x17, end, wds) + segment(0x80, end, b"")
    return shown + cleared


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("out")
    parser.add_argument("--width", type=int, default=1280)
    parser.add_argument("--height", type=int, default=720)
    args = parser.parse_args()
    screen = (args.width, args.height)
    with open(args.out, "wb") as f:
        f.write(display_set(0, 1.0, 5.0, screen, 3))
        f.write(display_set(1, 6.0, 10.0, screen, 5))


if __name__ == "__main__":
    main()
