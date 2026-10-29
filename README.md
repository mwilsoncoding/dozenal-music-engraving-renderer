# domunor (DOzenal MUsic NOtation Renderer)

Engraving software modeling DOzenal MUsic NOtation (domuno).

This is an implementation of the ideas discussed in [the Dozenal Music Notation repository](https://github.com/mwilsoncoding/dozenal-music-notation)

Any source code found therein is exploratory and local to that repo, not a recommendation.

## Dependencies

The runtime dependency set is currently empty. Prefer a small, deliberate
runtime dependency footprint: add a dependency when its product value justifies
its maintenance, security, and distribution costs. Development and repository
automation tools may use external dependencies without changing this runtime
policy.

## Bootstrap CLI

`cargo run -- path/to/score.musicxml [--output path/to/score.svg]` writes a
self-contained SVG. Without `--output`, the input extension is replaced with
`.svg` beside the input.

Output uses A4 portrait Pages by default with 15 mm margins. Use
`--page-size letter` for Letter paper. Complete Systems are placed top-down
without vertical rebalancing; MusicXML `<print new-page="yes"/>` starts a new
Page. Supported MusicXML page dimensions in `<defaults>` or `<print>` select A4
or Letter paper under the score's scaling; print-level dimensions apply to the
Page they begin, and omitted dimensions inherit the score defaults. These hints
take precedence over `--page-size`. Custom dimensions and explicit margins warn
and use renderer defaults. Multi-page SVG output stacks white Page surfaces on
a gray canvas.

This SVG-only MVP accepts uncompressed, no-namespace MusicXML 4.0
`score-partwise` scores with one or more Parts, each represented by one Dozenal
Staff and one voice. Part order and names follow the score's part-list, and
measures align by ordinal position across Parts. Supported content includes
concert pitches in octaves 0 through 9, the implemented duration and dotted
forms, rests, ties, explicit beams, representable meters, and in-staff chords.
Unsupported score content is rejected with contextual diagnostics.

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
