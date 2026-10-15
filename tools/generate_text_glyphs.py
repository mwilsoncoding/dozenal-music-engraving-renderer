#!/usr/bin/env python3
"""Generate path-only Noto Sans glyph data for the supported text repertoire."""

import argparse
import json
import unicodedata
from pathlib import Path

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.recordingPen import DecomposingRecordingPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

SUPPORTED_CODEPOINTS = (
    list(range(0x20, 0x7F))
    + list(range(0xA0, 0x100))
    + list(range(0x100, 0x180))
    + [0x218A, 0x218B]
)


def number(value):
    result = f"{value:.4f}".rstrip("0").rstrip(".") or "0"
    return result if "." in result else f"{result}.0"


def rust_char(character):
    return f"'\\u{{{ord(character):04x}}}'"


def normalization_source():
    compositions = {}
    combining_marks = set()
    for codepoint in SUPPORTED_CODEPOINTS:
        character = chr(codepoint)
        decomposed = unicodedata.normalize("NFD", character)
        combining_marks.update(
            item for item in decomposed if unicodedata.combining(item)
        )
        definition = unicodedata.decomposition(character)
        if not definition or definition.startswith("<"):
            continue
        parts = [chr(int(part, 16)) for part in definition.split()]
        if len(parts) == 2 and unicodedata.normalize("NFC", "".join(parts)) == character:
            compositions[(parts[0], parts[1])] = character

    lines = [
        f"// Generated from Unicode {unicodedata.unidata_version} canonical composition data.",
        "pub(crate) fn normalize(value: &str) -> String {",
        "    let mut ordered = Vec::new();",
        "    for character in value.chars() {",
        "        let class = combining_class(character);",
        "        if class == 0 {",
        "            ordered.push(character);",
        "        } else {",
        "            let start = ordered",
        "                .iter()",
        "                .rposition(|item| combining_class(*item) == 0)",
        "                .map_or(0, |index| index + 1);",
        "            let insert = (start..ordered.len())",
        "                .find(|index| combining_class(ordered[*index]) > class)",
        "                .unwrap_or(ordered.len());",
        "            ordered.insert(insert, character);",
        "        }",
        "    }",
        "    let mut composed = Vec::new();",
        "    let mut starter = None;",
        "    let mut last_class = 0;",
        "    for character in ordered {",
        "        let class = combining_class(character);",
        "        if let Some(index) = starter {",
        "            if last_class == 0 || last_class < class {",
        "                if let Some(composite) = compose(composed[index], character) {",
        "                    composed[index] = composite;",
        "                    continue;",
        "                }",
        "            }",
        "        }",
        "        if class == 0 {",
        "            starter = Some(composed.len());",
        "        }",
        "        composed.push(character);",
        "        last_class = class;",
        "    }",
        "    composed.into_iter().collect()",
        "}",
        "",
        "fn compose(starter: char, mark: char) -> Option<char> {",
        "    match (starter, mark) {",
    ]
    for (base, mark), result in sorted(compositions.items(), key=lambda item: (ord(item[0][0]), ord(item[0][1]))):
        lines.append(f"        ({rust_char(base)}, {rust_char(mark)}) => Some({rust_char(result)}),")
    lines.extend(["        _ => None,", "    }", "}", "", "fn combining_class(character: char) -> u8 {", "    match character {"])
    for mark in sorted(combining_marks, key=ord):
        lines.append(f"        {rust_char(mark)} => {unicodedata.combining(mark)},")
    lines.extend(["        _ => 0,", "    }", "}", ""])
    return "\n".join(lines)


def transformed_glyph(glyph_set, glyph_name, transform):
    recording = DecomposingRecordingPen(glyph_set)
    glyph_set[glyph_name].draw(recording)
    path_pen = SVGPathPen(glyph_set)
    recording.replay(TransformPen(path_pen, transform))
    bounds_pen = BoundsPen(glyph_set)
    glyph_set[glyph_name].draw(TransformPen(bounds_pen, transform))
    return path_pen.getCommands(), bounds_pen.bounds or (0, 0, 0, 0)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--font", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--normalization-output", type=Path)
    args = parser.parse_args()

    font = TTFont(args.font)
    version = font["name"].getDebugName(5)
    if version != "Version 2.015":
        raise SystemExit(f"expected Noto Sans Version 2.015, got {version!r}")
    if "fvar" in font:
        axes = {axis.axisTag: axis.defaultValue for axis in font["fvar"].axes}
        instantiateVariableFont(font, axes, inplace=True)

    glyph_set = font.getGlyphSet()
    cmap = font.getBestCmap()
    metrics = font["hmtx"].metrics
    glyph_names = {}
    for codepoint in SUPPORTED_CODEPOINTS:
        glyph_name = cmap.get(codepoint)
        if glyph_name is not None:
            glyph_names[codepoint] = glyph_name
        elif codepoint not in (0x218A, 0x218B):
            raise SystemExit(f"Noto Sans is missing U+{codepoint:04X}")
    glyph_names[0x218A] = cmap[ord("2")]
    glyph_names[0x218B] = cmap[ord("3")]

    records = []
    for codepoint in SUPPORTED_CODEPOINTS:
        glyph_name = glyph_names[codepoint]
        bounds_pen = BoundsPen(glyph_set)
        glyph_set[glyph_name].draw(bounds_pen)
        source_bounds = bounds_pen.bounds or (0, 0, 0, 0)
        if codepoint == 0x218A:
            xmin, ymin, xmax, ymax = source_bounds
            transform = (-1, 0, 0, 1, xmin + xmax, -(ymin + ymax))
        elif codepoint == 0x218B:
            xmin, _, xmax, _ = source_bounds
            transform = (-1, 0, 0, -1, xmin + xmax, 0)
        else:
            transform = (1, 0, 0, -1, 0, 0)
        path, bounds = transformed_glyph(glyph_set, glyph_name, transform)
        advance = metrics[glyph_name][0]
        path_literal = json.dumps(path)
        bounds_literal = ", ".join(number(value) for value in bounds)
        records.append(
            f"        {rust_char(chr(codepoint))} => Some(Glyph {{ "
            f"path: {path_literal}, advance: {number(advance)}, bounds: [{bounds_literal}] }}),"
        )
    output = [
        "// Generated by tools/generate_text_glyphs.py from Noto Sans 2.015.",
        "// Source: Google Fonts commit 2984c575fdce412ee02b2baaba67672b9a9434d8.",
        "#[derive(Clone, Copy)]",
        "pub(crate) struct Glyph {",
        "    pub(crate) path: &'static str,",
        "    pub(crate) advance: f64,",
        "    pub(crate) bounds: [f64; 4],",
        "}",
        "pub(crate) const UNITS_PER_EM: f64 = 1000.0;",
        "pub(crate) fn glyph(character: char) -> Option<Glyph> {",
        "    match character {",
        *records,
        "        _ => None,",
        "    }",
        "}",
        "pub(crate) fn supports(character: char) -> bool {",
        "    glyph(character).is_some()",
        "}",
        "",
    ]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(output), encoding="utf-8")
    if args.normalization_output:
        args.normalization_output.parent.mkdir(parents=True, exist_ok=True)
        args.normalization_output.write_text(normalization_source(), encoding="utf-8")


if __name__ == "__main__":
    main()
