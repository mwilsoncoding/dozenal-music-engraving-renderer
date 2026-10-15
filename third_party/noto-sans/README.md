# Noto Sans Text Glyphs

`NotoSans-2.015-variable.ttf` is Noto Sans Version 2.015 from Google Fonts commit `2984c575fdce412ee02b2baaba67672b9a9434d8`. Its SHA-256 is `bfb7bb691513f12e734dc346c03a03f784912432d7e3fa8e56efcf906fe86b3d`. The accompanying `OFL.txt` is the upstream SIL Open Font License 1.1 notice.

`src/noto_sans_glyphs.rs` contains static SVG outlines and advances generated from the font at its default axes (`wght=400`, `wdth=100`). The dozenal `↊` outline is derived from `2` by a 180-degree rotation about its ink-bounds center; `↋` is derived from `3` by a horizontal flip about its ink-bounds center. Both preserve the source advance width.

To regenerate the table, install FontTools 4.56.0, then run:

```sh
python3 tools/generate_text_glyphs.py \
  --font third_party/noto-sans/NotoSans-2.015-variable.ttf \
  --output src/noto_sans_glyphs.rs
```

The renderer uses only the generated Rust table at runtime; it does not load fonts or depend on system font configuration.
