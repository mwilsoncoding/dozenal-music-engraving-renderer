# Research: MusicXML 4.0 dynamics vocabulary

**Scope:** MusicXML 4.0 reference and XSD only. These findings describe the format, not renderer behavior or product scope.

## Content model and vocabulary

`<dynamics>` contains zero or more children, in any order and with repetition, chosen from: `<p>`, `<pp>`, `<ppp>`, `<pppp>`, `<ppppp>`, `<pppppp>`; `<f>`, `<ff>`, `<fff>`, `<ffff>`, `<fffff>`, `<ffffff>`; `<mp>`, `<mf>`, `<sf>`, `<sfp>`, `<sfpp>`, `<fp>`, `<rf>`, `<rfz>`, `<sfz>`, `<sffz>`, `<fz>`, `<n>`, `<pf>`, `<sfzp>`; and `<other-dynamics>`. The XSD models this as an optional, unbounded choice; the individual named dynamics are empty elements. ([`<dynamics>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/dynamics/), [official `musicxml.xsd` listing](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

The reference says the names use the actual dynamic letters to avoid ambiguity among abbreviations, and that child elements may be combined to form marks not covered by one element (for example, `<sf/><mp/>`). `<other-dynamics>` is for a mark not covered by combinations of the individual children; its content is a string and it has an optional `smufl` glyph-name attribute. ([`<dynamics>`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/dynamics/), [`<other-dynamics>`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/other-dynamics/))

## Attributes with presentation relevance

The `<dynamics>` XSD type includes these attribute groups:

- `print-style-align`: color, font family/size/style/weight, horizontal and vertical alignment, and positioning attributes such as `default-x`, `default-y`, `relative-x`, and `relative-y`.
- `placement`: above/below placement.
- `text-decoration`: underline, overline, and line-through.
- `enclosure`: enclosure shape and associated formatting.
- `optional-unique-id`: optional document-unique ID.

These are schema attribute groups, and the reference describes their presentation meanings. The reference specifies that `placement` on `<dynamics>` applies when it is associated with a `<note>`; for a `<direction>`, it is ignored in favor of `<direction>`'s `placement`. The `<dynamics>` type does **not** include the `print-object` attribute group, so `print-object` is not an attribute of this element in the XSD. ([`<dynamics>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/dynamics/), [official `musicxml.xsd` listing](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

## Schema validity boundary

The XSD permits `<dynamics/>` because its child choice has `minOccurs="0"`; it also permits any number and ordering of the listed children. A child outside that choice is not valid against this content model. Each standard dynamic child is empty; `<other-dynamics>` carries string content. This records only constraints stated by the XSD, not broader conformance or rendering behavior. ([official `musicxml.xsd` listing](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/), [`<other-dynamics>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/other-dynamics/))