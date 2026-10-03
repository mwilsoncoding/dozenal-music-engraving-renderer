# Research: Redistributable Dozenal-Compatible Font Assets

Comparison of font assets that could be bundled by a self-contained music renderer. This is a shortlist, not a font selection. Assets in `mwilsoncoding/dozenal-music-notation` were excluded from consideration.

## Findings

| Candidate | Version and source | U+0030–U+0039 | U+218A / U+218B | License and tradeoffs |
| --- | --- | --- | --- | --- |
| GNU Unifont | 18.0.01; official GNU FTP [OTF](https://ftp.gnu.org/gnu/unifont/unifont-18.0.01/unifont-18.0.01.otf) and [source archive](https://ftp.gnu.org/gnu/unifont/unifont-18.0.01/unifont-18.0.01.tar.gz) | Verified: all ten digits | Verified: both present | The release's `COPYING` offers the font under OFL 1.1 or GPL-2-or-later with a font-embedding exception. OFL supports redistribution with software and modified fonts subject to its conditions. Unifont is designed as a 16-pixel bitmap font, so its pixel-grid appearance is a poor default for polished engraving; inspect the actual outlines at target sizes before considering it. |
| Noto Sans Symbols 2 Regular | Repository commit [`ffebf8c1ee449e544955a7e813c54f9b73848eac`](https://github.com/notofonts/noto-fonts/tree/ffebf8c1ee449e544955a7e813c54f9b73848eac); [TTF](https://raw.githubusercontent.com/notofonts/noto-fonts/ffebf8c1ee449e544955a7e813c54f9b73848eac/unhinted/ttf/NotoSansSymbols2/NotoSansSymbols2-Regular.ttf) | Verified: all ten digits | Verified absent: both | OFL 1.1. It provides a vector font with a broad symbol repertoire, but is not a native tone-glyph solution. It could only be considered as a style/base-font comparison if the missing glyphs are supplied separately. |
| Bravura | Release `bravura-1.482`, tag commit [`37b194378b710cc40e406ab6c4b07608bb9548ae`](https://github.com/steinbergmedia/bravura/commit/37b194378b710cc40e406ab6c4b07608bb9548ae); [OTF](https://github.com/steinbergmedia/bravura/releases/download/bravura-1.482/Bravura.otf) | Verified absent: all ten digits | Verified absent: both | OFL 1.1, with reserved font name `Bravura`. This is a separate music-symbol option, not a tone font: Bravura is the reference SMuFL font, whose musical glyph mappings use the Unicode Private Use Area. Bundle alongside a tone font only if its glyph design and metrics suit the renderer. |

The positive native match in this tested set is Unifont. Its coverage is useful evidence, not an endorsement of its visual style. The Noto candidate is a near miss; Bravura addresses the distinct music-symbol layer. Symbola was not shortlisted because this pass did not verify a downloadable font binary and usable redistribution terms from its publisher materials.

## Coverage Verification

I downloaded the exact upstream OTF/TTF files linked above and checked their cmap-derived coverage with Fontconfig's `fc-query --format='%{charset}'`. A short Node.js check expanded the reported hexadecimal ranges and tested each of U+0030 through U+0039, U+218A, and U+218B individually. This checks codepoint mappings in the files, not whether the glyph designs are visually appropriate or well-matched.

| Binary inspected | U+0030–U+0039 | U+218A | U+218B | SHA-256 |
| --- | --- | --- | --- | --- |
| `unifont-18.0.01.otf` | all present | present | present | `88d0a14d4aa9a96419720b39ba5da921a59560d76d4ec7d523acc06ed85cd3c2` |
| `NotoSansSymbols2-Regular.ttf` | all present | absent | absent | `882d142b9a1ef3fd7fa4225dbe95c10fab6664206eb4964c8ff705a4f6d02988` |
| `Bravura.otf` | all absent | absent | absent | `cdf0f893ee1fdb64b7f6713d71ee0dcfc349c0ac01429a8e451b01a9e79f5f3b` |

The test used `fc-query` rather than a font editor or a rendering fallback, so missing codepoints were not mistaken for glyphs supplied by an installed system font. No glyph rendering comparison was performed.

## Redistribution and Outline Conversion

The inspected Unifont release `COPYING`, Noto `LICENSE`, and Bravura release `OFL.txt` contain the SIL Open Font License 1.1. Its text permits bundling and redistribution of original or modified font software with software when the copyright notice and license accompany each copy. A modified font must remain under OFL; a reserved font name cannot be used for a modified version. The license defines a modified version to include removing or substituting components and changing formats, so font subsetting or format conversion should be treated as a font modification and redistributed under those conditions.

OFL 1.1 says that its requirement to keep font software under OFL does not apply to documents created using the font. This supports using font-rendered glyphs in generated output, but the license does not specifically name “extract glyph outlines to SVG paths.” Whether a renderer's reusable, distributed path asset is output artwork or a modified font component depends on how it is created and packaged; do not assume that distinction without reviewing the actual distribution. Bravura additionally reserves the name `Bravura` for unmodified font software. Unifont is dual-licensed; the GPL font-embedding exception specifically describes embedding the font or unaltered portions in a document, so this report relies on its OFL option rather than extending that exception to converted outlines. These are readings of the published terms, not legal advice.

## Remaining Human Choices

- Decide whether Unifont's visibly pixel-grid-derived forms are acceptable at note, chord, and octave-label sizes. Native cmap coverage alone does not answer this.
- Decide whether the renderer needs one coherent text/music font or accepts separate tone and SMuFL fonts with separate metrics and tuning.
- Compare tone glyph shapes, spacing, weight, and baseline against ordinary digits and the chosen music symbols in representative scores.
- Decide whether exported SVG should contain editable text with an embedded font, or font-independent path geometry. Path output improves viewer independence but is not selectable/searchable text; settle the license interpretation for the chosen packaging model.
- Choose the supported font format, subsetting policy, attribution/license delivery, and whether PNG output is rasterized from the same pinned geometry pipeline.

## Sources

- GNU Unifont 18.0.01 [release directory](https://ftp.gnu.org/gnu/unifont/unifont-18.0.01/), [OTF](https://ftp.gnu.org/gnu/unifont/unifont-18.0.01/unifont-18.0.01.otf), and [source archive containing `README` and `COPYING`](https://ftp.gnu.org/gnu/unifont/unifont-18.0.01/unifont-18.0.01.tar.gz).
- Noto Fonts at commit [`ffebf8c1ee449e544955a7e813c54f9b73848eac`](https://github.com/notofonts/noto-fonts/tree/ffebf8c1ee449e544955a7e813c54f9b73848eac): [Noto Sans Symbols 2 TTF](https://raw.githubusercontent.com/notofonts/noto-fonts/ffebf8c1ee449e544955a7e813c54f9b73848eac/unhinted/ttf/NotoSansSymbols2/NotoSansSymbols2-Regular.ttf) and [LICENSE](https://github.com/notofonts/noto-fonts/blob/ffebf8c1ee449e544955a7e813c54f9b73848eac/LICENSE).
- Steinberg [Bravura 1.482 release](https://github.com/steinbergmedia/bravura/releases/tag/bravura-1.482), [release OTF](https://github.com/steinbergmedia/bravura/releases/download/bravura-1.482/Bravura.otf), [release OFL.txt](https://github.com/steinbergmedia/bravura/releases/download/bravura-1.482/OFL.txt), and [project README](https://github.com/steinbergmedia/bravura/blob/master/README.md) describing its SMuFL role.
- [SMuFL specification](https://w3c-cg.github.io/smufl/latest/), for standardized musical glyph mappings.
- SIL [Open Font License 1.1 FAQ](https://openfontlicense.org/ofl-faq/) and the OFL text included in the upstream license files above.