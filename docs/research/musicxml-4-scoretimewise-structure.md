# MusicXML 4.0 `score-timewise` Structure and Renderer Impact

**Context:** Wayfinder research ticket “Research score-timewise structure and renderer impact.” This note compares the two MusicXML 4.0 score organizations and records implications for a future `score-timewise` acceptance milestone. It does not change the current parser contract or commit the project to support all MusicXML features.

## Findings at a glance

`score-partwise` and `score-timewise` are two XML organizations for score data, not two different note-notation vocabularies. Partwise places each part’s ordered measures together; timewise places each score measure together and nests the contributing parts inside it. Both use the same document metadata and part list, and both measure forms reuse the `music-data` group for the ordered musical content. The change is therefore structural at the document and traversal boundary, while the renderer still needs per-part musical state and event order.

The current importer is explicitly a narrow `score-partwise` parser: it expects one part and processes that part’s measures in order. Future timewise acceptance should add a separate structural path/normalization layer that yields the existing renderer’s per-part measure sequences, then reuse the established event interpretation where the shared content and supported subset permit it. This is a renderer architecture recommendation, not a MusicXML requirement.

## Standards facts

### Document roots and container order

The MusicXML 4.0 reference and XSD define separate document roots, `<score-partwise>` and `<score-timewise>`. Each follows the same leading score-level sequence: optional work/movement/identification/defaults data, zero or more credits, then `<part-list>`. The following score-data container differs:

- `score-partwise` contains the required part list followed by one or more `<part>` elements. Each `<part>` contains one or more `<measure>` elements.
- `score-timewise` contains the required part list followed by one or more `<measure>` elements. Each timewise `<measure>` contains one or more `<part>` elements.

