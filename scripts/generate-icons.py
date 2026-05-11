#!/usr/bin/env python3
"""
Generate all Tauri app icons (desktop, Windows, Android, iOS) from a source PNG.

Requirements:
    pip install Pillow
    macOS only: iconutil (built-in) is used for .icns generation

Usage:
    python scripts/generate-icons.py [source_image]

    Defaults to packages/ui/src/lib/assets/images/favicon.png
"""

import os
import sys
import struct
import shutil
import subprocess
import tempfile
from pathlib import Path
from PIL import Image

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
REPO_ROOT = Path(__file__).resolve().parent.parent
ICONS_DIR = REPO_ROOT / "apps" / "browser-shell" / "src-tauri" / "icons"
APP_ICON_PATH = REPO_ROOT / "apps" / "browser-shell" / "app-icon.png"
DEFAULT_SOURCE = REPO_ROOT / "packages" / "ui" / "src" / "lib" / "assets" / "images" / "favicon.png"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def open_source(path: Path) -> Image.Image:
    img = Image.open(path).convert("RGBA")
    print(f"Source: {path}  ({img.width}×{img.height})")
    return img


def resize(img: Image.Image, size: int) -> Image.Image:
    return img.resize((size, size), Image.LANCZOS)


def save(img: Image.Image, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    img.save(dest, "PNG", optimize=True)
    print(f"  wrote {dest.relative_to(REPO_ROOT)}")


def flat_on_white(img: Image.Image, size: int) -> Image.Image:
    """Resize and composite on solid white (required for iOS / ICO)."""
    resized = resize(img, size)
    bg = Image.new("RGBA", (size, size), (255, 255, 255, 255))
    bg.paste(resized, mask=resized.split()[3])
    return bg.convert("RGB")


def android_foreground(img: Image.Image, total_dp: int) -> Image.Image:
    """
    Android adaptive-icon foreground: icon centred in total_dp canvas
    with ~33% padding on each side so the safe zone (66%) fills nicely.
    Canvas is transparent.
    """
    canvas = Image.new("RGBA", (total_dp, total_dp), (0, 0, 0, 0))
    # safe zone = 66 % of total; icon fills safe zone
    icon_size = round(total_dp * 0.66)
    icon = resize(img, icon_size)
    offset = (total_dp - icon_size) // 2
    canvas.paste(icon, (offset, offset), mask=icon.split()[3])
    return canvas


# ---------------------------------------------------------------------------
# Desktop icons
# ---------------------------------------------------------------------------

def generate_app_icon(img: Image.Image) -> None:
    """1024×1024 source icon used by Tauri (apps/browser-shell/app-icon.png)."""
    print("\n── app-icon.png ─────────────────────────────────────────")
    icon = flat_on_white(img, 1024)
    icon.save(APP_ICON_PATH, "PNG", optimize=True)
    print(f"  wrote {APP_ICON_PATH.relative_to(REPO_ROOT)}")


def generate_desktop(img: Image.Image) -> None:
    print("\n── Desktop ──────────────────────────────────────────────")
    specs = [
        ("32x32.png",       32,  False),
        ("64x64.png",       64,  False),
        ("128x128.png",     128, False),
        ("128x128@2x.png",  256, False),
        ("icon.png",        512, False),
    ]
    for filename, size, _ in specs:
        save(resize(img, size), ICONS_DIR / filename)


# ---------------------------------------------------------------------------
# Windows Store logos
# ---------------------------------------------------------------------------

def generate_windows(img: Image.Image) -> None:
    print("\n── Windows Store ────────────────────────────────────────")
    specs = [
        ("Square30x30Logo.png",  30),
        ("Square44x44Logo.png",  44),
        ("Square71x71Logo.png",  71),
        ("Square89x89Logo.png",  89),
        ("Square107x107Logo.png", 107),
        ("Square142x142Logo.png", 142),
        ("Square150x150Logo.png", 150),
        ("Square284x284Logo.png", 284),
        ("Square310x310Logo.png", 310),
        ("StoreLogo.png",        50),
    ]
    for filename, size in specs:
        save(flat_on_white(img, size), ICONS_DIR / filename)


# ---------------------------------------------------------------------------
# ICO (multi-size)
# ---------------------------------------------------------------------------

def generate_ico(img: Image.Image) -> None:
    print("\n── ICO ──────────────────────────────────────────────────")
    dest = ICONS_DIR / "icon.ico"
    dest.parent.mkdir(parents=True, exist_ok=True)
    sizes = [(s, s) for s in (16, 24, 32, 48, 64, 128, 256)]
    frames = []
    for s, _ in sizes:
        frame = flat_on_white(img, s)
        frames.append(frame)
    # Pillow writes a proper multi-image ICO when append_images is given
    frames[0].save(
        dest,
        format="ICO",
        sizes=sizes,
        append_images=frames[1:],
    )
    print(f"  wrote {dest.relative_to(REPO_ROOT)}")


# ---------------------------------------------------------------------------
# ICNS (macOS) via iconutil
# ---------------------------------------------------------------------------

def generate_icns(img: Image.Image) -> None:
    print("\n── ICNS (macOS) ─────────────────────────────────────────")
    dest = ICONS_DIR / "icon.icns"

    # iconutil requires a specific iconset directory structure
    icns_specs = [
        ("icon_16x16.png",       16),
        ("icon_16x16@2x.png",    32),
        ("icon_32x32.png",       32),
        ("icon_32x32@2x.png",    64),
        ("icon_64x64.png",       64),
        ("icon_64x64@2x.png",    128),
        ("icon_128x128.png",     128),
        ("icon_128x128@2x.png",  256),
        ("icon_256x256.png",     256),
        ("icon_256x256@2x.png",  512),
        ("icon_512x512.png",     512),
        ("icon_512x512@2x.png",  1024),
    ]

    with tempfile.TemporaryDirectory(suffix=".iconset") as iconset_dir:
        for filename, size in icns_specs:
            frame = flat_on_white(img, size)
            frame.save(os.path.join(iconset_dir, filename))

        result = subprocess.run(
            ["iconutil", "-c", "icns", iconset_dir, "-o", str(dest)],
            capture_output=True, text=True
        )
        if result.returncode != 0:
            print(f"  [WARN] iconutil failed: {result.stderr.strip()}")
            print("         Falling back to Pillow ICNS writer …")
            # Pillow fallback (less complete but works cross-platform)
            largest = flat_on_white(img, 1024)
            largest.save(dest, format="ICNS")

    print(f"  wrote {dest.relative_to(REPO_ROOT)}")


# ---------------------------------------------------------------------------
# Android
# ---------------------------------------------------------------------------

ANDROID_DENSITIES = {
    "mipmap-mdpi":    {"launcher": 48,  "foreground": 108},
    "mipmap-hdpi":    {"launcher": 72,  "foreground": 162},
    "mipmap-xhdpi":   {"launcher": 96,  "foreground": 216},
    "mipmap-xxhdpi":  {"launcher": 144, "foreground": 324},
    "mipmap-xxxhdpi": {"launcher": 192, "foreground": 432},
}


def generate_android(img: Image.Image) -> None:
    print("\n── Android ──────────────────────────────────────────────")
    android_dir = ICONS_DIR / "android"

    for density, sizes in ANDROID_DENSITIES.items():
        dp = android_dir / density
        l_size = sizes["launcher"]
        fg_size = sizes["foreground"]

        # ic_launcher – flat (background colour comes from XML)
        launcher = flat_on_white(img, l_size)
        save(launcher, dp / "ic_launcher.png")

        # ic_launcher_round – same but circular mask
        round_img = make_round(flat_on_white(img, l_size))
        save(round_img, dp / "ic_launcher_round.png")

        # ic_launcher_foreground – transparent canvas, centred icon
        fg = android_foreground(img, fg_size)
        save(fg, dp / "ic_launcher_foreground.png")

    # Adaptive-icon XML (already exists, but write it to be safe)
    anydpi_dir = android_dir / "mipmap-anydpi-v26"
    anydpi_dir.mkdir(parents=True, exist_ok=True)
    _write_text(anydpi_dir / "ic_launcher.xml", ADAPTIVE_ICON_XML)
    _write_text(anydpi_dir / "ic_launcher_round.xml", ADAPTIVE_ICON_XML)

    values_dir = android_dir / "values"
    values_dir.mkdir(parents=True, exist_ok=True)
    _write_text(values_dir / "ic_launcher_background.xml", BACKGROUND_COLOR_XML)


ADAPTIVE_ICON_XML = """\
<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
  <foreground android:drawable="@mipmap/ic_launcher_foreground"/>
  <background android:drawable="@color/ic_launcher_background"/>
</adaptive-icon>
"""

BACKGROUND_COLOR_XML = """\
<?xml version="1.0" encoding="utf-8"?>
<resources>
  <color name="ic_launcher_background">#ffffff</color>
</resources>
"""


def make_round(img: Image.Image) -> Image.Image:
    """Apply a circular mask to a square RGBA/RGB image."""
    size = img.size[0]
    img = img.convert("RGBA")
    mask = Image.new("L", (size, size), 0)
    from PIL import ImageDraw
    draw = ImageDraw.Draw(mask)
    draw.ellipse((0, 0, size - 1, size - 1), fill=255)
    result = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    result.paste(img, mask=mask)
    return result


def _write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    print(f"  wrote {path.relative_to(REPO_ROOT)}")


# ---------------------------------------------------------------------------
# iOS
# ---------------------------------------------------------------------------

IOS_SPECS = [
    # (filename, logical_size, scale)  → pixel = logical * scale
    ("AppIcon-20x20@1x.png",    20,  1),
    ("AppIcon-20x20@2x.png",    20,  2),
    ("AppIcon-20x20@2x-1.png",  20,  2),
    ("AppIcon-20x20@3x.png",    20,  3),
    ("AppIcon-29x29@1x.png",    29,  1),
    ("AppIcon-29x29@2x.png",    29,  2),
    ("AppIcon-29x29@2x-1.png",  29,  2),
    ("AppIcon-29x29@3x.png",    29,  3),
    ("AppIcon-40x40@1x.png",    40,  1),
    ("AppIcon-40x40@2x.png",    40,  2),
    ("AppIcon-40x40@2x-1.png",  40,  2),
    ("AppIcon-40x40@3x.png",    40,  3),
    ("AppIcon-60x60@2x.png",    60,  2),
    ("AppIcon-60x60@3x.png",    60,  3),
    ("AppIcon-76x76@1x.png",    76,  1),
    ("AppIcon-76x76@2x.png",    76,  2),
    ("AppIcon-83.5x83.5@2x.png", 84, 2),   # 83.5 rounded → 167 px
    ("AppIcon-512@2x.png",      512, 2),   # 1024 px – App Store
]


def generate_ios(img: Image.Image) -> None:
    print("\n── iOS ───────────────────────────────────────────────────")
    ios_dir = ICONS_DIR / "ios"
    for filename, logical, scale in IOS_SPECS:
        px = logical * scale
        # iOS icons must NOT have transparency; Apple rejects them otherwise
        icon = flat_on_white(img, px)
        save(icon, ios_dir / filename)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    source_path = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_SOURCE
    if not source_path.exists():
        print(f"Error: source image not found: {source_path}", file=sys.stderr)
        sys.exit(1)

    img = open_source(source_path)

    generate_app_icon(img)
    generate_desktop(img)
    generate_windows(img)
    generate_ico(img)
    generate_icns(img)
    generate_android(img)
    generate_ios(img)

    print("\nDone ✓")


if __name__ == "__main__":
    main()
