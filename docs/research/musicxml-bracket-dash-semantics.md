# Research: MusicXML 4.0 Bracket and Dashes Semantics

**Scope:** Official MusicXML 4.0 references and XSD for `<bracket>` and `<dashes>` in score-partwise directions. This records format semantics, not renderer behavior or product decisions.

## Placement and lifecycle

Both elements are empty visual spans permitted inside `<direction-type>` within a direction. `<bracket>` is used with words in modern directions; `<dashes>` represents dashes, for example with `cresc.` and `dim.` marks. Each requires a `type` of datatype `start-stop-continue`: `start` begins the span, `stop` ends it, and `continue` carries it onward, including across system breaks. Both require corresponding start/stop/continue events in the direction stream for an intended span, but the XSD does not enforce matching events, ordering, or a complete pair. ([`<direction>`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction/), [`<direction-type>`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction-type/), [`<bracket>`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/bracket/), [`<dashes>`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/dashes/), [`start-stop-continue`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/start-stop-continue/), [official `musicxml.xsd` listing](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

## Attributes and defaults

| Attribute | `<bracket>` | `<dashes>` | Semantics / documented default |
| --- | --- | --- | --- |
| `type` | Required, `start-stop-continue` | Required, `start-stop-continue` | `start`, `stop`, or `continue`; no default. |
| `line-end` | Required, `line-end` | Not present | Bracket end treatment: `up`, `down`, `both`, `arrow`, or `none`; no default is documented. |
| `line-type` | Optional, `line-type` | Not present | `solid`, `dashed`, `dotted`, or `wavy`. Bracket reference says solid if unspecified. |
| `end-length` | Optional, `tenths` | Not present | Length of an up/down bracket jog; no default documented. |
| `dash-length` | Optional, `tenths` | Optional, `tenths` | Length of dashes; bracket reference says ignored unless its `line-type` is dashed. No default documented. |
| `space-length` | Optional, `tenths` | Optional, `tenths` | Length of spaces; bracket reference says ignored unless its `line-type` is dashed. No default documented. |
| `number` | Optional, `number-level` | Optional, `number-level` | Distinguishes multiple same-type spans that overlap in MusicXML document order. Range 1–16; datatype documentation says an omitted optional value is 1. |
| `color` | Optional, `color` | Optional, `color` | Element color; no default documented. |
| `default-x`, `default-y` | Optional, `tenths` | Optional, `tenths` | Overrides computed default position; no defaults documented. X is relative to the measure/system start; Y to the staff top line. Positive X is right, positive Y is up. |
| `relative-x`, `relative-y` | Optional, `tenths` | Optional, `tenths` | Offset from default position; no defaults documented. Positive X is right, positive Y is up. |
| `id` | Optional, `ID` | Optional, `ID` | ID unique to the document; no default documented. |

The XSD declares `line-end` and `type` required on `<bracket>`, and `type` required on `<dashes>`. It declares `number` as optional `number-level` on both, without an attribute-level default; the default of 1 comes from the `number-level` datatype documentation. That datatype is a positive integer bounded from 1 through 16. Its documentation describes numbers as distinguishing overlapping concurrent objects of the same type and notes that starts/stops may occur in arbitrary document order when polyphonic parts are involved. The XSD contains no bracket/dashes identity constraint requiring a matching number, enforcing start/stop pairing, or disallowing overlap. ([`number-level`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/number-level/), [`line-end`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/line-end/), [`line-type`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/line-type/), [official `musicxml.xsd` listing](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/))

Other listed positioning and appearance attributes are optional and have no documented default in these element references. `bracket` additionally has `line-end`, `line-type`, and `end-length`; `dashes` has none of those attributes. `dash-length` and `space-length` are available on both. ([`<bracket>`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/bracket/), [`<dashes>`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/dashes/))

## Visual notation versus pedal and playback

These elements describe line-based visual direction spans. They are not `<pedal>` marks, which have their own element and lifecycle, and neither element itself specifies playback behavior. Playback parameters are represented separately by `<sound>` when present in a direction; the bracket/dashes references define no automatic playback effect or linkage to `<sound>`. ([`<pedal>`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/pedal/), [`<sound>`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/sound/), [`<direction>`](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction/))