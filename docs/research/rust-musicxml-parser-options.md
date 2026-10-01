# Rust MusicXML Parser Options

Exploratory research for [Assess Rust MusicXML parser options against the MVP subset](https://github.com/mwilsoncoding/dozenal-music-engraving-renderer/issues/8). The exact supported MusicXML subset and compressed-file policy remain undecided in [Define the MVP MusicXML input subset](https://github.com/mwilsoncoding/dozenal-music-engraving-renderer/issues/2), so conclusions are provisional.

## Options

### `roxmltree`

[`roxmltree`](https://docs.rs/roxmltree/0.21.1/roxmltree/) builds a read-only XML tree, supports namespaces, and reports parse errors with source positions ([`Document`](https://docs.rs/roxmltree/0.21.1/roxmltree/struct.Document.html), [`Error`](https://docs.rs/roxmltree/0.21.1/roxmltree/enum.Error.html)). This is a straightforward fit for a small, strict MusicXML subset: inspect the tree, map supported elements into a renderer-owned score model, and reject unsupported content. The tradeoff is retaining the parsed tree in memory. It supplies XML structure, not MusicXML score semantics.

### `quick-xml`

[`quick-xml`](https://docs.rs/quick-xml/0.42.0/quick_xml/reader/struct.Reader.html) provides event-based incremental parsing, typed XML errors, and byte positions. Its [`NsReader`](https://docs.rs/quick-xml/0.42.0/quick_xml/reader/struct.NsReader.html) manages namespace declarations, while the application handles MusicXML mapping and semantic validation. This can be preferable if streaming or input size matters, but leaves more parsing state and unknown-element policy to the application. Line/column diagnostics would need to be derived from byte offsets. Version 0.42.0 declares Rust 1.86 as its minimum version.

### Dedicated `musicxml` crate

The [`musicxml` 1.1.2 crate](https://docs.rs/musicxml/1.1.2/musicxml/) provides partwise/timewise score types, note concepts including pitch, rest, tie, voice, and time modification, and advertises `.mxl` support. Its parser is a handwritten XML-to-tree implementation ([parser source](https://docs.rs/crate/musicxml/1.1.2/source/src/parser/xml_parser.rs)); parser/model errors are strings without source positions. The note deserializer ([source](https://docs.rs/crate/musicxml/1.1.2/source/src/elements/note.rs)) ignores unrecognized child elements, which does not provide strict reject-unsupported behavior without added validation. The custom compressed-file parser ([source](https://docs.rs/crate/musicxml/1.1.2/source/src/parser/zip_parser.rs)) should be tested against the chosen `.mxl` policy rather than treated as a guarantee of compatibility.

## Provisional Recommendation

If the MVP accepts a deliberately small subset and prioritizes useful diagnostics plus explicit rejection, start with `roxmltree` and implement a narrow mapping into the renderer's own score model. Choose `quick-xml` if streaming is a demonstrated requirement. Adopt the dedicated crate only if fixtures confirm its behavior for the selected XML forms and the project addresses unknown-element rejection and location-aware errors. Evaluate `.mxl` archive handling separately if compressed MusicXML is included.

This recommendation should be revisited when the input subset is resolved. The crate's current maintenance activity was not established by this research.

## Sources

- [`roxmltree` documentation](https://docs.rs/roxmltree/0.21.1/roxmltree/)
- [`quick-xml` Reader documentation](https://docs.rs/quick-xml/0.42.0/quick_xml/reader/struct.Reader.html)
- [`quick-xml` namespace reader](https://docs.rs/quick-xml/0.42.0/quick_xml/reader/struct.NsReader.html)
- [`quick-xml` error types](https://docs.rs/quick-xml/0.42.0/quick_xml/errors/index.html)
- [`musicxml` documentation](https://docs.rs/musicxml/1.1.2/musicxml/)
- [`musicxml` parser source](https://docs.rs/crate/musicxml/1.1.2/source/src/parser/xml_parser.rs)
- [`musicxml` note model source](https://docs.rs/crate/musicxml/1.1.2/source/src/elements/note.rs)
- [`musicxml` compressed-file parser source](https://docs.rs/crate/musicxml/1.1.2/source/src/parser/zip_parser.rs)
