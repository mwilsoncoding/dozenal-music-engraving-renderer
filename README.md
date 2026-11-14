# domunor (DOzenal MUsic NOtation Renderer)

Engraving software modeling DOzenal MUsic NOtation (domuno).

This is an implementation of the ideas discussed in [the Dozenal Music Notation repository](https://github.com/mwilsoncoding/dozenal-music-notation)

Any source code found therein is exploratory and local to that repo, not a recommendation.

## Dependencies

The renderer uses pure-Rust dependencies for vector PDF export. Prefer a small,
deliberate runtime dependency footprint: add a dependency when its product value
justifies its maintenance, security, and distribution costs. Development and
repository automation tools may use external dependencies without changing this
runtime policy.

## Bootstrap CLI

`cargo run -- path/to/score.musicxml [--output path/to/score.svg|path/to/score.pdf]`
writes a self-contained SVG or vector PDF, selected by the output extension.
Without `--output`, the input extension is replaced with `.svg` beside the
input. PDF output contains one PDF Page per composed score Page, with A4 or
Letter physical geometry and basic title/producer metadata.

Output uses A4 portrait Pages by default with 15 mm margins. Use
`--page-size letter` for Letter paper. Complete Systems are placed top-down
without vertical rebalancing; MusicXML `<print new-page="yes"/>` starts a new
Page. Multi-page SVG stacks white Page surfaces on a gray canvas, while PDF
exports each Page individually. Supported MusicXML page dimensions in
`<defaults>` or `<print>` select A4 or Letter paper under the score's scaling;
print-level dimensions apply to the Page they begin, and omitted dimensions
inherit the score defaults. These hints
take precedence over `--page-size`. Custom dimensions and explicit margins warn
and use renderer defaults.
Use `--scale S` to uniformly scale score content by a positive finite
multiplier (`1.0` is natural size) while keeping Page dimensions and margins
fixed. The scale is applied when determining automatic System breaks and when
rendering; scaled System bounds also determine Page composition.
MusicXML `<work><work-title>` and `<movement-title>` provide a score title.
The work title takes precedence; the movement title is used when no work title
is present. A selected title is engraved, centered above the first System on
the first Page, and included in SVG and PDF title metadata. PDF metadata falls
back to the input filename when neither title is present. `<work-number>` is
accepted as metadata but is not engraved. The title uses the bundled text
glyph repertoire and remains at natural size when `--scale` is used.

This MVP accepts uncompressed, no-namespace MusicXML 4.0
`score-partwise` scores with one or more Parts, each represented by one Dozenal
Staff and one voice. Part order and names follow the score's part-list, and
measures align by ordinal position across Parts. Supported content includes
concert pitches in octaves 0 through 9, the implemented duration and dotted
forms, rests, ties, explicit beams, representable meters, and in-staff chords.
Unsupported score content is rejected with contextual diagnostics.
Pitched events and chord-symbol roots and bass tones use the active input-staff
transposition and are engraved at sounding pitch.
Time signatures are rendered where explicitly notated rather than repeated
automatically at each System start; the Dozenal Clef appears at the score start.
Later Systems begin with a barline and align in-measure content with the
Dozenal Clef's horizontal position, reclaiming the remaining space when
annotations are absent.

The renderer-owned XML 1.0 processor handles UTF-8 and UTF-16 input and the
conventional MusicXML DOCTYPE and internal subset. It does not resolve external
resources and does not validate against a DTD or XSD; scores that require
externally declared entities are rejected. This is an application-specific
MusicXML profile, not a claim of general MusicXML support or validation.

## Render all examples

```bash
find examples/ \
-mindepth 1 \
-type f \
-name '*.musicxml' \
-exec cargo run -- {} \;
```

## Technical Inspirations

- [lilypond](https://github.com/lilypond/lilypond)
- [MuseScore](https://github.com/musescore/musescore)
