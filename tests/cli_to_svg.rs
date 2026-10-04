use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch_dir() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("domunor-cli-{}-{nonce}", std::process::id()))
}

fn assert_filled_glyph_paths(svg: &str) {
    for line in svg.lines().filter(|line| {
        line.contains("class=\"tonehead\"")
            || line.contains("class=\"rest\"")
            || line.contains("class=\"flag\"")
            || line.contains("id=\"meter-")
            || line.contains("id=\"octave-")
    }) {
        assert!(
            line.contains("fill=\"#171717\""),
            "glyph must be filled: {line}"
        );
        let is_bravo_symbol = line.contains("class=\"rest\"") || line.contains("class=\"flag\"");
        let expected_fill_rule = if is_bravo_symbol {
            "nonzero"
        } else {
            "evenodd"
        };
        assert!(
            line.contains(&format!("fill-rule=\"{expected_fill_rule}\"")),
            "glyph fill rule should be {expected_fill_rule}: {line}"
        );
        assert!(
            !line.contains("stroke="),
            "glyph must not use strokes: {line}"
        );
    }
}

fn assert_measure_barline_layout(svg: &str) {
    let barlines = svg
        .lines()
        .filter(|line| line.contains("id=\"barline-"))
        .collect::<Vec<_>>();
    assert_eq!(
        barlines.len(),
        9,
        "expected one barline per 4/4 measure: {svg}"
    );
    assert!(
        barlines.iter().all(|line| line.contains(" 27.5V108.5")),
        "measure barline borders should meet the outer staff-line borders: {svg}"
    );
    let final_barline_x = barlines[8]
        .split(" d=\"M")
        .nth(1)
        .and_then(|value| value.split(' ').next())
        .and_then(|value| value.parse::<i32>().ok())
        .expect("final barline x position");
    let staff_end_x = svg
        .lines()
        .find(|line| line.contains("id=\"staff-line-1\""))
        .and_then(|line| line.split('H').nth(1))
        .and_then(|value| value.split('\"').next())
        .and_then(|value| value.parse::<i32>().ok())
        .expect("staff end x position");
    assert_eq!(
        final_barline_x, staff_end_x,
        "final barline must meet the staff ends: {svg}"
    );
}

fn utf16_bytes(source: &str, little_endian: bool) -> Vec<u8> {
    let mut bytes = if little_endian {
        vec![0xff, 0xfe]
    } else {
        vec![0xfe, 0xff]
    };
    for unit in source.encode_utf16() {
        bytes.extend(if little_endian {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        });
    }
    bytes
}

