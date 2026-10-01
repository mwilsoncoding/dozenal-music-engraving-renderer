# Research: Dozenal Music Notation

Findings from the primary source repository [mwilsoncoding/dozenal-music-notation](https://github.com/mwilsoncoding/dozenal-music-notation) detailing the concepts, visual grammar, and technical artifacts of Dozenal Music Notation.

## Core Concepts & Philosophy

- **Pitch Numbering & Tone Discretization**:
  - Uses base-12 (dozenal / duodecimal) natural numbers for pitches: `0` through `↋` (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, ↊, ↋).
  - Tones are named using DSA standard glyphs: `↊` (U+218A, "dek" / ten) and `↋` (U+218B, "el" / eleven). "Doh" is used as the abbreviation for dozen.
  - Absolute pitch mapping: Scientific pitch notation sets $0 = \text{C}_0$ ($16.35160\text{ Hz}$). Therefore, $\text{A}_4 = 440\text{ Hz}$ is denoted as `49` (four doh nine = $4 \times 12 + 9 = 57$ semitones above $\text{C}_0$).
  - Source: [references/set-and-category-theory.md](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/references/set-and-category-theory.md), [references/dsa.md](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/references/dsa.md).

- **Elimination of Accidentals and Clefs**:
  - Eliminates sharps, flats, and natural signs entirely. Single discrete glyphs represent each pitch uniquely, avoiding enharmonic spelling confusion.
  - Traditional clefs (treble, bass, alto, etc.) are eliminated and replaced by **octave indicators** placed at the leftmost position of the staff wrapping the middle line (e.g., `4` below the center line and `5` above it for the standard middle range).
  - Source: [references/sharps-flats-clefs.md](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/references/sharps-flats-clefs.md), [references/other-labels-for-tones.md](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/references/other-labels-for-tones.md).

- **Staff Architecture**:
  - Consists of a 3-line staff (top line, middle/center line, bottom line), rather than a 5-line staff.
  - Octaves are laid out vertically across staff spaces/lines. In the prototypical 3-line staff wrapping octaves 4 and 5:
    - Octave 5 rests above the center line (between center and top line).
    - Octave 4 rests below the center line (between center and bottom line).
    - Ledger lines / extensions expand above the top line (octaves 6, 7, 8, 9) and below the bottom line (octaves 3, 2, 1, 0).
  - Numbers sit inside the staff lanes without line bisection, avoiding visual clutter.
  - Source: [references/sharps-flats-clefs.md](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/references/sharps-flats-clefs.md), [references/julian-carrillo.md](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/references/julian-carrillo.md), [fiddle.html](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/fiddle.html), [helloworld.svg](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/helloworld.svg).

## Rhythmic Notation (Mensural Influence)

Borrowing concepts from Mensural notation (brevis recta and brevis altera) to represent durations without hollow noteheads:
- **Whole note**: Number glyph alone (no stem).
- **Quarter note**: Number glyph with a stem.
- **Dotted quarter note**: Number glyph with a stem and a single dot.
- **Half note**: Number glyph with a stem and two vertically aligned dots (a colon `:`).
- **Dotted half note**: Number glyph with a stem and three dots (a colon followed by a period `:.` or U+2234 `∴`).
- **Sub-divisions (eighth, sixteenth, etc.)**: Standard flags and beams attached to note stems as in classical notation.
- Source: [references/mensural-notation.md](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/references/mensural-notation.md), [helloworld.svg](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/helloworld.svg).

## Polyphony, Chords, and Harmony

- **In-staff chords / intervals**:
  - Chords within the staff enclose tone glyphs in parenthesis/bracket-like curves (e.g. SVG path arc brackets) to group simultaneous pitches, with an attached stem, duration dots, and beams.
  - Polyphony across octaves places numbers in their respective octave positions.
  - Source: [helloworld.svg](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/helloworld.svg).
- **Chord symbols (Harmonic analysis above staff)**:
  - Modeled on jazz notation, written above the staff.
  - Root tone written in large font (e.g. `↊`, `3`, `8`).
  - Triadic quality and extensions written in superscript to the right of root (e.g., `138`, `7↊1`, `037`).
  - Slash chords: Bass tone notated in subscript. If a tone is both an extension and bass tone, both are indicated.
  - Source: [references/chord-notation.md](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/references/chord-notation.md), [helloworld.svg](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/helloworld.svg).

## Fonts and Rendering Assets

- **OutfitDozenal**: Google Outfit font modified with `fontdozenalizer` to include U+218A (`↊`) and U+218B (`↋`). Used for tone glyphs, octave indicators, and chord symbols.
- **ABC2SVG**: Font converted from abc2svg SFD to TTF using FontForge. Used for traditional musical glyphs like time signatures, rests, and flags.
- Source: [README.md](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/README.md), [helloworld.svg](https://github.com/mwilsoncoding/dozenal-music-notation/blob/main/helloworld.svg).
