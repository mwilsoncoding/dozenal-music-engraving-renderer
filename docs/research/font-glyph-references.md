# Research: Open-Source Glyph Font References

## Scope

This report records font and glyph facts for the dependent work on hand-drawn, self-contained SVG glyph paths. The intended reference split is DejaVu Sans for dozenal toneheads and octave indicators, and an open-source music font for time-signature digits, rests, and note flags. This is a source and coverage survey, not a font selection or a prescription for final path shapes.

## DejaVu Sans

The upstream [DejaVu Fonts 2.37 release](https://github.com/dejavu-fonts/dejavu-fonts/releases/tag/version_2_37) includes the [TTF release archive](https://github.com/dejavu-fonts/dejavu-fonts/releases/download/version_2_37/dejavu-fonts-ttf-2.37.zip). The inspected `DejaVuSans.ttf` is Book/regular (OpenType weight class 400); `DejaVuSans-Bold.ttf` is Bold (weight class 700). The family also ships ExtraLight (200) and oblique styles, but no static 600 face. Consequently, CSS `font-weight: 600` is a request, not a matching face in this release; CSS font matching rules apply when the family is loaded as static faces ([CSS Fonts, weight matching](https://www.w3.org/TR/css-fonts-4/#font-weight-matching)).

The inspected regular and bold TTFs both have 2,048 units per em and vertical ascender/descender values of 1,901/-483 font units. Each decimal digit has a constant advance within its face: 1,303 units in Book and 1,425 in Bold. These metrics are from the tagged 2.37 binaries, not inferred from rendered pixels.

| Character | Codepoint and Unicode name | DejaVu Sans cmap | Advance, Book 400 | Advance, Bold 700 |
| --- | --- | --- | ---: | ---: |
| 0 | U+0030 DIGIT ZERO | Present | 1303 | 1425 |
| 1 | U+0031 DIGIT ONE | Present | 1303 | 1425 |
| 2 | U+0032 DIGIT TWO | Present | 1303 | 1425 |
| 3 | U+0033 DIGIT THREE | Present | 1303 | 1425 |
| 4 | U+0034 DIGIT FOUR | Present | 1303 | 1425 |
| 5 | U+0035 DIGIT FIVE | Present | 1303 | 1425 |
| 6 | U+0036 DIGIT SIX | Present | 1303 | 1425 |
| 7 | U+0037 DIGIT SEVEN | Present | 1303 | 1425 |
| 8 | U+0038 DIGIT EIGHT | Present | 1303 | 1425 |
| 9 | U+0039 DIGIT NINE | Present | 1303 | 1425 |
| ↊ | U+218A TURNED DIGIT TWO | Missing | — | — |
| ↋ | U+218B TURNED DIGIT THREE | Missing | — | — |

The Unicode names are from the [Unicode 17.0.0 UnicodeData file](https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt). The release cmap includes U+2189 but not U+218A or U+218B, so the two dozenal glyphs have no outlines or advances in either inspected face. A fallback font could therefore supply different forms and metrics; there is no glyph in these files whose metrics can be used for either character. The family’s static 400 and 700 digits also differ in advance, so the weight selected for CSS 600 affects layout as well as stroke weight.

## Music-Font References

The official [SMuFL glyph-name metadata](https://w3c-cg.github.io/smufl/metadata/glyphnames.json) assigns the requested music symbols as follows. The time-signature digit 0 is included for reference; the requested range is 1–9.

| Glyph name | Codepoint | Meaning |
| --- | --- | --- |
| `timeSig0` | U+E080 | Time-signature digit 0 |
| `timeSig1` | U+E081 | Time-signature digit 1 |
| `timeSig2` | U+E082 | Time-signature digit 2 |
| `timeSig3` | U+E083 | Time-signature digit 3 |
| `timeSig4` | U+E084 | Time-signature digit 4 |
| `timeSig5` | U+E085 | Time-signature digit 5 |
| `timeSig6` | U+E086 | Time-signature digit 6 |
| `timeSig7` | U+E087 | Time-signature digit 7 |
| `timeSig8` | U+E088 | Time-signature digit 8 |
| `timeSig9` | U+E089 | Time-signature digit 9 |
| `restWhole` | U+E4E3 | Whole (semibreve) rest |
| `restHalf` | U+E4E4 | Half (minim) rest |
| `restQuarter` | U+E4E5 | Quarter (crotchet) rest |
| `rest8th` | U+E4E6 | Eighth (quaver) rest |
| `rest16th` | U+E4E7 | 16th rest |
| `rest32nd` | U+E4E8 | 32nd rest |
| `rest64th` | U+E4E9 | 64th rest |
| `flag8thUp`, `flag8thDown` | U+E240, U+E241 | Eighth flags above/below |
| `flag16thUp`, `flag16thDown` | U+E242, U+E243 | 16th flags above/below |
| `flag32ndUp`, `flag32ndDown` | U+E244, U+E245 | 32nd flags above/below |
| `flag64thUp`, `flag64thDown` | U+E246, U+E247 | 64th flags above/below |

SMuFL maps these named glyphs into the Unicode Private Use Area. Its metadata describes the flag glyphs as combining flags above or below, rather than complete note-and-stem symbols. The rest glyphs also have alternate musical-symbol codepoints, but the three inspected fonts were checked against their SMuFL PUA mappings.

| Font and pinned source | Available outline asset | Requested glyph coverage in inspected binary | Font license |
| --- | --- | --- | --- |
| [Bravura 1.482](https://github.com/steinbergmedia/bravura/tree/bravura-1.482), commit [`37b1943`](https://github.com/steinbergmedia/bravura/commit/37b194378b710cc40e406ab6c4b07608bb9548ae) | [`redist/otf/Bravura.otf`](https://raw.githubusercontent.com/steinbergmedia/bravura/37b194378b710cc40e406ab6c4b07608bb9548ae/redist/otf/Bravura.otf); upstream README identifies Bravura as the SMuFL reference font and says its sources are UFO files in the SMuFL repository ([README](https://github.com/steinbergmedia/bravura/blob/bravura-1.482/README.md)) | All timeSig0–9, restWhole–rest64th, and flag8th–flag64th up/down codepoints present | OFL 1.1, reserved font name `Bravura`; exact [OFL.txt](https://github.com/steinbergmedia/bravura/blob/bravura-1.482/redist/OFL.txt) |
| [Petaluma 1.065](https://github.com/steinbergmedia/petaluma/tree/petaluma-1.065), commit [`532dcc7`](https://github.com/steinbergmedia/petaluma/commit/532dcc7c9ae9b9d1e92cedf4f1d326cf2509f7d1) | [`redist/otf/Petaluma.otf`](https://raw.githubusercontent.com/steinbergmedia/petaluma/532dcc7c9ae9b9d1e92cedf4f1d326cf2509f7d1/redist/otf/Petaluma.otf) | All requested codepoints present | OFL 1.1, reserved font name `Petaluma`; exact [OFL.txt](https://github.com/steinbergmedia/petaluma/blob/petaluma-1.065/redist/OFL.txt) |
| [Leland v0.80](https://github.com/MuseScoreFonts/Leland/tree/v0.80), commit [`d91cf5d`](https://github.com/MuseScoreFonts/Leland/commit/d91cf5d21045e2c294541676ad386b7ffb4ac881) | [`Leland.otf`](https://raw.githubusercontent.com/MuseScoreFonts/Leland/d91cf5d21045e2c294541676ad386b7ffb4ac881/Leland.otf), Type 1 OpenType-CFF per the upstream [README](https://github.com/MuseScoreFonts/Leland/blob/v0.80/README.md); repository also provides [`leland_metadata.json`](https://github.com/MuseScoreFonts/Leland/blob/v0.80/leland_metadata.json) | All requested codepoints present | OFL 1.1, reserved font name `Leland`; exact [LICENSE.txt](https://github.com/MuseScoreFonts/Leland/blob/v0.80/LICENSE.txt) |

Coverage above was checked against the cmap of each linked, pinned font binary, not through system-font fallback. All three are static regular music-font binaries, and all three contain the requested PUA ranges. Bravura’s upstream description identifies it as the reference font for SMuFL; Leland’s upstream README identifies it as a MuseScore-developed, SMuFL-compliant OpenType-CFF music font. These are distinct source families for comparison, not a ranking. Per-font spacing and glyph metrics remain specific to each face and its metadata.

## License and Fallback Notes

The DejaVu 2.37 archive includes a `LICENSE` based on Bitstream Vera and Arev terms. It permits font redistribution and bundling with software, requires the relevant copyright/trademark notices and permission text to accompany font copies, and requires modified font files to use names that avoid the reserved words stated in the license. The license says a font may be sold as part of a larger software package but not sold by itself. The archive also distinguishes public-domain DejaVu changes and carries separate notices for imported glyph components; preserve the complete release license rather than reducing it to a short attribution line ([release `LICENSE`](https://github.com/dejavu-fonts/dejavu-fonts/blob/version_2_37/LICENSE)).

Bravura, Petaluma, and Leland each identify the font copyright holder and a family-specific Reserved Font Name in the cited OFL 1.1 file. OFL permits bundling the font software with software subject to its terms, requires the copyright notice and license to accompany redistributed font software, and restricts modified font software from using a reserved name. It also states that the font-software licensing requirement does not apply to documents created using the font. The licenses do not specifically classify reusable SVG path definitions copied from font outlines. Whether such path data is a document output or font-derived software should therefore be reviewed for the actual distribution; this report makes no legal determination. The same distinction matters for DejaVu's terms if reusable outline components, rather than rendered score output, are copied.

Because DejaVu Sans 2.37 lacks both dozenal characters, a second outline source or separately authored forms would be needed for complete cmap coverage; whichever source supplies them can affect style and advance metrics. Music-font flag codepoints are components positioned above or below a stem, and the SMuFL PUA values require a font with corresponding mappings. A system fallback that lacks those mappings cannot provide the requested glyphs reliably. No fallback strategy or final outline design is selected here.

## Verification

The inspected DejaVu release TTFs and the three pinned OTFs were downloaded from the linked upstream assets. `fc-query` cmap data was used to check codepoint presence; an OpenType parser was used to read DejaVu units per em, vertical metrics, weight classes, and decimal advances. This verifies mapping and font metrics, not visual suitability of the glyph designs at engraving sizes. No renderer code, tests, or implementation/spec issues were changed.

## Sources

- DejaVu Fonts [2.37 release](https://github.com/dejavu-fonts/dejavu-fonts/releases/tag/version_2_37), [TTF archive](https://github.com/dejavu-fonts/dejavu-fonts/releases/download/version_2_37/dejavu-fonts-ttf-2.37.zip), and versioned [`LICENSE`](https://github.com/dejavu-fonts/dejavu-fonts/blob/version_2_37/LICENSE).
- Unicode Consortium [Unicode 17.0.0 UnicodeData.txt](https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt), for U+218A and U+218B character names.
- W3C Community Group [SMuFL glyph-name metadata](https://w3c-cg.github.io/smufl/metadata/glyphnames.json) and [SMuFL specification](https://w3c-cg.github.io/smufl/latest/).
- Steinberg Media [Bravura 1.482 source](https://github.com/steinbergmedia/bravura/tree/bravura-1.482), [OTF asset](https://raw.githubusercontent.com/steinbergmedia/bravura/37b194378b710cc40e406ab6c4b07608bb9548ae/redist/otf/Bravura.otf), and [`OFL.txt`](https://github.com/steinbergmedia/bravura/blob/bravura-1.482/redist/OFL.txt).
- Steinberg Media [Petaluma 1.065 source](https://github.com/steinbergmedia/petaluma/tree/petaluma-1.065), [OTF asset](https://raw.githubusercontent.com/steinbergmedia/petaluma/532dcc7c9ae9b9d1e92cedf4f1d326cf2509f7d1/redist/otf/Petaluma.otf), and [`OFL.txt`](https://github.com/steinbergmedia/petaluma/blob/petaluma-1.065/redist/OFL.txt).
- MuseScoreFonts [Leland v0.80 source](https://github.com/MuseScoreFonts/Leland/tree/v0.80), [Leland.otf](https://raw.githubusercontent.com/MuseScoreFonts/Leland/d91cf5d21045e2c294541676ad386b7ffb4ac881/Leland.otf), [`leland_metadata.json`](https://github.com/MuseScoreFonts/Leland/blob/v0.80/leland_metadata.json), and [`LICENSE.txt`](https://github.com/MuseScoreFonts/Leland/blob/v0.80/LICENSE.txt).
- W3C [CSS Fonts Module Level 4: font-weight matching](https://www.w3.org/TR/css-fonts-4/#font-weight-matching), for matching a CSS numeric weight request to available static faces.