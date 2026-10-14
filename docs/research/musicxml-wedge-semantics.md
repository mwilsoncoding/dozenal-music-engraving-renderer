# Research: MusicXML 4.0 Wedge Semantics

**Scope:** Official MusicXML 4.0 element reference and XSD only. This records format semantics, not renderer behavior or product decisions.

## Element and score-partwise context

In a `score-partwise` document, `<wedge>` is one of the elements allowed inside `<direction-type>`, which is part of a `<direction>` within a part's measure. The direction reference describes directions as musical instructions and notes that `<direction-type>` contains one or more direction-type elements; a direction may also contain a separate `<sound>` element. ([`<direction>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction/), [`<direction-type>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction-type/), [official `musicxml.xsd` listing](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

The required `type` attribute has exactly three schema values: `crescendo`, `diminuendo`, and `stop`. A crescendo wedge starts closed on the left; a diminuendo wedge starts closed on the right; `stop` ends the wedge. The reference describes no `continue` value for `<wedge>`. Thus the documented lifecycle is represented by a start direction (`crescendo` or `diminuendo`) and an ending direction (`stop`), with the directions positioned in the score. ([`<wedge>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/wedge/), [`wedge-type` datatype](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/wedge-type/), [official `musicxml.xsd` listing](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

## Attributes

| Attribute | Schema type / default | Reference semantics |
| --- | --- | --- |
| `type` | `wedge-type`; required; no default | `crescendo`, `diminuendo`, or `stop`, as described above. |
| `spread` | `tenths`; no documented default | Distance between the wedge's sides. It is ignored at the start of a crescendo and at the end of a diminuendo. |
| `niente` | `yes-no`; default `no` | Indicates a niente (circle) at the crescendo start or diminuendo stop. |
| `line-type` | `line-type`; default `solid` | Wedge line style. The datatype values are `solid`, `dashed`, `dotted`, and `wavy`. |
| `dash-length` | `tenths`; no documented default | Dash length when `line-type="dashed"`. |
| `space-length` | `tenths`; no documented default | Space length when `line-type="dashed"`. |
| `color` | `color`; no documented default | Color of the wedge. |
| `default-x`, `default-y` | `tenths`; no documented defaults | Default horizontal and vertical positions. |
| `relative-x`, `relative-y` | `tenths`; no documented defaults | Relative horizontal and vertical positions. |
| `number` | `number-level`; optional; default `1` | Identifies the wedge when multiple wedges overlap. |

Attribute meanings and documented defaults are from the `<wedge>` reference. The `line-type`, `yes-no`, and `number-level` allowed values/meaning are defined by their official datatypes. ([`<wedge>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/wedge/), [`line-type`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/line-type/), [`yes-no`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/yes-no/), [`number-level`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/number-level/), [official `musicxml.xsd` listing](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

`number-level` distinguishes concurrent objects that overlap in document order, and its documented range is 1 through 16. The XSD gives `number` a default of `1`; it does not declare uniqueness, matching start/stop numbers, ordering, or a non-overlap constraint for wedges. Consequently, the schema does not prohibit repeated `number` values, including on overlapping wedges; it also does not enforce a particular start/stop pairing. ([`number-level` datatype](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/number-level/), [official `musicxml.xsd` listing](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

## Notation versus playback

`<wedge>` describes the visible crescendo/diminuendo wedge notation. Playback parameters are carried separately by `<sound>`, which the reference describes as containing general playback parameters. The `<wedge>` reference specifies no playback behavior and does not define an automatic relationship to `<sound>`; the two are distinct direction content. ([`<wedge>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/wedge/), [`<sound>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/sound/), [`<direction>` reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction/))