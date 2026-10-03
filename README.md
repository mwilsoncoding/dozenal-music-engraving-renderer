# domunor (DOzenal MUsic NOtation Renderer)

Engraving software modeling DOzenal MUsic NOtation (domuno).

This is an implementation of the ideas discussed in [the Dozenal Music Notation repository](https://github.com/mwilsoncoding/dozenal-music-notation)

Any source code found therein is exploratory and local to that repo, not a recommendation.

## Dependencies

None

## Bootstrap CLI

`cargo run -- path/to/score.musicxml [--output path/to/score.svg]` writes a
self-contained SVG. Without `--output`, the input extension is replaced with
`.svg` beside the input.

This first vertical slice intentionally accepts only a UTF-8, no-namespace
`score-partwise` document with one part, one measure, divisions of 1, and one
natural quarter-note event in voice 1 at octave 4 or 5. It rejects DTDs,
entities, additional score structures, and other XML constructs. This is a
temporary input subset, not a conforming or general-purpose MusicXML parser.

## Technical Inspirations

- [lilypond](https://github.com/lilypond/lilypond)
- [MuseScore](https://github.com/musescore/musescore)
