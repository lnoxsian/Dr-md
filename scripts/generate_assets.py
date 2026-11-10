#!/usr/bin/env python3
"""
Asset generation script for dr.md (Doctor Markdown).
Renders application icon sizes and logo assets using python3-pillow.
Extracts and composites vector parameters directly from assets/icons/dr-md.svg.
"""

import os
import sys
import io
import re
import math
import base64
import argparse
from PIL import Image, ImageDraw


def parse_and_render_svg_master(svg_path: str) -> Image.Image:
    """
    Renders a 512x512 master RGBA icon from dr-md.svg using pure Pillow.
    Extracts the embedded base64 artwork, applies the radial gradient background,
    positions the scaled art, and clips to the squircle rounded-rectangle mask.
    """
    with open(svg_path, "r", encoding="utf-8") as f:
        svg_content = f.read()

    # 1. Extract base64 embedded PNG
    match = re.search(r"data:image/png;base64,([A-Za-z0-9+/=]+)", svg_content)
    if not match:
        raise ValueError(f"No base64 embedded PNG found in {svg_path}")

    raw_png = base64.b64decode(match.group(1))
    art_img = Image.open(io.BytesIO(raw_png)).convert("RGBA")

    # 2. Dimensions & parameters
    width, height = 512, 512

    # Corner radius rx
    rx = 132
    rx_match = re.search(r'rx=["\']([0-9]+)["\']', svg_content)
    if rx_match:
        rx = int(rx_match.group(1))

    # Inner artwork scale and offset
    sub_w, sub_h, sub_x, sub_y = 700, 700, -94, -60
    sub_match = re.search(
        r'<svg[^>]+width=["\']([0-9]+)["\'][^>]+height=["\']([0-9]+)["\'][^>]+x=["\'](-?[0-9]+)["\'][^>]+y=["\'](-?[0-9]+)["\']',
        svg_content,
    )
    if sub_match:
        sub_w, sub_h, sub_x, sub_y = map(int, sub_match.groups())

    # 3. Render radial gradient background: #4d4d4d (center-top) to #000000 (edges)
    bg = Image.new("RGBA", (width, height), (0, 0, 0, 255))
    bg_pixels = bg.load()

    fx, fy = width / 2.0, 0.0  # cx=50%, fy=0%
    max_r = float(max(width, height))
    c_start = (77, 77, 77)  # #4d4d4d
    c_end = (0, 0, 0)       # #000000

    for y in range(height):
        dy_sq = (y - fy) ** 2
        for x in range(width):
            dist = math.sqrt((x - fx) ** 2 + dy_sq)
            t = min(1.0, dist / max_r)
            r = int(c_start[0] + (c_end[0] - c_start[0]) * t)
            g = int(c_start[1] + (c_end[1] - c_start[1]) * t)
            b = int(c_start[2] + (c_end[2] - c_start[2]) * t)
            bg_pixels[x, y] = (r, g, b, 255)

    # 4. Scale and paste artwork
    art_resized = art_img.resize((sub_w, sub_h), Image.Resampling.LANCZOS)
    bg.paste(art_resized, (sub_x, sub_y), art_resized)

    # 5. Anti-aliased rounded rectangle mask using 4x supersampling
    scale = 4
    mask_large = Image.new("L", (width * scale, height * scale), 0)
    draw = ImageDraw.Draw(mask_large)
    draw.rounded_rectangle(
        [0, 0, width * scale, height * scale],
        radius=rx * scale,
        fill=255,
    )
    mask = mask_large.resize((width, height), Image.Resampling.LANCZOS)

    # Composite final 512x512 master icon with transparent squircle corners
    master_icon = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    master_icon.paste(bg, (0, 0), mask)

    return master_icon


def generate_icons():
    source_svg = "assets/icons/dr-md.svg"
    output_dir = "assets/icons"

    if not os.path.exists(source_svg):
        print(f"Error: Source SVG not found at {source_svg}")
        return

    os.makedirs(output_dir, exist_ok=True)

    try:
        print(f"Rendering master icon from {source_svg}...")
        master_img = parse_and_render_svg_master(source_svg)

        # Save main 512x512 master PNG
        master_path = os.path.join(output_dir, "dr-md.png")
        master_img.save(master_path, "PNG", optimize=True)
        print(f"Generated {master_path}")

        # Generate each icon resolution
        sizes = [16, 32, 48, 64, 128, 256, 512]
        for size in sizes:
            resized_img = master_img.resize((size, size), Image.Resampling.LANCZOS)
            out_path = os.path.join(output_dir, f"dr-md_{size}x{size}.png")
            resized_img.save(out_path, "PNG", optimize=True)
            print(f"Generated {out_path}")

        # Multi-resolution ICO for Windows
        ico_path = os.path.join(output_dir, "dr-md.ico")
        master_img.save(
            ico_path,
            format="ICO",
            sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
        )
        print(f"Generated {ico_path}")

    except Exception as e:
        print(f"Error generating icons: {e}", file=sys.stderr)
        raise e


def generate_logo():
    source_paths = ["assets/logo/dr-md-logo.png", "assets/logo/dr-md-inapp.png"]
    source_path = None
    for path in source_paths:
        if os.path.exists(path):
            source_path = path
            break

    output_dir = "assets/logo"
    os.makedirs(output_dir, exist_ok=True)

    try:
        if source_path:
            img = Image.open(source_path).convert("RGBA")
            print(f"Loaded source logo from {source_path}")
        elif os.path.exists("assets/icons/dr-md.svg"):
            # Extract artwork directly from SVG if no logo exists
            with open("assets/icons/dr-md.svg", "r", encoding="utf-8") as f:
                content = f.read()
            match = re.search(r"data:image/png;base64,([A-Za-z0-9+/=]+)", content)
            if match:
                raw_png = base64.b64decode(match.group(1))
                img = Image.open(io.BytesIO(raw_png)).convert("RGBA")
                print("Loaded logo artwork from assets/icons/dr-md.svg")
            else:
                print("Error: Could not find or extract source logo.")
                return
        else:
            print("Error: No source logo found.")
            return

        # Optimize size: 256x256 for in-app welcome screen
        optimized_size = (256, 256)
        img_optimized = img.resize(optimized_size, Image.Resampling.LANCZOS)

        # Dark mode version
        dark_logo_path = os.path.join(output_dir, "dr-md-inapp-dark.png")
        img_optimized.save(dark_logo_path, "PNG", optimize=True)
        print(f"Generated {dark_logo_path}")

        # Light mode version: invert color channels while preserving alpha
        r, g, b, a = img_optimized.split()
        r_inverted = r.point(lambda p: 255 - p)
        g_inverted = g.point(lambda p: 255 - p)
        b_inverted = b.point(lambda p: 255 - p)
        img_inverted = Image.merge("RGBA", (r_inverted, g_inverted, b_inverted, a))

        light_logo_path = os.path.join(output_dir, "dr-md-inapp-light.png")
        img_inverted.save(light_logo_path, "PNG", optimize=True)
        print(f"Generated {light_logo_path}")

    except Exception as e:
        print(f"Error processing logo: {e}", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description="Generate app logo and icon assets.")
    parser.add_argument(
        "--type",
        choices=["all", "icons", "logo"],
        default="all",
        help="Asset type to generate (default: all)",
    )
    args = parser.parse_args()

    # Move to repository root if executed from scripts/
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    os.chdir(repo_root)

    if args.type in ("all", "icons"):
        print("Generating application icons...")
        generate_icons()

    if args.type in ("all", "logo"):
        print("\nGenerating application logos...")
        generate_logo()


if __name__ == "__main__":
    main()
