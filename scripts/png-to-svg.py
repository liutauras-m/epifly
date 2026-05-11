#!/usr/bin/env python3
"""
Convert a raster PNG into a clean, scalable SVG.

Uses VTracer (https://github.com/visioncortex/vtracer) — the current
state-of-the-art open-source raster→vector tracer (Rust core, Python bindings).
Produces small, layered SVGs with smooth Bézier curves and no JPEG-style noise.

Install:
    # vtracer 0.6.x has no wheel for Python 3.14 yet (segfault on arm64).
    # Use Python 3.13 or 3.12.
    python3.13 -m pip install vtracer pillow

Usage:
    python3.13 scripts/png-to-svg.py [input.png] [output.svg]

Defaults:
    input  = packages/ui/src/lib/assets/images/favicon.png
    output = packages/ui/src/lib/assets/images/favicon.svg
"""

from __future__ import annotations

import sys
from pathlib import Path

import vtracer
from PIL import Image

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_INPUT = REPO_ROOT / "packages" / "ui" / "src" / "lib" / "assets" / "images" / "favicon.png"
DEFAULT_OUTPUT = DEFAULT_INPUT.with_suffix(".svg")


# ---------------------------------------------------------------------------
# VTracer parameters tuned for clean logo / icon artwork
# ---------------------------------------------------------------------------
# Reference: https://www.visioncortex.org/vtracer-docs
TRACE_OPTS: dict = {
    # Colour quantisation
    "colormode":       "color",     # "color" | "binary"
    "color_precision": 6,           # 1–8 ; higher = more colours retained
    "layer_difference": 16,         # min colour delta between stacked layers

    # Hierarchy
    "hierarchical":    "stacked",   # "stacked" (filled) | "cutout"
    "mode":            "spline",    # "spline" (Bezier) | "polygon" | "none"

    # Curve fitting
    "filter_speckle":  4,           # discard regions smaller than N px
    "corner_threshold": 60,         # degrees; >angle ⇒ corner, else smooth
    "length_threshold": 4.0,        # min segment length before splitting
    "splice_threshold": 45,         # degrees for splicing curves
    "max_iterations":  10,          # curve-fit refinement passes
    "path_precision":  3,           # decimals kept in path data
}


def _display(path: Path) -> str:
    """Path string relative to repo root when possible, else absolute."""
    path = path.resolve()
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def trace(input_path: Path, output_path: Path) -> None:
    if not input_path.exists():
        sys.exit(f"Error: input not found: {input_path}")

    # Validate that the file is actually a readable image
    with Image.open(input_path) as im:
        w, h = im.size
        print(f"Input  : {_display(input_path)}  ({w}×{h}, {im.mode})")

    output_path.parent.mkdir(parents=True, exist_ok=True)

    vtracer.convert_image_to_svg_py(
        str(input_path),
        str(output_path),
        **TRACE_OPTS,
    )

    size_kb = output_path.stat().st_size / 1024
    print(f"Output : {_display(output_path)}  ({size_kb:.1f} KB)")
    print("Done ✓")


def main() -> None:
    args = sys.argv[1:]
    in_path = Path(args[0]).resolve() if len(args) >= 1 else DEFAULT_INPUT
    out_path = Path(args[1]).resolve() if len(args) >= 2 else in_path.with_suffix(".svg")
    trace(in_path, out_path)


if __name__ == "__main__":
    main()