#[test]
fn renders_fixture_to_default_and_explicit_svg_paths() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("minimal.musicxml");
    fs::write(&input, include_str!("fixtures/minimal.musicxml")).expect("write fixture");

    let default = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        default.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&default.stderr)
    );
    let default_svg = directory.join("minimal.svg");
    let svg = fs::read_to_string(&default_svg).expect("read default SVG");
    let view_box = svg
        .lines()
        .next()
        .and_then(|line| line.split("viewBox=\"0 ").nth(1))
        .and_then(|value| value.split('\"').next())
        .expect("viewBox");
    let background_geometry = view_box.split_whitespace().collect::<Vec<_>>();
    assert_eq!(background_geometry.len(), 3);
    assert_eq!(
        svg.lines().nth(1),
        Some(
            format!(
                "<rect id=\"background\" x=\"0\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#fff\"/>",
                background_geometry[0], background_geometry[1], background_geometry[2]
            )
            .as_str()
        ),
        "opaque white background must fill the viewBox behind the score"
    );
    assert_eq!(svg.matches("id=\"staff-line-").count(), 3);
    assert!(svg.contains("id=\"octave-4\""));
    assert!(svg.contains("id=\"octave-5\""));
    assert!(svg.contains("id=\"tonehead\""));
    assert_filled_glyph_paths(&svg);
    assert!(!svg.contains("<text"), "SVG glyphs must not rely on fonts");

    let explicit = directory.join("chosen.svg");
    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .arg("--output")
        .arg(&explicit)
        .output()
        .expect("run CLI with explicit output");
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(explicit.is_file(), "explicit SVG path was not created");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn accepts_a_single_voice_with_a_nondefault_identifier() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("voice-two.musicxml");
    let fixture =
        include_str!("fixtures/minimal.musicxml").replace("<voice>1</voice>", "<voice>2</voice>");
    fs::write(&input, fixture).expect("write single-voice score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "single voice with ID 2 was rejected: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("voice-two.svg").is_file());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_an_empty_voice_identifier_without_writing_svg() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("empty-voice.musicxml");
    let fixture =
        include_str!("fixtures/minimal.musicxml").replace("<voice>1</voice>", "<voice> </voice>");
    fs::write(&input, fixture).expect("write score with empty voice identifier");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success(),
        "empty voice identifier was accepted"
    );
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("voice> identifier must not be empty"));
    assert!(diagnostic
        .contains("XML path /score-partwise/part[@id='P1']/measure[@number='1']/note/voice"));
    assert!(
        !directory.join("empty-voice.svg").exists(),
        "invalid score must not produce an SVG"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn spaces_sequential_notes_according_to_duration() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("rhythmic-sequence.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>",
        "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type></note>\n      <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type></note>\n      <note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>\n      <note><pitch><step>F</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>",
    ).replace(
        "<divisions>1</divisions>",
        "<divisions>2</divisions>",
    );
    fs::write(&input, fixture).expect("write rhythmic score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "rhythmic score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("rhythmic-sequence.svg")).expect("read SVG");
    let positions = svg
        .lines()
        .filter(|line| line.contains("class=\"tonehead\""))
        .map(|line| {
            let start = line.find("translate(").expect("tonehead transform") + 10;
            let end = line[start..].find(' ').expect("x/y separator") + start;
            line[start..end].parse::<i32>().expect("x coordinate")
        })
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 4, "{svg}");
    let intervals = positions
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect::<Vec<_>>();
    assert!(intervals[0] > 0, "notes must progress horizontally: {svg}");
    assert_eq!(intervals[0], intervals[1], "equal eighth intervals: {svg}");
    assert_eq!(intervals[2], intervals[0] * 2, "quarter spacing: {svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn enforces_minimum_clearance_between_vertically_overlapping_event_geometry() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("collision-spacing.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let notes = "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>64th</type></note>\n      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>64th</type></note>\n      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>64th</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(original_note, notes)
        .replace("<divisions>1</divisions>", "<divisions>16</divisions>");
    fs::write(&input, fixture).expect("write collision-spacing score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "collision-spacing score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("collision-spacing.svg")).expect("read SVG");
    let tonehead_xs = svg
        .lines()
        .filter(|line| line.contains("class=\"tonehead\""))
        .map(|line| {
            line.split("transform=\"translate(")
                .nth(1)
                .and_then(|value| value.split_whitespace().next())
                .and_then(|value| value.parse::<i32>().ok())
                .expect("tonehead x position")
        })
        .collect::<Vec<_>>();
    assert_eq!(tonehead_xs.len(), 3, "{svg}");
    assert!(
        tonehead_xs.windows(2).all(|pair| pair[1] - pair[0] >= 14),
        "overlapping C-tone bounds must have a 2-unit gap after stroke extents: {tonehead_xs:?}\n{svg}"
    );
    let barline = svg
        .lines()
        .find(|line| line.contains("id=\"barline-1\""))
        .expect("final measure barline");
    assert!(
        barline.contains(" 27.5V108.5"),
        "barline must span the staff only: {barline}"
    );
    let barline_x = barline
        .split(" d=\"M")
        .nth(1)
        .and_then(|value| value.split(' ').next())
        .and_then(|value| value.parse::<i32>().ok())
        .expect("barline x position");
    assert!(
        barline_x - tonehead_xs[2] >= 16,
        "barline should clear the final tonehead and flags: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn pads_events_from_both_sides_of_measure_barlines() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("barline-padding.musicxml");
    let second_measure = r#"
    <measure number="2">
            <note><rest/><duration>1</duration><voice>1</voice><type>64th</type></note>
    </measure>"#;
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<divisions>1</divisions>", "<divisions>16</divisions>")
        .replace(
            original_note,
            "<note><rest/><duration>1</duration><voice>1</voice><type>64th</type></note>",
        )
        .replace("  </part>", &format!("{second_measure}\n  </part>"));
    fs::write(&input, fixture).expect("write barline-padding score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "barline-padding score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("barline-padding.svg")).expect("read SVG");
    let rest_xs = svg
        .lines()
        .filter(|line| line.contains("class=\"rest\""))
        .map(|line| {
            line.split("transform=\"translate(")
                .nth(1)
                .and_then(|value| value.split_whitespace().next())
                .and_then(|value| value.parse::<i32>().ok())
                .expect("tonehead x position")
        })
        .collect::<Vec<_>>();
    assert_eq!(rest_xs.len(), 2, "{svg}");
    let barline_x = svg
        .lines()
        .find(|line| line.contains("id=\"barline-1\""))
        .and_then(|line| line.split(" d=\"M").nth(1))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<i32>().ok())
        .expect("first barline x position");
    let before_gap = f64::from(barline_x) - (f64::from(rest_xs[0]) + 13.845);
    let after_gap = f64::from(rest_xs[1]) + 1.155 - f64::from(barline_x);
    assert!(
        before_gap >= 5.0 && after_gap >= 5.0,
        "events should have at least 5 SVG units of padding on both sides of the barline (before={before_gap}, after={after_gap}): {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn engraves_supported_note_durations_and_only_the_two_dotted_forms() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("duration-forms.musicxml");
    let forms = [
        ("whole", 64, false),
        ("half", 32, false),
        ("quarter", 16, false),
        ("eighth", 8, false),
        ("16th", 4, false),
        ("32nd", 2, false),
        ("64th", 1, false),
        ("quarter", 24, true),
        ("half", 48, true),
    ];
    let events = forms
        .iter()
        .map(|(note_type, ticks, dotted)| {
            let dot = if *dotted { "<dot/>" } else { "" };
            format!(
                "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>{ticks}</duration><voice>1</voice><type>{note_type}</type>{dot}</note>\n"
            )
        })
        .collect::<String>();
    let source = include_str!("fixtures/minimal.musicxml");
    let note_start = source.find("      <note>").expect("canonical note start");
    let note_end = source.find("      </note>").expect("canonical note end") + 13;
    let fixture = format!("{}{}{}", &source[..note_start], events, &source[note_end..])
        .replace("<divisions>1</divisions>", "<divisions>16</divisions>");
    fs::write(&input, fixture).expect("write duration score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "duration score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("duration-forms.svg")).expect("read SVG");
    assert_eq!(
        svg.matches("class=\"tonehead\"").count(),
        forms.len(),
        "{svg}"
    );
    assert_eq!(svg.matches("class=\"stem\"").count(), 8, "{svg}");
    assert_eq!(svg.matches("class=\"flag\"").count(), 4, "{svg}");
    assert_eq!(svg.matches("class=\"duration-mark\"").count(), 6, "{svg}");
    let dotted_half_marks = (0..3)
        .map(|mark| {
            let id = format!("id=\"duration-mark-8-{mark}\"");
            let path = svg
                .lines()
                .find(|line| line.contains(&id))
                .expect("dotted-half duration mark");
            let coordinates = path
                .split(" d=\"M")
                .nth(1)
                .and_then(|value| value.split('a').next())
                .expect("duration-mark path coordinates");
            let (x, y) = coordinates
                .split_once(' ')
                .expect("duration-mark x/y coordinates");
            (
                x.parse::<i32>().expect("dot x"),
                y.parse::<i32>().expect("dot y"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(dotted_half_marks[0].0, dotted_half_marks[1].0);
    assert_eq!(
        dotted_half_marks[2].1, dotted_half_marks[1].1,
        "the dotted-half period must align with the lower colon dot"
    );
    let horizontal_gap = dotted_half_marks[2].0 - dotted_half_marks[1].0;
    let vertical_gap = dotted_half_marks[1].1 - dotted_half_marks[0].1;
    assert_eq!(
        horizontal_gap, vertical_gap,
        "horizontal period spacing should match the vertical colon-dot spacing"
    );
    let dotted_quarter_mark = svg
        .lines()
        .find(|line| line.contains("id=\"duration-mark-7-0\""))
        .expect("dotted-quarter duration mark");
    let dotted_quarter_y = dotted_quarter_mark
        .split(" d=\"M")
        .nth(1)
        .and_then(|value| value.split('a').next())
        .and_then(|coordinates| coordinates.split_once(' ').map(|(_, y)| y))
        .and_then(|value| value.parse::<i32>().ok())
        .expect("dotted-quarter dot y position");
    assert_eq!(
        dotted_quarter_y, dotted_half_marks[1].1,
        "dotted-quarter dot must align with the lower colon dot"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn stems_extend_away_from_the_center_staff_line() {
    for octave in [4, 5, 9] {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join(format!("centered-stem-{octave}.musicxml"));
        let fixture = include_str!("fixtures/minimal.musicxml")
            .replace("<octave>4</octave>", &format!("<octave>{octave}</octave>"));
        fs::write(&input, fixture).expect("write centered-stem score");

        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "octave {octave} score failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let svg = fs::read_to_string(directory.join(format!("centered-stem-{octave}.svg")))
            .expect("read SVG");
        let tonehead = svg
            .lines()
            .find(|line| line.contains("id=\"tonehead\""))
            .expect("tonehead path");
        let tonehead_position = tonehead
            .split("transform=\"translate(")
            .nth(1)
            .and_then(|value| value.split(')').next())
            .map(|value| {
                value
                    .split_whitespace()
                    .map(|coordinate| coordinate.parse::<i32>().expect("tonehead coordinate"))
                    .collect::<Vec<_>>()
            })
            .expect("tonehead position");
        let stem = svg
            .lines()
            .find(|line| line.contains("id=\"stem\""))
            .expect("stem path");
        let stem_geometry = stem
            .split(" d=\"M")
            .nth(1)
            .and_then(|value| value.split('"').next())
            .expect("stem geometry");
        let (stem_start, stem_end) = stem_geometry.split_once('V').expect("stem endpoint");
        let stem_start = stem_start
            .split_whitespace()
            .map(|coordinate| coordinate.parse::<i32>().expect("stem start coordinate"))
            .collect::<Vec<_>>();
        let stem_end_y = stem_end.parse::<i32>().expect("stem end y");
        assert!(
            (stem_start[0] - tonehead_position[0] - 6).abs() == 0,
            "octave {octave} stem should be centered over its tonehead: {svg}"
        );
        let (expected_start_y, should_extend_up) = if octave <= 4 {
            (tonehead_position[1] - 1, true)
        } else {
            (tonehead_position[1] + 19, false)
        };
        assert_eq!(
            stem_start[1], expected_start_y,
            "octave {octave} stem should attach on the correct side of its tonehead: {svg}"
        );
        assert_eq!(
            stem_end_y < stem_start[1],
            should_extend_up,
            "octave {octave} stem should extend away from the center staff line: {svg}"
        );
        let tonehead_top = f64::from(tonehead_position[1] + 1);
        let tonehead_bottom = f64::from(tonehead_position[1] + 17);
        let stem_edge = if should_extend_up {
            f64::from(stem_start[1]) + 0.75
        } else {
            f64::from(stem_start[1]) - 0.75
        };
        let stem_gap = if should_extend_up {
            tonehead_top - stem_edge
        } else {
            stem_edge - tonehead_bottom
        };
        assert_eq!(stem_gap, 1.25, "stem gap should match on both sides: {svg}");
        let view_top = svg
            .lines()
            .next()
            .and_then(|line| line.split("viewBox=\"0 ").nth(1))
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse::<i32>().ok())
            .expect("viewBox top");
        assert!(
            view_top <= stem_end_y,
            "octave {octave} stem must fit in the viewBox: {svg}"
        );

        fs::remove_dir_all(directory).expect("remove test directory");
    }
}

#[test]
fn chord_stem_direction_follows_the_majority_side_of_the_center_line() {
    for (octaves, should_extend_up, attachment_octave) in [
        (&[5, 6, 4][..], false, 4),
        (&[4, 3, 5][..], true, 5),
        (&[5, 4][..], true, 5),
    ] {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join("chord-stem-direction.musicxml");
        let steps = ["C", "C", "C"];
        let notes = octaves
            .iter()
            .zip(steps)
            .enumerate()
            .map(|(index, (octave, step))| {
                let chord_marker = if index == 0 { "" } else { "<chord/>" };
                format!(
                    "      <note>{chord_marker}<pitch><step>{step}</step><octave>{octave}</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>"
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let first_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
        let fixture = include_str!("fixtures/minimal.musicxml").replace(first_note, &notes);
        fs::write(&input, fixture).expect("write chord score");

        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "chord {octaves:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let svg = fs::read_to_string(directory.join("chord-stem-direction.svg")).expect("read SVG");
        let tonehead_y = svg
            .lines()
            .filter(|line| line.contains("class=\"tonehead\""))
            .filter_map(|line| {
                let octave = line
                    .split("data-octave=\"")
                    .nth(1)?
                    .split('\"')
                    .next()?
                    .parse::<u8>()
                    .ok()?;
                let y = line
                    .split("transform=\"translate(")
                    .nth(1)?
                    .split_whitespace()
                    .nth(1)?
                    .split(')')
                    .next()?
                    .parse::<i32>()
                    .ok()?;
                Some((octave, y))
            })
            .collect::<Vec<_>>();
        assert_eq!(tonehead_y.len(), octaves.len(), "{svg}");
        let anchor_y = tonehead_y
            .iter()
            .find(|(octave, _)| *octave == attachment_octave)
            .map(|(_, y)| *y)
            .expect("outer tonehead for stem attachment");
        let stem = svg
            .lines()
            .find(|line| line.contains("id=\"stem\""))
            .expect("chord stem");
        let stem_geometry = stem
            .split(" d=\"M")
            .nth(1)
            .and_then(|value| value.split('\"').next())
            .expect("stem geometry");
        let (stem_start, stem_end) = stem_geometry.split_once('V').expect("stem endpoint");
        let stem_start_y = stem_start
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<i32>().ok())
            .expect("stem start y");
        let stem_end_y = stem_end.parse::<i32>().expect("stem end y");
        let expected_start_y = if should_extend_up {
            anchor_y - 1
        } else {
            anchor_y + 19
        };
        assert_eq!(stem_start_y, expected_start_y, "{svg}");
        assert_eq!(stem_end_y < stem_start_y, should_extend_up, "{svg}");

        fs::remove_dir_all(directory).expect("remove test directory");
    }
}

#[test]
fn engraves_stacked_chord_tones_as_one_event_with_shared_rhythm_geometry() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("stacked-chord.musicxml");
    let source = include_str!("fixtures/minimal.musicxml");
    let first_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let chord = "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>\n      <note><chord/><pitch><step>G</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>";
    let fixture = source.replace(first_note, chord);
    fs::write(&input, fixture).expect("write stacked chord score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "stacked chord failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("stacked-chord.svg")).expect("read SVG");
    let tonehead_transforms = svg
        .lines()
        .filter(|line| line.contains("class=\"tonehead\"") && line.contains("duration=\"half\""))
        .map(|line| {
            let start = line.find("translate(").expect("tonehead transform") + 10;
            let end = line[start..].find(')').expect("transform end") + start;
            line[start..end].to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(tonehead_transforms.len(), 2, "{svg}");
    assert_eq!(
        tonehead_transforms[0].split_whitespace().next(),
        tonehead_transforms[1].split_whitespace().next()
    );
    assert_eq!(svg.matches("class=\"stem\"").count(), 1, "{svg}");
    assert_eq!(svg.matches("class=\"duration-mark\"").count(), 2, "{svg}");
    assert_eq!(svg.matches("class=\"chord-bracket\"").count(), 2, "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn canonical_score_covers_supported_mvp_content_in_self_contained_svg() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("canonical.musicxml");
    fs::write(&input, include_str!("fixtures/canonical.musicxml")).expect("write canonical score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "canonical score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("canonical.svg")).expect("read SVG");
    assert_eq!(svg.matches("id=\"staff-line-").count(), 3, "{svg}");
    assert_measure_barline_layout(&svg);
    assert!(svg.contains("id=\"octave-4\""), "{svg}");
    assert!(svg.contains("id=\"octave-5\""), "{svg}");
    assert!(svg.contains("id=\"meter-numerator\""), "{svg}");
    assert_filled_glyph_paths(&svg);
    assert!(svg.contains("id=\"meter-denominator\""), "{svg}");
    assert_filled_glyph_paths(&svg);
    assert_eq!(svg.matches("class=\"tonehead\"").count(), 24, "{svg}");
    assert_eq!(svg.matches("class=\"rest\"").count(), 10, "{svg}");
    assert_eq!(svg.matches("class=\"stem\"").count(), 21, "{svg}");
    assert_eq!(svg.matches("class=\"duration-mark\"").count(), 6, "{svg}");
    assert_eq!(svg.matches("class=\"flag\"").count(), 1, "{svg}");
    assert_eq!(svg.matches("class=\"beam\"").count(), 3, "{svg}");
    assert_eq!(svg.matches("class=\"beam-hook\"").count(), 1, "{svg}");
    assert!(
        svg.contains("id=\"beam-7-1\""),
        "16th/32nd primary beam: {svg}"
    );
    assert!(
        svg.contains("id=\"beam-7-2\""),
        "16th/32nd secondary beam: {svg}"
    );
    assert!(
        !svg.contains("id=\"flag-6\""),
        "beamed 16th should not be flagged: {svg}"
    );
    assert!(
        !svg.contains("id=\"flag-7\""),
        "hooked 32nd level should not be a flag: {svg}"
    );
    assert!(
        svg.contains("id=\"beam-hook-7-3\""),
        "32nd third-level hook: {svg}"
    );
    assert_eq!(svg.matches("class=\"tie\"").count(), 1, "{svg}");
    assert_eq!(svg.matches("class=\"chord-bracket\"").count(), 4, "{svg}");
    assert_eq!(
        svg.matches("id=\"ledger-line-below-event-").count(),
        2,
        "{svg}"
    );
    assert_eq!(
        svg.matches("id=\"ledger-line-above-event-").count(),
        2,
        "{svg}"
    );
    for value in 0..=11 {
        assert!(
            svg.contains(&format!("data-tone=\"{value}\"")),
            "missing Tone {value}: {svg}"
        );
    }
    for duration in ["whole", "half", "quarter", "eighth", "16th", "32nd", "64th"] {
        assert!(
            svg.contains(&format!("data-duration=\"{duration}\"")),
            "missing {duration}: {svg}"
        );
    }
    assert!(svg.contains("data-tone=\"0\" data-octave=\"0\""), "{svg}");
    assert!(svg.contains("data-tone=\"9\" data-octave=\"9\""), "{svg}");
    assert!(
        svg.contains("data-event=\"0\" data-tone-index=\"0\" data-tone=\"11\" data-octave=\"3\""),
        "lowered written pitch plus the retained +1 transpose should render as B3: {svg}"
    );
    for octave in 0..=9 {
        assert!(
            svg.contains(&format!("data-octave=\"{octave}\"")),
            "missing octave {octave}: {svg}"
        );
    }

    let first_chord_tones = svg
        .lines()
        .filter(|line| line.contains("class=\"tonehead\"") && line.contains("data-event=\"28\""))
        .collect::<Vec<_>>();
    assert_eq!(first_chord_tones.len(), 2, "{svg}");
    let transforms = first_chord_tones
        .iter()
        .map(|line| {
            let start = line
                .find("transform=\"translate(")
                .expect("tonehead transform")
                + 19;
            let end = line[start..].find(')').expect("transform end") + start;
            line[start..end].to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        transforms[0].split_whitespace().next(),
        transforms[1].split_whitespace().next()
    );
    assert_ne!(
        transforms[0].split_whitespace().nth(1),
        transforms[1].split_whitespace().nth(1)
    );

    let view_box = svg
        .split("viewBox=\"0 ")
        .nth(1)
        .and_then(|value| value.split('"').next())
        .expect("viewBox")
        .split_whitespace()
        .collect::<Vec<_>>();
    let view_width = view_box[1].parse::<i32>().expect("viewBox width");
    let staff_end = svg
        .lines()
        .find(|line| line.contains("id=\"staff-line-1\""))
        .and_then(|line| line.split('H').nth(1))
        .and_then(|value| value.split('"').next())
        .and_then(|value| value.parse::<i32>().ok())
        .expect("staff width");
    assert!(
        view_width > staff_end,
        "viewBox must contain the full staff width: {svg}"
    );
    assert!(!svg.contains("<text"), "SVG glyphs must not rely on fonts");
    assert!(
        !svg.contains("<image") && !svg.contains("href="),
        "SVG must be self-contained paths"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_no_transposition_fixture_with_correct_measure_barlines() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("no-transposition.musicxml");
    fs::write(&input, include_str!("fixtures/no-transposition.musicxml"))
        .expect("write no-transposition fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "no-transposition score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("no-transposition.svg")).expect("read SVG");
    assert_measure_barline_layout(&svg);

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_malformed_chord_structures_and_mismatched_event_semantics() {
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let cases = [
        (
            "chord-without-predecessor",
            "<note><chord/><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>",
            1,
            "preceding pitched event",
        ),
        (
            "rest-chord",
            "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>\n      <note><chord/><rest/><duration>1</duration><voice>1</voice><type>quarter</type></note>",
            1,
            "rest chords are unsupported",
        ),
        (
            "chord-after-rest",
            "<note><rest/><duration>1</duration><voice>1</voice><type>quarter</type></note>\n      <note><chord/><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>",
            1,
            "preceding pitched event",
        ),
        (
            "mismatched-chord-duration",
            "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>\n      <note><chord/><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>",
            1,
            "duration must match",
        ),
        (
            "mismatched-chord-beams",
            "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type><beam>forward hook</beam></note>\n      <note><chord/><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type></note>",
            2,
            "beam declarations must match",
        ),
        (
            "mismatched-chord-voice",
            "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>\n      <note><chord/><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type></note>",
            1,
            "voice",
        ),
        (
            "chord-tie-stop-without-start",
            "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>\n      <note><chord/><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><tie type=\"stop\"/><voice>1</voice><type>quarter</type></note>",
            1,
            "tie stop has no preceding tie start",
        ),
    ];

    for (name, notes, divisions, expected_diagnostic) in cases {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join(format!("{name}.musicxml"));
        let fixture = include_str!("fixtures/minimal.musicxml")
            .replace(original_note, notes)
            .replace(
                "<divisions>1</divisions>",
                &format!("<divisions>{divisions}</divisions>"),
            );
        fs::write(&input, fixture).expect("write malformed chord score");

        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(!output.status.success(), "{name} was accepted");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains(expected_diagnostic),
            "{name}: {diagnostic}"
        );
        assert!(diagnostic.contains("XML path"), "{name}: {diagnostic}");
        assert!(
            !directory.join(format!("{name}.svg")).exists(),
            "{name} emitted partial SVG"
        );

        fs::remove_dir_all(directory).expect("remove test directory");
    }
}

#[test]
fn separates_same_octave_chord_tones_and_clears_the_preceding_event() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("same-octave-chord.musicxml");
    let source = include_str!("fixtures/minimal.musicxml");
    let first_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let notes = "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type></note>\n      <note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>\n      <note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>";
    let fixture = source
        .replace(first_note, notes)
        .replace("<divisions>1</divisions>", "<divisions>2</divisions>");
    fs::write(&input, fixture).expect("write same-octave chord score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "same-octave chord failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("same-octave-chord.svg")).expect("read SVG");
    let chord_tones = svg
        .lines()
        .filter(|line| line.contains("class=\"tonehead\"") && line.contains("data-event=\"1\""))
        .collect::<Vec<_>>();
    assert_eq!(chord_tones.len(), 2, "{svg}");
    let transforms = chord_tones
        .iter()
        .map(|line| {
            let marker = "transform=\"translate(";
            let start = line.find(marker).expect("tonehead transform") + marker.len();
            let end = line[start..].find(')').expect("transform end") + start;
            line[start..end]
                .split_whitespace()
                .map(|value| value.parse::<i32>().expect("transform coordinate"))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_ne!(
        transforms[0][0], transforms[1][0],
        "same-lane heads must not overlap: {svg}"
    );
    assert_eq!(transforms[1][0] - transforms[0][0], 12, "{svg}");
    assert_eq!(
        transforms[0][1], transforms[1][1],
        "same octave lane: {svg}"
    );
    let first_event_x = svg
        .lines()
        .find(|line| line.contains("class=\"tonehead\"") && line.contains("data-event=\"0\""))
        .and_then(|line| line.split("translate(").nth(1))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<i32>().ok())
        .expect("preceding event x coordinate");
    let bracket_left_x = svg
        .lines()
        .find(|line| line.contains("class=\"chord-bracket\"") && line.contains("left"))
        .and_then(|line| line.split("d=\"M").nth(1))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<i32>().ok())
        .expect("chord bracket x coordinate");
    assert!(
        bracket_left_x > first_event_x + 10,
        "chord bracket must clear the preceding event: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn positions_rests_relative_to_the_center_staff_line() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("rest-positions.musicxml");
    let source = include_str!("fixtures/minimal.musicxml")
        .replace("<divisions>1</divisions>", "<divisions>16</divisions>");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let rest_notes = [
        ("whole", 64),
        ("half", 32),
        ("quarter", 16),
        ("eighth", 8),
        ("16th", 4),
        ("32nd", 2),
        ("64th", 1),
    ]
    .into_iter()
    .map(|(duration, units)| {
        format!(
            "<note><rest/><duration>{units}</duration><voice>1</voice><type>{duration}</type></note>"
        )
    })
    .collect::<Vec<_>>()
    .join("\n      ");
    let fixture = source.replace(original_note, &format!("      {rest_notes}"));
    fs::write(&input, fixture).expect("write rest-position score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "rest-position score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("rest-positions.svg")).expect("read SVG");
    let rest_lines = svg
        .lines()
        .filter(|line| line.contains("class=\"rest\""))
        .collect::<Vec<_>>();
    assert_eq!(rest_lines.len(), rest_notes.lines().count(), "{svg}");

    for (line, (duration, expected_y)) in rest_lines.iter().zip([
        ("whole", 63.66),
        ("half", 56.34),
        ("quarter", 59.0),
        ("eighth", 59.0),
        ("16th", 59.0),
        ("32nd", 59.0),
        ("64th", 59.0),
    ]) {
        let actual_y = line
            .split("transform=\"translate(")
            .nth(1)
            .and_then(|value| value.split_whitespace().nth(1))
            .and_then(|value| value.strip_suffix(")\""))
            .and_then(|value| value.parse::<f64>().ok())
            .expect("rest y translation");
        assert!(
            (actual_y - expected_y).abs() < 0.001,
            "{duration} rest should align with the center staff line at y={expected_y}, got y={actual_y}: {line}"
        );
    }

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_simple_numeric_meter_and_rests() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("meter-and-rest.musicxml");
    let source = include_str!("fixtures/minimal.musicxml");
    let fixture = source
        .replace(
            "<attributes><divisions>1</divisions></attributes>",
            "<attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>",
        )
        .replace(
            "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>",
            "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>\n      <note><rest/><duration>1</duration><voice>1</voice><type>quarter</type></note>",
        );
    fs::write(&input, fixture).expect("write meter and rest score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "meter and rest score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("meter-and-rest.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"tonehead\"").count(), 1, "{svg}");
    assert_eq!(svg.matches("class=\"rest\"").count(), 1, "{svg}");
    assert!(
        svg.contains("class=\"rest\"") && svg.contains("fill-rule=\"nonzero\""),
        "{svg}"
    );
    assert!(
        svg.contains("d=\"M5.775 10.110C6.255 10.710"),
        "quarter rest should use the Bravura path: {svg}"
    );
    assert!(svg.contains("id=\"meter-numerator\""), "{svg}");
    assert!(svg.contains("id=\"meter-denominator\""), "{svg}");
    assert!(
        svg.contains("id=\"meter-numerator\" transform=\"translate(68 30) scale(2)\""),
        "time-signature numerator should fill the octave 5/6 lanes: {svg}"
    );
    assert!(
        svg.contains("id=\"meter-denominator\" transform=\"translate(68 70) scale(2)\""),
        "time-signature denominator should fill the octave 3/4 lanes: {svg}"
    );
    assert!(!svg.contains("<text"), "SVG glyphs must not rely on fonts");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn connects_equal_pitch_ties_across_measure_boundaries() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("cross-measure-tie.musicxml");
    let source = include_str!("fixtures/minimal.musicxml");
    let first_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let first_tie = "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><tie type=\"start\"/><voice>1</voice><type>quarter</type></note>";
    let second_measure = "\n    <measure number=\"2\"><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><tie type=\"stop\"/><voice>1</voice><type>quarter</type></note></measure>";
    let fixture = source
        .replace(first_note, first_tie)
        .replace("  </part>", &format!("{second_measure}\n  </part>"));
    fs::write(&input, fixture).expect("write tied score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "tied score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("cross-measure-tie.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"tonehead\"").count(), 2, "{svg}");
    assert_eq!(svg.matches("class=\"tie\"").count(), 1, "{svg}");
    assert!(svg.contains("class=\"tie\" d=\"M"), "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn connects_each_same_octave_chord_tie_to_its_tonehead_offset() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("same-octave-chord-ties.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let notes = "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><tie type=\"start\"/><voice>1</voice><type>quarter</type></note>\n      <note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><tie type=\"start\"/><voice>1</voice><type>quarter</type></note>\n      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><tie type=\"stop\"/><voice>1</voice><type>quarter</type></note>\n      <note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><tie type=\"stop\"/><voice>1</voice><type>quarter</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml").replace(original_note, notes);
    fs::write(&input, fixture).expect("write tied chord score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "tied chord failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("same-octave-chord-ties.svg")).expect("read SVG");
    let stop_tone_xs = svg
        .lines()
        .filter(|line| line.contains("class=\"tonehead\"") && line.contains("data-event=\"1\""))
        .map(|line| {
            let marker = "transform=\"translate(";
            let start = line.find(marker).expect("tonehead transform") + marker.len();
            line[start..]
                .split_whitespace()
                .next()
                .expect("tonehead x coordinate")
                .parse::<i32>()
                .expect("integer tonehead x coordinate")
        })
        .collect::<Vec<_>>();
    let tie_stops = svg
        .lines()
        .filter(|line| line.contains("class=\"tie\""))
        .map(|line| {
            let tone_index = line
                .split("id=\"tie-1-")
                .nth(1)
                .expect("tie tone index")
                .split('"')
                .next()
                .expect("tie tone index")
                .parse::<usize>()
                .expect("integer tie tone index");
            let path = line.split(" d=\"").nth(1).expect("tie path data");
            let end_x = path
                .split('C')
                .nth(1)
                .expect("tie curve")
                .split_whitespace()
                .nth(4)
                .expect("tie endpoint x coordinate")
                .parse::<i32>()
                .expect("integer tie endpoint x coordinate");
            (tone_index, end_x)
        })
        .collect::<Vec<_>>();
    assert_eq!(tie_stops.len(), stop_tone_xs.len(), "{svg}");
    for (tone_index, end_x) in tie_stops {
        assert_eq!(end_x, stop_tone_xs[tone_index] + 5, "{svg}");
    }

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn preserves_explicit_musicxml_eighth_note_beams() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("beamed-eighths.musicxml");
    let source = include_str!("fixtures/minimal.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let notes = "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type><beam number=\"1\">begin</beam></note>\n      <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type><beam number=\"1\">end</beam></note>";
    let fixture = source
        .replace("<divisions>1</divisions>", "<divisions>2</divisions>")
        .replace(original_note, notes);
    fs::write(&input, fixture).expect("write beamed score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "beamed score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("beamed-eighths.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"beam\"").count(), 1, "{svg}");
    assert_eq!(svg.matches("class=\"flag\"").count(), 0, "{svg}");
    let beam = svg
        .lines()
        .find(|line| line.contains("class=\"beam\""))
        .expect("beam path");
    assert!(
        beam.contains("d=\"M98.75 39.00L117.25 39.00L117.25 42.00L98.75 42.00Z\""),
        "flat beam should meet the outer stem borders with a flush top edge: {beam}"
    );
    assert!(
        beam.contains("fill=\"#171717\"") && !beam.contains("stroke="),
        "{beam}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_multi_octave_beaming_fixture_geometry() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("multi-octave-beaming.musicxml");
    fs::write(
        &input,
        include_str!("fixtures/multi-octave-beaming.musicxml"),
    )
    .expect("write multi-octave beaming fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "multi-octave beaming fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("multi-octave-beaming.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"tonehead\"").count(), 17, "{svg}");
    assert_eq!(svg.matches("class=\"stem\"").count(), 16, "{svg}");
    assert_eq!(svg.matches("class=\"beam\"").count(), 9, "{svg}");
    assert_eq!(svg.matches("class=\"beam-hook\"").count(), 1, "{svg}");
    assert_eq!(svg.matches("class=\"flag\"").count(), 0, "{svg}");

    let expected_stems = [
        (0, 68, 97),
        (1, 48, 77),
        (2, 88, 117),
        (3, 68, 97),
        (4, 68, 97),
        (5, 88, 117),
        (6, 108, 137),
        (7, 48, 77),
        (8, 28, -1),
        (9, 68, 39),
        (10, 68, 97),
        (11, 48, 77),
        (12, 88, 59),
        (13, 68, 39),
        (14, 68, 39),
        (15, 88, 59),
    ];
    let rendered_stems = (0..expected_stems.len())
        .map(|event_index| {
            let stem_id = if event_index == 0 {
                "id=\"stem\"".to_owned()
            } else {
                format!("id=\"stem-{event_index}\"")
            };
            let line = svg
                .lines()
                .find(|line| line.contains(&stem_id))
                .expect("stem path");
            let path = line
                .split(" d=\"M")
                .nth(1)
                .and_then(|value| value.split('"').next())
                .expect("stem path data");
            let (start, end) = path.split_once('V').expect("stem endpoints");
            let start_y = start
                .split_whitespace()
                .nth(1)
                .and_then(|value| value.parse::<i32>().ok())
                .expect("stem start y");
            let end_y = end.parse::<i32>().expect("stem end y");
            (event_index, start_y, end_y)
        })
        .collect::<Vec<_>>();

    assert!(
        rendered_stems == expected_stems,
        "multi-octave beamed groups should use the agreed group-wide stem directions"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn angled_beams_are_flush_parallelograms_and_hooks_follow_the_beam_slope() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("multi-octave-beaming.musicxml");
    fs::write(
        &input,
        include_str!("fixtures/multi-octave-beaming.musicxml"),
    )
    .expect("write multi-octave beaming fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "multi-octave beaming fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("multi-octave-beaming.svg")).expect("read SVG");
    let path_points = |path_id: &str| {
        let line = svg
            .lines()
            .find(|line| line.contains(&format!("id=\"{path_id}\"")))
            .expect("beam path");
        assert!(
            line.contains("fill=\"#171717\"") && !line.contains("stroke="),
            "beam should be a filled parallelogram: {line}"
        );
        let path = line
            .split(" d=\"")
            .nth(1)
            .and_then(|value| value.split('\"').next())
            .and_then(|value| value.strip_prefix('M'))
            .and_then(|value| value.strip_suffix('Z'))
            .expect("closed parallelogram path");
        path.split('L')
            .map(|point| {
                let (x, y) = point.split_once(' ').expect("path point");
                (
                    x.parse::<f64>().expect("point x"),
                    y.parse::<f64>().expect("point y"),
                )
            })
            .collect::<Vec<_>>()
    };
    let stem_anchors = (0..16)
        .map(|event_index| {
            let stem_id = if event_index == 0 {
                "id=\"stem\"".to_owned()
            } else {
                format!("id=\"stem-{event_index}\"")
            };
            let line = svg
                .lines()
                .find(|line| line.contains(&stem_id))
                .expect("beam stem");
            let path = line
                .split(" d=\"M")
                .nth(1)
                .and_then(|value| value.split('\"').next())
                .expect("stem path");
            let (start, end) = path.split_once('V').expect("stem endpoints");
            let x = start
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<f64>().ok())
                .expect("stem x");
            let start_y = start
                .split_whitespace()
                .nth(1)
                .and_then(|value| value.parse::<f64>().ok())
                .expect("stem start y");
            let end_y = end.parse::<f64>().expect("stem end y");
            (x, start_y, end_y)
        })
        .collect::<Vec<_>>();

    for (end_event_index, level, edge_offset) in [
        (1, 1, 0.75),
        (3, 1, 0.75),
        (5, 1, -0.75),
        (7, 1, 0.75),
        (9, 1, 0.75),
        (11, 1, 0.75),
        (11, 2, 0.75),
        (13, 1, -0.75),
        (15, 1, 0.75),
    ] {
        let points = path_points(&format!("beam-{end_event_index}-{level}"));
        assert_eq!(points.len(), 4, "beam should have four corners: {points:?}");
        let (outer_start, outer_end, inner_end, inner_start) =
            (points[0], points[1], points[2], points[3]);
        assert!((outer_start.0 - inner_start.0).abs() < 0.01);
        assert!((outer_end.0 - inner_end.0).abs() < 0.01);

        let (start_x, _, start_end_y) = stem_anchors[end_event_index - 1];
        let (end_x, _, end_end_y) = stem_anchors[end_event_index];
        let stems_up = stem_anchors[end_event_index].2 < stem_anchors[end_event_index].1;
        let beam_direction = if stems_up { 1.0 } else { -1.0 };
        let level_offset = f64::from(level - 1) * 5.0 * beam_direction;
        let slope = (end_end_y - start_end_y) / (end_x - start_x);
        assert!((outer_start.0 - (start_x + edge_offset)).abs() < 0.01);
        assert!((outer_end.0 - (end_x + edge_offset)).abs() < 0.01);
        assert!((outer_start.1 - (start_end_y + level_offset)).abs() < 0.02);
        assert!((outer_end.1 - (end_end_y + level_offset)).abs() < 0.02);
        let outer_edge = (outer_end.0 - outer_start.0, outer_end.1 - outer_start.1);
        let inner_edge = (inner_end.0 - inner_start.0, inner_end.1 - inner_start.1);
        assert!((outer_edge.0 * inner_edge.1 - outer_edge.1 * inner_edge.0).abs() < 0.05);
        let inner_vector = (inner_start.0 - outer_start.0, inner_start.1 - outer_start.1);
        let perpendicular_thickness =
            (outer_edge.0 * inner_vector.1 - outer_edge.1 * inner_vector.0).abs()
                / outer_edge.0.hypot(outer_edge.1);
        assert!((perpendicular_thickness - 3.0).abs() < 0.02);
        assert!((slope - outer_edge.1 / outer_edge.0).abs() < 0.01);
    }

    let primary = path_points("beam-11-1");
    let hook = path_points("beam-hook-11-3");
    assert_eq!(
        hook.len(),
        4,
        "hook should also be a parallelogram: {hook:?}"
    );
    let stem_x = stem_anchors[11].0;
    assert!((hook[0].0 - (stem_x - 9.0 + 0.75)).abs() < 0.01);
    assert!((hook[1].0 - (stem_x + 0.75)).abs() < 0.01);
    let beam_slope = (primary[1].1 - primary[0].1) / (primary[1].0 - primary[0].0);
    let hook_slope = (hook[1].1 - hook[0].1) / (hook[1].0 - hook[0].0);
    assert!((beam_slope - hook_slope).abs() < 0.001);

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_unclosed_explicit_beam_groups_without_svg() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("unclosed-beam.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let beamed_note = "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type><beam number=\"1\">begin</beam></note>";
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<divisions>1</divisions>", "<divisions>2</divisions>")
        .replace(original_note, beamed_note);
    fs::write(&input, fixture).expect("write unclosed beam score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("beam group is missing an end marker"),
        "{diagnostic}"
    );
    assert!(!directory.join("unclosed-beam.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_forward_beam_hook_as_a_beam_segment() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("forward-hook.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<divisions>1</divisions>", "<divisions>2</divisions>")
        .replace(
            "<type>quarter</type>",
            "<type>eighth</type><beam>forward hook</beam>",
        );
    fs::write(&input, fixture).expect("write score with a forward beam hook");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "forward hook failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("forward-hook.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"beam-hook\"").count(), 1, "{svg}");
    assert_eq!(svg.matches("class=\"flag\"").count(), 0, "{svg}");
    let hook = svg
        .lines()
        .find(|line| line.contains("class=\"beam-hook\""))
        .expect("beam hook path");
    assert!(
        hook.contains("d=\"M98.75 39.00L106.25 39.00L106.25 42.00L98.75 42.00Z\""),
        "forward hook should use the same filled beam geometry: {hook}"
    );
    assert!(
        hook.contains("fill=\"#171717\"") && !hook.contains("stroke="),
        "{hook}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_smooth_32nd_flags_toward_a_down_stem_notehead() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("down-stem-32nd.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<divisions>1</divisions>", "<divisions>16</divisions>")
        .replace("<octave>4</octave>", "<octave>5</octave>")
        .replace(
            "<duration>1</duration><voice>1</voice><type>quarter</type>",
            "<duration>2</duration><voice>1</voice><type>32nd</type>",
        );
    fs::write(&input, fixture).expect("write down-stem 32nd score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "down-stem score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("down-stem-32nd.svg")).expect("read SVG");
    let flags = svg
        .lines()
        .filter(|line| line.contains("class=\"flag\""))
        .collect::<Vec<_>>();
    assert_eq!(flags.len(), 1, "one compound flag glyph per note: {svg}");
    let flag = flags[0];
    assert!(flag.contains("transform=\"translate(97.25 97)\""), "{flag}");
    assert!(flag.contains("fill-rule=\"nonzero\""), "{flag}");
    assert!(
        !flag.contains("rotate("),
        "Bravura down flag should use its own outline: {flag}"
    );
    let path = flag
        .split(" d=\"")
        .nth(1)
        .and_then(|value| value.split('\"').next())
        .expect("compound flag path");
    assert!(
        path.starts_with("M8.190-20.280"),
        "expected Bravura 32nd-down path: {flag}"
    );
    assert!(path.contains('C'), "flag contours should be curved: {flag}");
    assert_eq!(
        path.matches('M').count(),
        3,
        "base flag plus two ridges: {flag}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn applies_chromatic_transposition_before_mapping_pitch() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("transposed.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "</attributes>",
        "<transpose><chromatic>2</chromatic></transpose></attributes>",
    );
    fs::write(&input, fixture).expect("write transposed score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "transposed score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("transposed.svg")).expect("read SVG");
    assert!(
        svg.contains("data-tone=\"2\""),
        "C4 + 2 semitones should map to Tone 2"
    );
    assert!(
        svg.contains("transform=\"translate(92 69)\""),
        "result remains in octave 4"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn applies_chromatic_octave_change_and_double_transposition() {
    let cases = [
        ("chromatic-down-two", "<chromatic>-2</chromatic>", 10, 3),
        (
            "octave-change-up",
            "<chromatic>0</chromatic><octave-change>1</octave-change>",
            0,
            5,
        ),
        (
            "double-above",
            "<chromatic>0</chromatic><double above=\"yes\"/>",
            0,
            5,
        ),
        ("double-below", "<chromatic>0</chromatic><double/>", 0, 3),
    ];

    for (name, transpose_children, expected_tone, expected_octave) in cases {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join(format!("{name}.musicxml"));
        let fixture = include_str!("fixtures/minimal.musicxml").replace(
            "</attributes>",
            &format!("<transpose>{transpose_children}</transpose></attributes>"),
        );
        fs::write(&input, fixture).expect("write transposed score");

        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "{name} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let svg = fs::read_to_string(directory.join(format!("{name}.svg"))).expect("read SVG");
        assert!(
            svg.contains(&format!("data-tone=\"{expected_tone}\"")),
            "{name}: {svg}"
        );
        assert!(
            svg.contains(&format!("data-octave=\"{expected_octave}\"")),
            "{name}: {svg}"
        );

        fs::remove_dir_all(directory).expect("remove test directory");
    }
}

#[test]
fn maps_enharmonic_alterations_across_octave_boundaries() {
    let cases = [
        (
            "c-flat-crosses-down",
            "<step>C</step><octave>4</octave>",
            "<step>C</step><alter>-1</alter><octave>4</octave>",
            11,
            3,
        ),
        (
            "b-sharp-crosses-up",
            "<step>C</step><octave>4</octave>",
            "<step>B</step><alter>1</alter><octave>4</octave>",
            0,
            5,
        ),
        (
            "c-double-flat",
            "<step>C</step><octave>4</octave>",
            "<step>C</step><alter>-2</alter><octave>4</octave>",
            10,
            3,
        ),
        (
            "c-double-sharp",
            "<step>C</step><octave>4</octave>",
            "<step>C</step><alter>2</alter><octave>4</octave>",
            2,
            4,
        ),
    ];

    for (name, old_pitch, new_pitch, expected_tone, expected_octave) in cases {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join(format!("{name}.musicxml"));
        let fixture = include_str!("fixtures/minimal.musicxml").replace(old_pitch, new_pitch);
        fs::write(&input, fixture).expect("write pitch-boundary score");

        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "{name} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let svg = fs::read_to_string(directory.join(format!("{name}.svg"))).expect("read SVG");
        assert!(
            svg.contains(&format!("data-tone=\"{expected_tone}\"")),
            "{name}: {svg}"
        );
        assert!(
            svg.contains(&format!("data-octave=\"{expected_octave}\"")),
            "{name}: {svg}"
        );

        fs::remove_dir_all(directory).expect("remove test directory");
    }
}

#[test]
fn renders_all_ten_octave_lanes_with_ledger_lines_and_fitting_viewbox() {
    let expected_lane_y = [149, 129, 109, 89, 69, 49, 29, 9, -11, -31];
    let mut rendered_lane_y = [0; 10];
    for octave in 0..=9 {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join(format!("octave-{octave}.musicxml"));
        let fixture = include_str!("fixtures/minimal.musicxml")
            .replace("<octave>4</octave>", &format!("<octave>{octave}</octave>"));
        fs::write(&input, fixture).expect("write octave fixture");

        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "octave {octave} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let svg =
            fs::read_to_string(directory.join(format!("octave-{octave}.svg"))).expect("read SVG");
        assert!(svg.contains(&format!("data-octave=\"{octave}\"")), "{svg}");
        let tonehead = svg
            .lines()
            .find(|line| line.contains(&format!("data-octave=\"{octave}\"")))
            .expect("tonehead path");
        let lane_y = tonehead
            .split("transform=\"translate(")
            .nth(1)
            .and_then(|value| value.split(')').next())
            .and_then(|value| value.split_whitespace().nth(1))
            .and_then(|value| value.parse::<i32>().ok())
            .expect("tonehead y position");
        assert_eq!(lane_y, expected_lane_y[octave as usize], "octave {octave}");
        rendered_lane_y[octave as usize] = lane_y;
        for staff_line_y in [28, 68, 108] {
            assert!(
                svg.contains(&format!("d=\"M12 {staff_line_y}H")),
                "missing staff line at y={staff_line_y}: {svg}"
            );
        }
        assert!(
            svg.contains("id=\"octave-4\" transform=\"translate(26 69)\"")
                && svg.contains("id=\"octave-5\" transform=\"translate(26 49)\""),
            "octave 4/5 lane positions must match the left-side indicators: {svg}"
        );

        let expected_ledger_y = if octave <= 1 {
            Some(148)
        } else if octave >= 8 {
            Some(-12)
        } else {
            None
        };
        let ledger_lines = svg
            .lines()
            .filter(|line| line.contains("id=\"ledger-line-"))
            .collect::<Vec<_>>();
        assert_eq!(
            ledger_lines.len(),
            usize::from(expected_ledger_y.is_some()),
            "octave {octave}: {svg}"
        );
        if let Some(ledger_y) = expected_ledger_y {
            assert!(
                ledger_lines[0].contains(&format!(" {ledger_y}H")),
                "octave {octave} ledger line should be at y={ledger_y}: {}",
                ledger_lines[0]
            );
        }
        let adjacent_line_y = match octave {
            0..=1 => 148,
            2..=3 => 108,
            4..=5 => 68,
            6..=7 => 28,
            8..=9 => -12,
            _ => unreachable!(),
        };
        assert!(
            lane_y + 16 < adjacent_line_y || lane_y + 2 > adjacent_line_y,
            "octave {octave} C glyph intersects line at y={adjacent_line_y}"
        );
        assert!(svg.contains("viewBox=\"0 "), "missing viewBox: {svg}");

        fs::remove_dir_all(directory).expect("remove test directory");
    }

    let octave_4_center = rendered_lane_y[4] + 9;
    let octave_5_center = rendered_lane_y[5] + 9;
    assert_eq!(octave_4_center - octave_5_center, 20);
    assert_eq!((octave_4_center + octave_5_center) / 2, 68);
}

#[test]
fn rejects_normalized_pitch_outside_octaves_zero_through_nine() {
    let cases = [
        (
            "below-zero",
            "<step>C</step><octave>4</octave>",
            "<step>C</step><octave>0</octave>",
            "<transpose><chromatic>-1</chromatic></transpose>",
        ),
        (
            "above-nine",
            "<step>C</step><octave>4</octave>",
            "<step>B</step><alter>1</alter><octave>9</octave>",
            "",
        ),
    ];

    for (name, old_pitch, new_pitch, transpose) in cases {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join(format!("{name}.musicxml"));
        let mut fixture = include_str!("fixtures/minimal.musicxml").replace(old_pitch, new_pitch);
        if !transpose.is_empty() {
            fixture = fixture.replace("</attributes>", &format!("{transpose}</attributes>"));
        }
        fs::write(&input, fixture).expect("write out-of-range score");

        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(!output.status.success(), "{name} was accepted");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains("unsupported concert pitch octave"),
            "{diagnostic}"
        );
        assert!(!directory.join(format!("{name}.svg")).exists());

        fs::remove_dir_all(directory).expect("remove test directory");
    }
}

#[test]
fn rejects_transposition_integer_overflow_without_panicking() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("transposition-overflow.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "</attributes>",
        "<transpose><chromatic>0</chromatic><octave-change>2147483647</octave-change><double above=\"yes\"/></transpose></attributes>",
    );
    fs::write(&input, fixture).expect("write overflowing transposition");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("transposition is out of range"),
        "{diagnostic}"
    );
    assert!(!directory.join("transposition-overflow.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_unsupported_score_shape_without_creating_svg() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("unsupported.musicxml");
    fs::write(&input, "<score-timewise version=\"4.0\"></score-timewise>")
        .expect("write unsupported score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success(), "unsupported root was accepted");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unsupported"),
        "missing unsupported-shape diagnostic: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!directory.join("unsupported.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn reports_source_context_for_an_unsupported_musicxml_version() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("wrong-version.musicxml");
    let fixture =
        include_str!("fixtures/minimal.musicxml").replace("version=\"4.0\"", "version=\"3.0\"");
    let root_offset = fixture.find("<score-partwise").expect("root element");
    fs::write(&input, fixture).expect("write wrong-version score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success(), "unsupported version was accepted");
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains(input.to_str().expect("UTF-8 input path")),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains(&format!("byte {root_offset}, line 2, column 1")),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("XML path /score-partwise"),
        "{diagnostic}"
    );
    assert!(!directory.join("wrong-version.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_unhandled_note_attributes_without_creating_svg() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("extra-attribute.musicxml");
    let fixture =
        include_str!("fixtures/minimal.musicxml").replace("<note>", "<note print-object=\"no\">");
    fs::write(&input, fixture).expect("write unsupported score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success(),
        "unsupported note attribute was accepted"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unsupported"),
        "missing unsupported-shape diagnostic: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!directory.join("extra-attribute.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn processes_musicxml_doctype_and_internal_general_entity() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("doctype.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", "")
        .replace(
            "<score-partwise",
            "<!DOCTYPE score-partwise PUBLIC '-//MusicXML//DTD MusicXML 4.0 Partwise//EN' 'https://www.musicxml.org/dtds/partwise.dtd' [<!ENTITY label 'Music'>]>\n<score-partwise",
        )
        .replace(
            "<part-name>Music</part-name>",
            "<part-name>&label;</part-name>",
        );
    fs::write(&input, fixture).expect("write score with internal entity");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");

    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("doctype.svg").is_file());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn recursively_expands_internal_general_entities() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("nested-entities.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", "")
        .replace(
            "<score-partwise",
            "<!DOCTYPE score-partwise [<!ENTITY base 'Music'><!ENTITY label '&base;'>]>\n<score-partwise",
        )
        .replace("<part-name>Music</part-name>", "<part-name>&label;</part-name>");
    fs::write(&input, fixture).expect("write score with nested entities");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("nested-entities.svg").is_file());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn processes_internal_parameter_entities_in_the_internal_subset() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("parameter-entity.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", "")
        .replace(
            "<score-partwise",
            "<!DOCTYPE score-partwise [<!ENTITY % label_decl \"<!ENTITY label 'Music'>\">%label_decl;]>\n<score-partwise",
        )
        .replace("<part-name>Music</part-name>", "<part-name>&label;</part-name>");
    fs::write(&input, fixture).expect("write score with parameter entity");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("parameter-entity.svg").is_file());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn expands_parameter_entities_inside_entity_values() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("parameter-entity-value.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", "")
        .replace("version=\"4.0\"", "version=\"&ver;\"")
        .replace(
            "<score-partwise",
            "<!DOCTYPE score-partwise [<!ENTITY % digits '4.0'><!ENTITY ver '%digits;'>]>\n<score-partwise",
        );
    fs::write(&input, fixture).expect("write score with a parameter entity value");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("parameter-entity-value.svg").is_file());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn parses_markup_in_general_entity_replacement_text() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("markup-entity.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", "")
        .replace("<part-name>Music</part-name>", "&label;")
        .replace(
            "<score-partwise",
            "<!DOCTYPE score-partwise [<!ENTITY label '<part-name>Music</part-name>'>] >\n<score-partwise",
        );
    fs::write(&input, fixture).expect("write score with markup entity");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("markup-entity.svg").is_file());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_markup_entity_replacement_in_attribute_values() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("attribute-markup-entity.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", "")
        .replace("version=\"4.0\"", "version=\"&less;\"")
        .replace(
            "<score-partwise",
            "<!DOCTYPE score-partwise [<!ENTITY less '<'>]>\n<score-partwise",
        );
    fs::write(&input, fixture).expect("write score with attribute markup entity");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("markup in an entity replacement"),
        "{diagnostic}"
    );
    assert!(!directory.join("attribute-markup-entity.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn malformed_xml_reports_original_byte_and_normalized_line_column_and_path() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("bad-close.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("?>\n", "?>\r\n<!-- \u{1f3b5} -->\r\n")
        .replace("</pitch>", "</pitxh>");
    let failure_offset = fixture.find("</pitxh>").expect("mismatched tag") + "</pitxh>".len();
    let normalized = fixture.replace("\r\n", "\n").replace('\r', "\n");
    let normalized_offset = normalized.find("</pitxh>").expect("normalized tag") + "</pitxh>".len();
    let prefix = &normalized[..normalized_offset];
    let expected_line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let expected_column = prefix
        .rsplit('\n')
        .next()
        .expect("last line")
        .chars()
        .count()
        + 1;
    fs::write(&input, fixture).expect("write malformed score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains(input.to_str().expect("UTF-8 path")));
    assert!(
        diagnostic.contains(&format!("byte {failure_offset}")),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains(&format!("line {expected_line}, column {expected_column}")),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("/score-partwise/part[@id='P1']/measure[@number='1']/note/pitch"),
        "{diagnostic}"
    );
    assert!(!directory.join("bad-close.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn reports_depth_and_entity_expansion_limits_as_resource_errors() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");

    let depth_input = directory.join("too-deep.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", "");
    let wrapper_count = 59;
    let nested = format!(
        "{}{}{}",
        "<wrapper>".repeat(wrapper_count),
        fixture,
        "</wrapper>".repeat(wrapper_count)
    );
    fs::write(&depth_input, nested).expect("write deeply nested score");
    let depth_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&depth_input)
        .output()
        .expect("run CLI");
    assert!(!depth_output.status.success());
    assert!(String::from_utf8_lossy(&depth_output.stderr).contains("resource limit XML"));
    assert!(!directory.join("too-deep.svg").exists());

    let expansion_input = directory.join("entity-expansion.musicxml");
    let expanded_text = "x".repeat(8 * 1024 * 1024 + 1);
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", "")
        .replace(
            "<score-partwise",
            &format!(
                "<!DOCTYPE score-partwise [<!ENTITY large '{expanded_text}'>]>\n<score-partwise"
            ),
        )
        .replace(
            "<part-name>Music</part-name>",
            "<part-name>&large;</part-name>",
        );
    fs::write(&expansion_input, fixture).expect("write expansion-limit score");
    let expansion_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&expansion_input)
        .output()
        .expect("run CLI");
    assert!(!expansion_output.status.success());
    let diagnostic = String::from_utf8_lossy(&expansion_output.stderr);
    assert!(diagnostic.contains("resource limit XML"), "{diagnostic}");
    assert!(diagnostic.contains("aggregate limit"), "{diagnostic}");
    assert!(!directory.join("entity-expansion.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_exponential_entity_expansion_before_memory_exhaustion() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("exponential-entities.musicxml");
    let mut declarations = format!("<!ENTITY e0 '{}'>", "x".repeat(8 * 1024));
    for level in 1..=14 {
        let previous = level - 1;
        declarations.push_str(&format!("<!ENTITY e{level} '&e{previous};&e{previous};'>"));
    }
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", "")
        .replace(
            "<score-partwise",
            &format!("<!DOCTYPE score-partwise [{declarations}]>\n<score-partwise"),
        )
        .replace(
            "<part-name>Music</part-name>",
            "<part-name>&e14;</part-name>",
        );
    fs::write(&input, fixture).expect("write exponentially expanding score");

    let output = Command::new("sh")
        .args([
            "-c",
            "ulimit -v 65536; exec \"$1\" \"$2\"",
            "sh",
            env!("CARGO_BIN_EXE_domunor"),
        ])
        .arg(&input)
        .output()
        .expect("run CLI with a 64-MiB address-space limit");
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(diagnostic.contains("resource limit XML"), "{diagnostic}");
    assert!(diagnostic.contains("aggregate limit"), "{diagnostic}");
    assert!(!directory.join("exponential-entities.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_input_over_64_mib_as_a_resource_limit() {
    use std::fs::OpenOptions;

    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("too-large.musicxml");
    OpenOptions::new()
        .create(true)
        .write(true)
        .open(&input)
        .expect("create oversized input")
        .set_len(64 * 1024 * 1024 + 1)
        .expect("create sparse oversized input");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("resource limit XML"), "{diagnostic}");
    assert!(diagnostic.contains("64-MiB") || diagnostic.contains("67108864"));
    assert!(!directory.join("too-large.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_malformed_markup_declarations_in_the_internal_subset() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("malformed-dtd.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<!DOCTYPE score-partwise [<!ELEMENT score-partwise ???>] >\n",
    );
    fs::write(&input, fixture).expect("write malformed internal subset");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success(),
        "malformed internal DTD declaration was accepted"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("malformed XML"),
        "missing DTD grammar diagnostic: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!directory.join("malformed-dtd.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn accepts_valid_element_attribute_and_notation_declarations() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("valid-dtd.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<!DOCTYPE score-partwise [<?dtd instruction?><!ELEMENT score-partwise (part-list,part)><!ATTLIST score-partwise version CDATA #REQUIRED><!NOTATION sample PUBLIC '-//example//NOTATION sample//EN'><!ELEMENT foo:bar:baz ANY>] >\n",
    );
    fs::write(&input, fixture).expect("write valid internal subset");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "valid internal declarations were rejected: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("valid-dtd.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_malformed_doctype_external_identifiers() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("malformed-system-id.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<!DOCTYPE score-partwise SYSTEM not-quoted>\n",
    );
    fs::write(&input, fixture).expect("write malformed DOCTYPE");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success(),
        "malformed DOCTYPE external identifier was accepted"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("malformed XML"),
        "missing DOCTYPE grammar diagnostic"
    );
    assert!(!directory.join("malformed-system-id.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn accepts_gt_inside_quoted_external_entity_identifier_without_resolving_it() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("quoted-external-id.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<!DOCTYPE score-partwise [<!ENTITY ext SYSTEM 'https://example.invalid/a>b'>]>\n",
    );
    fs::write(&input, fixture).expect("write quoted external entity identifier");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "quoted external identifier should parse without resolution: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("quoted-external-id.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn applies_internal_attlist_default_attributes_before_musicxml_import() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("default-attributes.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", "")
        .replace("<score-partwise version=\"4.0\">", "<score-partwise>")
        .replace("<score-part id=\"P1\">", "<score-part id=\"  P1  \">")
        .replace("<part id=\"P1\">", "<part>")
        .replace(
            "<score-partwise>",
            "<!DOCTYPE score-partwise [<!ENTITY ver '4.0'><!ATTLIST score-partwise version CDATA '&ver;'><!ATTLIST score-part id ID #IMPLIED><!ATTLIST part id ID 'P1'>]>\n<score-partwise>",
        );
    fs::write(&input, fixture).expect("write score using default attributes");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "DTD defaults were not applied: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("default-attributes.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_malformed_xml_declaration_order_and_spacing() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    for (index, declaration) in [
        "<?xml version=\"1.0\" standalone=\"yes\" encoding=\"UTF-8\"?>",
        "<?xml version=\"1.0\"encoding=\"UTF-8\"?>",
    ]
    .into_iter()
    .enumerate()
    {
        let input = directory.join(format!("bad-declaration-{index}.musicxml"));
        let fixture = include_str!("fixtures/minimal.musicxml")
            .replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>", declaration);
        fs::write(&input, fixture).expect("write malformed XML declaration");

        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(
            !output.status.success(),
            "malformed XML declaration {index} was accepted"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("malformed XML"),
            "missing XML declaration diagnostic"
        );
        assert!(!directory
            .join(format!("bad-declaration-{index}.svg"))
            .exists());
    }

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn reports_independent_musicxml_value_errors_together() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("multiple-values.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<step>C</step>", "<step>H</step>")
        .replace("<duration>1</duration>", "<duration>2</duration>");
    fs::write(&input, fixture).expect("write score with independent invalid values");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("unsupported pitch step \"H\""),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("duration"), "{diagnostic}");
    assert!(!directory.join("multiple-values.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn accepts_utf16_bom_and_normalizes_xml_line_endings() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("UTF-8", "UTF-16")
        .replace('\n', "\r\n");

    for (suffix, little_endian) in [("le", true), ("be", false)] {
        let input = directory.join(format!("encoded-{suffix}.musicxml"));
        fs::write(&input, utf16_bytes(&fixture, little_endian)).expect("write UTF-16 score");
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "UTF-16 {suffix} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(directory.join(format!("encoded-{suffix}.svg")).is_file());
    }

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn malformed_encoding_diagnostics_do_not_invent_unicode_positions() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let utf8 = b"<score-partwise>\n\xf0\x9f\x8e\xb5\xc3(\n".to_vec();
    let mut utf16 = utf16_bytes("<score-partwise>\n", true);
    for unit in "\u{1f3b5}".encode_utf16() {
        utf16.extend(unit.to_le_bytes());
    }
    utf16.extend(0xd800_u16.to_le_bytes());

    for (name, input_bytes) in [("invalid-utf8", utf8), ("invalid-utf16", utf16)] {
        let input = directory.join(format!("{name}.musicxml"));
        fs::write(&input, input_bytes).expect("write malformed encoded document");
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(!output.status.success());
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostic.contains("byte "), "{diagnostic}");
        assert!(
            diagnostic.contains("line/column unavailable"),
            "{diagnostic}"
        );
        assert!(!diagnostic.contains(", line "), "{diagnostic}");
        assert!(!directory.join(format!("{name}.svg")).exists());
    }

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn enforces_namespace_well_formedness_and_no_namespace_musicxml_profile() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");

    let namespaced_input = directory.join("namespaced.musicxml");
    let namespaced = include_str!("fixtures/minimal.musicxml").replace(
        "<score-partwise version=\"4.0\">",
        "<score-partwise xmlns=\"urn:example:music\" version=\"4.0\">",
    );
    fs::write(&namespaced_input, namespaced).expect("write namespaced score");
    let namespaced_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&namespaced_input)
        .output()
        .expect("run CLI");
    assert!(!namespaced_output.status.success());
    assert!(String::from_utf8_lossy(&namespaced_output.stderr).contains("unsupported root"));
    assert!(!directory.join("namespaced.svg").exists());

    let undeclared_input = directory.join("undeclared-prefix.musicxml");
    fs::write(&undeclared_input, "<score-partwise:score version=\"4.0\"/>")
        .expect("write undeclared-prefix document");
    let undeclared = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&undeclared_input)
        .output()
        .expect("run CLI");
    assert!(!undeclared.status.success());
    let undeclared_diagnostic = String::from_utf8_lossy(&undeclared.stderr);
    assert!(undeclared_diagnostic.contains("malformed XML"));
    assert!(undeclared_diagnostic.contains("undeclared namespace prefix"));
    assert!(!directory.join("undeclared-prefix.svg").exists());

    let invalid_qname_input = directory.join("invalid-qname.musicxml");
    let invalid_qname = include_str!("fixtures/minimal.musicxml").replace(
        "<score-partwise version=\"4.0\">",
        "<score-partwise xmlns:a=\"urn:example\" a:1invalid=\"x\" version=\"4.0\">",
    );
    fs::write(&invalid_qname_input, invalid_qname).expect("write invalid QName score");
    let invalid_qname_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&invalid_qname_input)
        .output()
        .expect("run CLI");
    assert!(!invalid_qname_output.status.success());
    let diagnostic = String::from_utf8_lossy(&invalid_qname_output.stderr);
    assert!(diagnostic.contains("malformed XML"), "{diagnostic}");
    assert!(diagnostic.contains("QName"), "{diagnostic}");
    assert!(!directory.join("invalid-qname.svg").exists());

    let invalid_prefix_input = directory.join("invalid-prefix-qname.musicxml");
    let invalid_prefix = include_str!("fixtures/minimal.musicxml")
        .replace("<score-partwise", "<:score-partwise")
        .replace("</score-partwise>", "</:score-partwise>");
    fs::write(&invalid_prefix_input, invalid_prefix).expect("write invalid-prefix QName score");
    let invalid_prefix_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&invalid_prefix_input)
        .output()
        .expect("run CLI");
    assert!(!invalid_prefix_output.status.success());
    let diagnostic = String::from_utf8_lossy(&invalid_prefix_output.stderr);
    assert!(diagnostic.contains("malformed XML"), "{diagnostic}");
    assert!(diagnostic.contains("QName"), "{diagnostic}");
    assert!(!directory.join("invalid-prefix-qname.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_external_entity_references_without_writing_svg() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("external-entity.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", "")
        .replace(
            "<score-partwise",
            "<!DOCTYPE score-partwise [<!ENTITY ext SYSTEM 'file:///etc/passwd'>]>\n<score-partwise",
        )
        .replace("<part-name>Music</part-name>", "<part-name>&ext;</part-name>");
    fs::write(&input, fixture).expect("write score with external entity");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("external entity reference is not resolved"));
    assert!(diagnostic.contains(input.to_str().expect("UTF-8 path")));
    assert!(!directory.join("external-entity.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}
