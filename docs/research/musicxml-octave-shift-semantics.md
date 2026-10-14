# Research: MusicXML 4.0 Octave-Shift Semantics

**Scope:** Official MusicXML 4.0 `score-partwise` reference and XSD. This records format semantics only; it does not make renderer or product recommendations.

## Element and purpose

In `score-partwise`, `<octave-shift>` is a direction type: it appears inside `<direction-type>`, within a `<direction>` in a part's measure. The element is empty. Its purpose is to show notes at a different written octave because printing the pitches at their performed values would be difficult. For example, an 8va above the staff is encoded as a shift down from the pitches in the note data. ([`score-partwise`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-partwise/), [`direction`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction/), [`direction-type`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction-type/), [`octave-shift`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/octave-shift/))

## Type, size, and span markers

The required `type` is an `up-down-stop-continue` token. Its allowed values are exactly `up`, `down`, `stop`, and `continue`. The reference says `up` or `down` specifies the start and direction of the shift relative to performed values; `stop` ends a span and `continue` marks its continuation. The reference does not further define when `continue` is required or prescribe a particular system-break rule for octave shifts. ([element reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/octave-shift/), [`up-down-stop-continue` datatype](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/up-down-stop-continue/), [XSD declaration](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

`size` is optional, is an XML Schema positive integer, and defaults to `8`. The element reference identifies `8` as one octave, `15` as two octaves, and `22` as three octaves. The schema does not restrict the attribute to only those three values; its datatype admits any positive integer. ([element reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/octave-shift/), [XSD declaration](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

## Numbering and matching

Optional `number` has type `number-level`, whose allowed range is 1 through 16. The XSD datatype documentation says the implied value is 1 by default and describes the range as distinguishing up to 16 same-type objects that overlap in MusicXML document order. The `<octave-shift>` reference specifically says the number distinguishes overlapping octave shifts. The XSD does not declare a default directly on this attribute; the default of 1 is stated by the datatype documentation. ([`octave-shift`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/octave-shift/), [`number-level`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/number-level/), [XSD declaration](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

The reference and schema provide no octave-shift-specific constraint that a `stop` or `continue` must match a prior `up`/`down` by number, that every start must have a stop, or that spans cannot overlap. The schema only constrains the `number` value itself; matching/lifecycle validity is not schema-enforced. ([element reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/octave-shift/), [XSD declaration and datatype](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

## Position and line appearance

The XSD gives `<octave-shift>` the `print-style` and `dashed-formatting` attribute groups. Relevant optional print-style attributes are `default-x`, `default-y`, `relative-x`, `relative-y`, font attributes, and `color`. The reference defines the position adjustments in tenths: default positions establish the origin and relative positions adjust from that default. It assigns no defaults to these optional position/font/color attributes. ([element reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/octave-shift/), [XSD declaration and attribute groups](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

The dashed-formatting attributes are optional `dash-length` and `space-length`, both in tenths; the XSD assigns neither a default. The generic `line-type` attribute is not included in the `<octave-shift>` XSD type, and the element reference does not list it. The schema notes that dash/space lengths apply to dashed lines, but this element has no schema-declared `line-type` selector. ([element reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/octave-shift/), [XSD declaration and `dashed-formatting`](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

## Display transposition versus sounding pitch

The octave-shift describes a notational/display convention: the encoded note pitches remain the performed or true pitches, while the octave-shift indicates how the written notes are displaced for readability. Its `up`/`down` values describe the displayed shift relative to those pitches. The element's reference and XSD do not specify changing playback or sounding pitch; this element alone is not a playback-pitch instruction. ([`octave-shift`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/octave-shift/), [XSD octave-shift documentation](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))