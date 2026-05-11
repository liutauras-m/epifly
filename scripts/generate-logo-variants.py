#!/usr/bin/env python3

from __future__ import annotations

import argparse
import csv
import colorsys
import re
from dataclasses import dataclass
from pathlib import Path


SOURCE_PRIMARY = "#FC5C03"
HEX_COLOR_RE = re.compile(r'fill="(#(?:[0-9A-Fa-f]{6}))"')
MARKDOWN_COLOR_RE = re.compile(r"^-\s+[^:]+:\s*(?P<name>.+?)\s+(?P<hex>#[0-9A-Fa-f]{6})\s*$")
HEX_RE = re.compile(r"#[0-9A-Fa-f]{6}")


@dataclass(frozen=True)
class Variant:
    slug: str
    hex_color: str
    source: str


EXTRA_VARIANTS = (
    Variant("ember", "#FF7A45", "supporting brand warm"),
    Variant("solar-gold", "#FFB547", "supporting brand energy"),
    Variant("lagoon-teal", "#0FA7A0", "supporting brand cool"),
    Variant("midnight-navy", "#0F172A", "supporting brand depth"),
)


def hex_to_rgb(value: str) -> tuple[float, float, float]:
    value = value.lstrip("#")
    return tuple(int(value[index : index + 2], 16) / 255 for index in (0, 2, 4))


def rgb_to_hex(rgb: tuple[float, float, float]) -> str:
    return "#" + "".join(f"{round(max(0, min(1, channel)) * 255):02X}" for channel in rgb)


def circular_offset(source_hue: float, anchor_hue: float) -> float:
    diff = source_hue - anchor_hue
    if diff > 0.5:
        diff -= 1
    if diff < -0.5:
        diff += 1
    return diff


def clamp(value: float, low: float = 0.0, high: float = 1.0) -> float:
    return max(low, min(high, value))


def is_neutral_fill(hex_color: str) -> bool:
    red, green, blue = hex_to_rgb(hex_color)
    hue, lightness, saturation = colorsys.rgb_to_hls(red, green, blue)
    del hue
    return saturation < 0.18 or lightness > 0.9


def map_fill(hex_color: str, target_hex: str, source_anchor_hex: str) -> str:
    if is_neutral_fill(hex_color):
        return hex_color.upper()

    source_rgb = hex_to_rgb(hex_color)
    source_hue, source_lightness, source_saturation = colorsys.rgb_to_hls(*source_rgb)

    anchor_rgb = hex_to_rgb(source_anchor_hex)
    anchor_hue, anchor_lightness, anchor_saturation = colorsys.rgb_to_hls(*anchor_rgb)

    target_rgb = hex_to_rgb(target_hex)
    target_hue, target_lightness, target_saturation = colorsys.rgb_to_hls(*target_rgb)

    hue = (target_hue + circular_offset(source_hue, anchor_hue) * 0.35) % 1.0
    lightness_delta = source_lightness - anchor_lightness
    lightness = clamp(target_lightness + lightness_delta * 0.92, 0.08, 0.92)

    if anchor_saturation == 0:
        saturation_ratio = 1.0
    else:
        saturation_ratio = source_saturation / anchor_saturation

    saturation = clamp(target_saturation * (0.7 + saturation_ratio * 0.45), 0.0, 1.0)

    if target_saturation < 0.08:
        hue = target_hue
        saturation = clamp(target_saturation + source_saturation * 0.08, 0.0, 0.14)

    return rgb_to_hex(colorsys.hls_to_rgb(hue, lightness, saturation))


def parse_branding_variants(branding_path: Path) -> list[Variant]:
    text = branding_path.read_text(encoding="utf-8")
    variants = parse_csv_variants(text)
    if not variants:
        variants = parse_markdown_variants(text)
    if not variants:
        raise ValueError(f"Could not extract brand colors from {branding_path}")
    return variants + list(EXTRA_VARIANTS)


def parse_csv_variants(text: str) -> list[Variant]:
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    if not lines:
        return []
    header = lines[0].lower().replace(" ", "")
    if header != "role,color,hex,usage":
        return []

    reader = csv.DictReader(lines)
    variants = []
    for row in reader:
        color_name = slugify(row["Color"])
        variants.append(
            Variant(
                slug=color_name,
                hex_color=row["Hex"].strip().upper(),
                source=f"branding:{row['Role'].strip()}",
            )
        )
    return variants


def parse_markdown_variants(text: str) -> list[Variant]:
    variants = []
    for line in text.splitlines():
        if len(HEX_RE.findall(line)) != 1:
            continue
        match = MARKDOWN_COLOR_RE.match(line.strip())
        if not match:
            continue
        color_name = slugify(match.group("name"))
        variants.append(
            Variant(
                slug=color_name,
                hex_color=match.group("hex").upper(),
                source="branding:markdown",
            )
        )
    return dedupe_variants(variants)


def dedupe_variants(variants: list[Variant]) -> list[Variant]:
    seen: set[str] = set()
    deduped: list[Variant] = []
    for variant in variants:
        if variant.slug in seen:
            continue
        seen.add(variant.slug)
        deduped.append(variant)
    return deduped


def slugify(value: str) -> str:
    normalized = re.sub(r"[^a-z0-9]+", "-", value.strip().lower())
    return normalized.strip("-")


def write_variant(svg_text: str, variant: Variant, output_dir: Path) -> Path:
    replacements: dict[str, str] = {}

    def replace_fill(match: re.Match[str]) -> str:
        source_fill = match.group(1).upper()
        if source_fill not in replacements:
            replacements[source_fill] = map_fill(source_fill, variant.hex_color, SOURCE_PRIMARY)
        return f'fill="{replacements[source_fill]}"'

    updated_svg = HEX_COLOR_RE.sub(replace_fill, svg_text)
    output_path = output_dir / f"logo-{variant.slug}.svg"
    output_path.write_text(updated_svg, encoding="utf-8")
    return output_path


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Generate branded SVG color variants from logo.svg.")
    parser.add_argument("source_svg", type=Path, help="Path to the source logo.svg file")
    parser.add_argument("branding_csv", type=Path, help="Path to branding.md CSV file")
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=None,
        help="Directory where logo-<variant>.svg files will be written",
    )
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    source_svg = args.source_svg.resolve()
    branding_csv = args.branding_csv.resolve()
    output_dir = args.output_dir.resolve() if args.output_dir else source_svg.parent

    svg_text = source_svg.read_text(encoding="utf-8")
    variants = parse_branding_variants(branding_csv)

    for variant in variants:
        output_path = write_variant(svg_text, variant, output_dir)
        print(f"wrote {output_path.name} [{variant.hex_color}] from {variant.source}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())