Thus the traversal is part-major in partwise (`part → measure → measure content`) and measure-major in timewise (`measure → part → measure content`). These are schema content models; schema-valid structure does not itself guarantee that a renderer supports every permitted element. Sources: [`score-partwise` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-partwise/), [`score-timewise` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-timewise/), and [MusicXML 4.0 XSD](https://www.w3.org/2021/06/musicxml40/musicxml.xsd).

### Measures, numbering, and attributes

Both measure forms use the `measure` element name, but are distinct declarations for their respective parent contexts. The reference/XSD define a required `number` attribute and optional `implicit`, `non-controlling`, and `width` attributes. The number is a string-valued label, not a schema-enforced sequential integer counter. `implicit` indicates an implicit measure; `non-controlling` marks a measure that does not control measure numbering; `width` is a layout value in tenths. These fields are not interchangeable with the renderer’s internal zero-based measure index. Sources: [`measure (partwise)`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/measure-partwise/), [`measure (timewise)`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/measure-timewise/), and the [XSD](https://www.w3.org/2021/06/musicxml40/musicxml.xsd).

### Part membership and order

`<part-list>` is shared by both roots. Its `<score-part>` entries declare parts using IDs, and the order of those entries establishes score part order. `<part-group>` entries express grouping (such as braces/brackets), not an alternate event traversal. In partwise, each top-level `<part id>` identifies its declared part. In timewise, each measure’s nested `<part id>` identifies which part’s content is present in that measure. The XSD gives timewise measures one or more nested part elements and requires their IDs; it does not turn the part-list into the musical content itself. Sources: [`part-list` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/part-list/), [`score-part` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-part/), [`measure (timewise)` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/measure-timewise/), and the [XSD](https://www.w3.org/2021/06/musicxml40/musicxml.xsd).

### Shared musical content and semantics

The XSD defines shared `music-data` content for both partwise and timewise measures. It includes the same ordered, interleavable musical elements, including `attributes`, `note`, `backup`, `forward`, `direction`, `harmony`, `figured-bass`, `sound`, and `barline` (among others). The `note` and `attributes` declarations and their content models are shared rather than separate timewise vocabularies. The order within each part’s measure content remains meaningful: cursor movement and mid-measure state changes must be interpreted in sequence. Sources: [XSD](https://www.w3.org/2021/06/musicxml40/musicxml.xsd), [`note` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/note/), [`attributes` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/attributes/), and [`measure (partwise)` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/measure-partwise/).

This shared content model supports reuse of note, pitch, duration, chord, tie, beam, and supported notation interpretation after the containing part stream is identified. It does not mean state is global across parts: divisions, transposition, meter, and other part-local state must follow the corresponding part’s ordered measures. The MusicXML partwise and timewise examples/reference describe alternate score organizations; they do not assign different note meanings based solely on the root. Sources: [MusicXML 4.0 reference index](https://www.w3.org/2021/06/musicxml40/musicxml-reference/), [`score-partwise` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-partwise/), [`score-timewise` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-timewise/), and the [XSD](https://www.w3.org/2021/06/musicxml40/musicxml.xsd).

### Structural and validation differences

The structural constraints differ at the score-data level: `score-partwise` requires part containers whose content is measures; `score-timewise` requires measure containers whose content is parts. The shared `music-data` definition does not erase that parent/child distinction. Likewise, a parser that checks only whether a root is a valid MusicXML name has not established that the child order, required elements, attributes, types, or references conform to the XSD.

An XSD validator can check the respective declared content models, occurrence constraints, attribute types, and ID/IDREF relationships. It does not establish the renderer’s support for musical constructs, does not create missing score-part data, and does not decide how partial score data should be rendered. Acceptance policy for missing/extra part entries, duplicate logical part content, unsupported constructs, or consistency beyond schema constraints must be explicit in the importer/renderer contract. Sources: [MusicXML 4.0 XSD](https://www.w3.org/2021/06/musicxml40/musicxml.xsd), [`score-partwise` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-partwise/), and [`score-timewise` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-timewise/).

## Local parser assumptions

The current implementation is in [`src/main.rs`](../../src/main.rs#L178):

- It rejects any root other than no-namespace `<score-partwise>` and requires version `4.0`.
- It requires exactly `<part-list>` followed by one `<part>`, exactly one `<score-part>`, and matching part IDs. This is a stricter supported subset than the MusicXML XSD, which allows multiple score parts.
- It iterates `<measure>` children of that single part in source order and requires each measure’s `number` attribute. It does not use the source number as its vector index.
- It processes each measure’s children in order, carrying divisions, transposition, and meter state through the part’s measures. It keeps tie/beam tracking for that part. Each measure is converted to a renderer `Measure` containing events, harmonies, and meter.

Consequently, the current internal `Score` is already organized as a flat ordered list of measures for one part. It does not currently carry a part identity/order dimension through parsing. A timewise document would fail at the root check before any measure content is interpreted.

## Renderer choices for a future acceptance milestone

These are recommendations, not standards mandates:

1. **Set the acceptance contract narrowly.** Decide whether the milestone means accepting both root forms for the exact existing renderer subset (currently one part and selected constructs), or expanding multi-part rendering at the same time. Do not infer multi-part support from accepting timewise’s container shape.
2. **Normalize structure before event interpretation.** For timewise input, walk measures in document order and route each nested `<part id>` content into that part’s own measure sequence, preserving each part’s relative measure and child order. Preserve the declared part-list order for later layout. If the renderer continues to support only one part, reject other declared/present parts with an explicit unsupported-shape diagnostic rather than silently dropping them.
3. **Keep state part-local.** Each normalized part stream needs its own divisions, transposition, meter, tie, beam, and other state. A naïve document-order walk that shares mutable state between nested parts would conflate simultaneous parts.
4. **Reuse supported event parsing.** Factor or adapt the existing per-measure event parser only after extracting the part identity and establishing the same ordered measure-content stream. Preserve current score-partwise acceptance/rejection behavior as a regression contract; do not loosen it as a side effect of adding a second root.
5. **Separate schema validity from product acceptance.** Choose whether the milestone runs XSD validation or performs its own structural checks; specify diagnostics and unsupported-feature behavior either way. Validation should not be represented as complete renderer support.

A focused acceptance suite should pair equivalent partwise/timewise fixtures for the supported subset and assert equivalent normalized measures/render output. Add cases for timewise part ordering by ID, absent or repeated part entries across measures according to the chosen policy, nonnumeric and implicit measure labels, multiple parts (accepted or explicitly rejected), part-local state, and unexpected children/attributes. Keep existing partwise fixture behavior unchanged.

## Primary sources

- [MusicXML 4.0 Final Community Group Report and edition index](https://www.w3.org/2021/06/musicxml40/) (1 June 2021).
- [MusicXML 4.0 `score-partwise` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-partwise/).
- [MusicXML 4.0 `score-timewise` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-timewise/).
- [MusicXML 4.0 partwise measure reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/measure-partwise/).
- [MusicXML 4.0 timewise measure reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/measure-timewise/).
- [MusicXML 4.0 `part-list` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/part-list/).
- [MusicXML 4.0 `score-part` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-part/).
- [MusicXML 4.0 `note` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/note/).
- [MusicXML 4.0 `attributes` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/attributes/).
- [Official MusicXML 4.0 XSD](https://www.w3.org/2021/06/musicxml40/musicxml.xsd).
