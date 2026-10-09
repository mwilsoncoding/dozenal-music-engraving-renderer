# Dozenal Music Notation Renderer

Software product that renders sheet music according to dozenal music notation from structured musical input.

## Language

### Pitch & Representation

**Tone**:
A discrete musical pitch in twelve-tone equal temperament denoted by a single base-12 digit (`0` through `↋`).
_Avoid_: Note name, pitch class, chromatic step

**Doh**:
The dozenal base unit (twelve), used in tone pronunciation and compound pitch representation.
_Avoid_: Dozen, base multiplier

**Dek**:
The tone representing value ten ($10_{10}$), notated with the glyph `↊` (U+218A).
_Avoid_: Ten, T, X

**El**:
The tone representing value eleven ($11_{10}$), notated with the glyph `↋` (U+218B).
_Avoid_: Eleven, E, L

**Absolute Pitch**:
An integer count of semitones from $0$ at $\text{C}_0$ ($16.35160\text{ Hz}$), where $\text{A}_4$ is $49$ (four doh nine).
_Avoid_: MIDI number, frequency, pitch class

### Staff & Layout

**Staff**:
A three-line grid consisting of top, center, and bottom lines defining discrete octave lanes.
_Avoid_: Five-line staff, chromatic staff, staves

**System**:
A horizontal row of one or more staves aligned across a score.
_Avoid_: Page, staff

**Page**:
A physical print surface with dimensions and orientation, margins, and a printable area for one or more Systems.
_Avoid_: Canvas, System

**Octave Indicator**:
A pair of vertically stacked dozenal digits flanking the center staff line at the start of a staff to establish register.
_Avoid_: Clef, key signature, clef sign

**Ledger Line**:
An extension line drawn above the top staff line or below the bottom staff line to support notes in extreme octaves.
_Avoid_: Clef change, 8va, octave shift

### Score Structure

**Part**:
A score-level sequence of measures identified in the part list, represented by one Dozenal Staff and containing one or more Voices.
_Avoid_: Staff, Voice

**Input Staff**:
A numbered staff in MusicXML input used to locate source events within a Part; all Input Staves in a Part map to that Part's single Dozenal Staff.
_Avoid_: Dozenal Staff

**Voice**:
An independent sequence of musical events within a part; an event may contain one tone or simultaneous tones.
_Avoid_: Part, chord

### Rhythm & Glyphs

**Tonehead**:
A single dozenal digit glyph placed within an octave lane representing a sounded pitch.
_Avoid_: Notehead, hollow notehead, oval

**Unpitched Mark**:
 An X-shaped glyph marking an unpitched event without indicating a Tone.
 _Avoid_: X tonehead, pitch marker

**Stem**:
A vertical line attached to a tonehead; every duration except a whole note uses a stem.
_Avoid_: Tail, stick

**Duration Dots**:
One or more dots accompanying a tonehead indicating duration under mensural rhythmic rules (`.` for dotted quarter, `:` for half, `:.` for dotted half).
_Avoid_: Augmentation dot, colon, rest marker

**Grace Note**:
A non-duration-bearing ornamental note whose pitch is shown with a Tonehead, placed before or after a principal note.
_Avoid_: Acciaccatura, appoggiatura

**Tuplet**:
A rhythmic grouping where a specified number of events fits the time normally occupied by a different number of notated values.
_Avoid_: Triplet, time-modification

**Chord Bracket**:
A curved delimiter enclosing simultaneous toneheads within an octave or beat.
_Avoid_: Notehead cluster, chord paren, chord enclosure

### Harmony

**Chord Symbol**:
A harmonic label positioned above the staff consisting of a large root tone digit and superscript quality or extension digits.
_Avoid_: Lead sheet symbol, Roman numeral, Nashville number

**Slash Chord**:
A chord symbol containing a subscript digit designating an explicit bass tone.
_Avoid_: Inversion symbol, bass slash

### Input & Output

**MusicXML**:
The standard structured score interchange format consumed as input for conversion into dozenal notation.
_Avoid_: Score file, sheet XML, MIDI file

**Engraved Image**:
The rendered visual output of the score in a graphic format such as SVG or PNG.
_Avoid_: Score sheet, render file, canvas export
