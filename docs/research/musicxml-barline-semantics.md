# MusicXML Barline Semantics for Overfull Measures

## Conclusion

Use the explicit MusicXML measure structure for ordinary measure-end barlines. Do not insert additional barlines merely because the accumulated note durations cross another 3/4 boundary. MusicXML also represents exceptional barlines, including barlines inside a measure, as explicit `<barline>` data. Neither the MusicXML 4.0 measure reference nor its measure attributes define a rule that automatically splits an overfull measure at the time signature's beat total.

The two fixtures each contain two `<measure>` elements, so the renderer's two measure-end barline paths are consistent with their encoded structure. Their durations do not describe ordinary 3/4 bars: measure 1 is severely overfull and measure 2 is short. Preserve their encoded boundaries, but report the mismatch separately. A renderer may reject such input under an explicitly strict 3/4 validation policy; it should not silently rewrite the score or treat duration mismatch alone as a MusicXML parsing error.

## Source Semantics

The source is the MusicXML 4.0 Final Community Group Report, dated 1 June 2021 ([official edition](https://www.w3.org/2021/06/musicxml40/)). Its reference defines `<measure>` as a container for musical data in a partwise score and documents `number` as the measure identifier. The `implicit` attribute is set to `yes` when the measure number should not appear, with pickup measures given as an example; for a pickup, number `0` and `implicit="yes"` are typical. The attribute defaults to `no` ([`<measure>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/measure-partwise/)).

The same reference distinguishes default single measure barlines from other barline styles: a barline other than a normal single barline is represented with `<barline>`. The `<barline>` reference says barline data is a child of a partwise measure and explicitly allows barlines within measures, such as dotted subdivisions in complex meters. Its `location` defaults to `right`; an explicit right barline belongs at the end of the measure data ([`<barline>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/barline/)). Thus, normal measure-end bars follow the encoded measure structure, while exceptional internal bars require their own score data. The reference does not instruct a renderer to synthesize internal bars from event-duration totals.

MusicXML represents time signatures with `<beats>` and `<beat-type>` ([`<time>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/time/)). `<divisions>` gives the number of duration divisions per quarter note ([`<divisions>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/divisions/)). A note marked `<chord/>` does not advance the musical position: its duration is accounted for by the preceding note without `<chord/>` ([`<chord>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/chord/)). These rules determine the fixture totals below.

For conventional Western meter, *Open Music Theory*, Version 2, says a measure is one group of beats, separated from other measures by bar lines; in simple meter, the time-signature numerator gives the number of beats per measure and the denominator gives the beat unit. It also describes an anacrusis (pickup) as the opening note(s) of an imaginary measure, commonly balanced by a shortened final measure (["Simple Meter and Time Signatures"](https://viva.pressbooks.pub/openmusictheory/chapter/simple-meter-and-time-signatures/), chapter published 1 July 2021). This notation convention explains why a short measure may be intentional, but does not make every short measure a pickup or authorize changing explicit MusicXML boundaries.

## Fixture Totals

Both fixtures declare `divisions=16` and `3/4`, so one quarter note is 16 duration ticks. For each measure, count sequential notes and rests; do not add the duration of a chord tone marked `<chord/>` a second time. The source files are [`canonical.musicxml`](../../tests/fixtures/canonical.musicxml) and [`no-transposition.musicxml`](../../tests/fixtures/no-transposition.musicxml). The latter omits the transpose element; all measure event durations are otherwise the same.

| Measure | Advancing duration | Quarter-note total | Comparison with 3/4 |
| --- | ---: | ---: | --- |
| 1 | 399 ticks / 16 divisions | 399/16 = 24.9375 | 8 complete 3-quarter spans plus 15/16 quarter note; overfull |
| 2 | 16 ticks / 16 divisions | 1 | 2 quarter notes short of 3; incomplete |

For each file, the explicit measure-end positions are therefore 399/16 quarter notes from the beginning of measure 1, then 415/16 quarter notes cumulatively (399/16 + 16/16) at the end of measure 2. The fixtures do not contain `implicit="yes"`. Measure 2 is consequently not identified as a pickup, and it is not the opening anacrusis described by the notation reference. It is an unmarked short final measure in the supplied data. Measure 1 is not a pickup either; it is overfull relative to its declared 3/4 meter.

If an application chose to infer regular 3/4 bars within measure 1, the candidate internal positions would be 3, 6, 9, 12, 15, 18, 21, and 24 quarter notes after that measure's start. Those eight positions would be additional bars inside one explicitly encoded measure, with the XML boundary following just 15/16 quarter note after the last one. No such internal `<barline>` elements are present. Measure 2 is only one quarter note long, so it contains no additional 3-quarter boundary.

## Renderer Behavior and Recommendation

The current renderer iterates over `score.measures` and adds one barline path after each measure's events ([`render_svg`](../../src/main.rs)). Running the CLI on both fixtures produced exactly these paths in each SVG:

```text
barline-1: M1152 28V108
barline-2: M1195 28V108
```

The resulting count is two paths, at the final horizontal positions `x=1152` and `x=1195`. These are layout coordinates, not metrical positions; the corresponding score-time boundaries are 399/16 and 415/16 quarter notes from the score start. The output matches the two explicit measure ends and does not insert the eight candidate meter-derived bars.

**Recommendation:** keep explicit measure boundaries authoritative for rendering and do not infer more barlines from duration totals. The first fixture measure is anomalous for an ordinary 3/4 bar, but MusicXML's cited measure/barline model gives no automatic split rule. Report or reject the mismatch only in a separate, explicitly enabled strict-meter validation policy that can account for voices, chords, and other timing constructs. In particular, do not mistake a short measure for a pickup unless the score context and/or MusicXML's implicit-measure metadata support that interpretation.

## Scope and Verification

- Result: `docs/research/musicxml-barline-semantics.md`.
- Branch: `research/musicxml-barline-semantics`.
- Renderer, tests, and implementation specification: unchanged; no test changes.
- Verification: rendered both fixtures with the existing CLI to temporary SVGs and checked the barline paths and x positions above.
- Blockers: none.