# Research: MuseScore and LilyPond text collision and placement

**Scope:** How score text, direction-like text, and rehearsal marks are placed when they approach other notation or each other. This compares pinned implementation snapshots and official manuals; it does not define renderer behavior.

## Sources and versions

Implementation observations use LilyPond commit [`e01b9d8`](https://github.com/lilypond/lilypond/commit/e01b9d8b7d4fc0d8a3ca033c74d2bbbeeeb85cfd) (`v2.25.2`) and MuseScore commit [`ac9d3bcd`](https://github.com/musescore/MuseScore/commit/ac9d3bcd3df7a19d16d7ef505db9e2b54235c022) (`v4.5.2`). They do not establish behavior for every release, user setting, or text class.

- Official LilyPond manual: [Vertical collision avoidance](https://lilypond.org/doc/v2.25/Documentation/notation/vertical-collision-avoidance), [Text marks](https://lilypond.org/doc/v2.25/Documentation/notation/writing-text#text-marks), and [Page layout](https://lilypond.org/doc/v2.25/Documentation/notation/page-layout).
- Official MuseScore Handbook 4: [Text](https://musescore.org/en/handbook/4/text), [Rehearsal marks](https://musescore.org/en/handbook/4/rehearsal-marks), and [Formatting](https://musescore.org/en/handbook/4/formatting).
- LilyPond source: [`script-column.cc`](https://github.com/lilypond/lilypond/blob/e01b9d8b7d4fc0d8a3ca033c74d2bbbeeeb85cfd/lily/script-column.cc), [`staff-spacing.cc`](https://github.com/lilypond/lilypond/blob/e01b9d8b7d4fc0d8a3ca033c74d2bbbeeeb85cfd/lily/staff-spacing.cc), and [`page-spacing.cc`](https://github.com/lilypond/lilypond/blob/e01b9d8b7d4fc0d8a3ca033c74d2bbbeeeb85cfd/lily/page-spacing.cc).
- MuseScore source: [`scoreverticalviewlayout.cpp`](https://github.com/musescore/MuseScore/blob/ac9d3bcd3df7a19d16d7ef505db9e2b54235c022/src/engraving/rendering/score/scoreverticalviewlayout.cpp#L1377-L1416), [`systemlayout.cpp`](https://github.com/musescore/MuseScore/blob/ac9d3bcd3df7a19d16d7ef505db9e2b54235c022/src/engraving/rendering/score/systemlayout.cpp#L2291-L2423), [`systemlayout.cpp`](https://github.com/musescore/MuseScore/blob/ac9d3bcd3df7a19d16d7ef505db9e2b54235c022/src/engraving/rendering/score/systemlayout.cpp#L2818-L3055), and [`textbase.cpp`](https://github.com/musescore/MuseScore/blob/ac9d3bcd3df7a19d16d7ef505db9e2b54235c022/src/engraving/dom/textbase.cpp).

## Implementation observations

### LilyPond

- LilyPond's vertical-collision documentation describes a system for placing objects outside the staff and controlling their vertical relationships. The source `script-column.cc` processes scripts in priority order and computes offsets for neighboring scripts; this supports vertical separation/stacking for applicable script-column objects, not a claim that all arbitrary text is automatically stacked.
- `staff-spacing.cc` uses staff-related extents and spacing constraints when determining vertical distances. `page-spacing.cc` then participates in spacing systems on the page. Thus collision-related extents can affect staff/page layout; that does not establish that every collision increases spacing, or that spacing is always preferred over moving an item.
- Text marks can be given explicit placement/offset controls in the manual. The sources reviewed do not establish a universal policy that authored text anchors are immutable, nor a universal policy that they are discarded when resolving collisions. Behavior depends on the grob, properties, and context.

### MuseScore

- In the pinned system-layout sequence, rehearsal marks are collected, laid out, and then passed to `Autoplace::autoplaceSegmentElement` after tempo-item alignment. This is direct evidence that rehearsal marks may be automatically repositioned by that layout path. It is not evidence that all score text or imported direction text uses the same path.
- `SystemLayout::layout2` computes distances between staves using their skylines, except where spacing is fixed; its comments also describe continuous-view distances that can grow when needed and may not shrink until a full layout. Consequently, skyline geometry can influence staff separation. The separate outside-element adjustment code examines overlap with adjacent staff skylines and can move eligible elements vertically, but has guards for already-stacked outside elements and item relationships.
- The cited text-base source is the owning text model/layout surface, but the reviewed evidence does not justify a blanket statement that authored positions are always preserved or always overridden. Autoplace status, style, item type, and explicit positioning matter.
- The cited paths support collision-aware repositioning and spacing changes, but do not prove that every text collision is resolved by stacking, or that page spacing is automatically expanded for every such case.

## Possible renderer recommendations (not adopted rules)

1. Treat authored anchor/placement data separately from the collision-adjusted display position; preserve the former even if a layout pass derives the latter.
2. Define collision behavior by text class. A rehearsal mark, staff-attached direction, and free/system text need not share a placement policy.
3. Consider a deterministic vertical stack for eligible same-side items, then test whether remaining geometric conflicts should expand staff/system spacing. State priorities and fixed-spacing behavior explicitly.
4. Keep a no-collision path that leaves authored placement unchanged, and record when automatic displacement occurs so behavior is inspectable.

These are design candidates inferred from the comparison, not policies attributed to either engine and not requirements for this renderer.

## Limits and uncertainty

The source snapshots are version-specific. The manuals document user-facing controls, not a complete algorithm contract. “Score text” covers multiple internal item classes: dynamics, rehearsal marks, technique/expression text, tempo text, and system text do not necessarily share collision behavior. The examined evidence confirms specific autoplace and spacing paths but does not establish the complete handling of every class, every imported MusicXML direction, or every combination of explicit offsets and collision settings. No controlled score was rendered in both programs.