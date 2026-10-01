# Rust Font Rendering Options

Research into font handling for a Rust music engraver whose exported images should not depend on fonts installed on the viewing machine. No particular font asset is recommended; repository font samples are exploratory.

## Options

### Embed font data in SVG

The renderer can place a TTF/OTF in an SVG `@font-face` rule, for example as a data URI. This makes the font bytes available to SVG consumers and preserves selectable text, but does not fix the consumer's shaping, layout, or rasterization implementation. Embedding therefore does not guarantee identical output between SVG viewers. It also increases file size and requires checking redistribution terms for the selected font. Sources: [CSS Fonts Level 4: `@font-face`](https://www.w3.org/TR/css-fonts-4/#font-face-rule), [MDN: `@font-face`](https://developer.mozilla.org/en-US/docs/Web/CSS/@font-face).

### Convert glyphs to SVG paths

The renderer can shape and position text in Rust, then emit each glyph as vector paths. The exported SVG no longer depends on fonts or text layout on the viewing machine, though viewers may still rasterize paths differently. Outlined text is no longer selectable/searchable as text and can increase SVG size; repeated outlines can be shared using SVG definitions and references. Font-derived outlines still require checking the font license and its embedding/derivative terms.

Relevant Rust building blocks have distinct roles:

- [`rustybuzz`](https://docs.rs/rustybuzz/latest/rustybuzz/) shapes text into glyph IDs and positions.
- [`ttf-parser`](https://docs.rs/ttf-parser/latest/ttf_parser/) reads font data, metrics, and glyph outlines.
- [`ab_glyph`](https://docs.rs/ab_glyph/latest/ab_glyph/) provides glyph positioning, outlines, and rasterization APIs.
- [`cosmic-text`](https://docs.rs/cosmic-text/latest/cosmic_text/) provides higher-level text shaping and layout.

For precise engraving placement, shaping/layout and outline extraction can remain separate: the layout decides glyph IDs and positions; the renderer emits the corresponding outlines at those positions.

### Bundle font data and rasterize PNG

A renderer can use bundled font data to produce PNG pixels directly. [`fontdue`](https://docs.rs/fontdue/latest/fontdue/) provides glyph rasterization and metrics, but rasterizing glyphs is not itself full text shaping/layout. For whole-score output, Rust can instead generate SVG geometry and rasterize it through a pinned [`resvg`](https://docs.rs/resvg/latest/resvg/) pipeline. Pinning the renderer version and render dimensions/settings gives a controlled PNG path; the output is raster and resolution-specific.

### Discover system fonts

[`font-kit`](https://docs.rs/font-kit/latest/font_kit/) supports font discovery/loading, including system fonts. This is useful when host fonts are an explicit feature, but is a poor default for portable output because installations and font versions vary.

## Recommendation

Use **SVG paths as the primary output**: bundle or otherwise load font data only while generating the score, shape and position text in Rust, and emit glyph outlines instead of relying on SVG viewer text layout. This best meets portability and stable appearance across machines while keeping the central engraving model vector-based. Add optional PNG export by rasterizing the generated geometry through a pinned `resvg` version and explicit output dimensions.

This does not promise pixel-identical rasterization across different SVG consumers. For byte/pixel reproducibility, define the PNG renderer/version and its render parameters as part of the product contract. If retaining selectable SVG text becomes more important than cross-viewer consistency, embedded fonts remain an alternative, but the portability guarantee should be stated as font-data availability rather than identical rendering.

## Sources

- [CSS Fonts Module Level 4: Font loading](https://www.w3.org/TR/css-fonts-4/#font-face-rule)
- [SVG 2: Text](https://www.w3.org/TR/SVG2/text.html)
- [MDN: `@font-face`](https://developer.mozilla.org/en-US/docs/Web/CSS/@font-face)
- [resvg repository: SVG feature support](https://github.com/linebender/resvg)
- [SIL Open Font License](https://openfontlicense.org/) (an example of a font license; not a recommendation for an asset)
- Rust crate documentation linked above for `rustybuzz`, `ttf-parser`, `ab_glyph`, `cosmic-text`, `fontdue`, `resvg`, and `font-kit`.
