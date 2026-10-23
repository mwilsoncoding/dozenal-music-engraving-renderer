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

fn path_translation(line: &str) -> (i32, i32) {
    let coordinates = line
        .split("transform=\"translate(")
        .nth(1)
        .and_then(|value| value.split_once(')'))
        .map(|(value, _)| value)
        .expect("path has a translation");
    let mut coordinates = coordinates.split_whitespace().map(|value| {
        value
            .parse::<i32>()
            .expect("translation coordinate is an integer")
    });
    (
        coordinates.next().expect("translation has an x coordinate"),
        coordinates.next().expect("translation has a y coordinate"),
    )
}

fn path_translation_f64(line: &str) -> (f64, f64) {
    let coordinates = line
        .split("transform=\"translate(")
        .nth(1)
        .and_then(|value| value.split_once(')'))
        .map(|(value, _)| value)
        .expect("path has a translation");
    let mut coordinates = coordinates.split_whitespace().map(|value| {
        value
            .parse::<f64>()
            .expect("translation coordinate is numeric")
    });
    (
        coordinates.next().expect("translation has an x coordinate"),
        coordinates.next().expect("translation has a y coordinate"),
    )
}

#[test]
fn renders_supported_articulation_marks() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("articulation.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(
            "</type>",
            "</type><notations><articulations><staccato/></articulations></notations>",
        )
        .replace(
            "</measure>",
            "<note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><notations><articulations><staccato placement=\"below\"/></articulations></notations></note></measure>",
        );
    fs::write(&input, fixture).expect("write articulation score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "articulation render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("articulation.svg")).expect("read SVG");
    assert_eq!(
        svg.matches("class=\"articulation-mark\"").count(),
        1,
        "{svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn centers_unpitched_mark_on_middle_staff_line_by_default() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("unpitched-center.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<pitch><step>C</step><octave>4</octave></pitch>",
        "<unpitched/>",
    );
    fs::write(&input, fixture).expect("write unpitched score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "unpitched render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("unpitched-center.svg")).expect("read SVG");
    let mark = svg
        .lines()
        .find(|line| line.contains("class=\"unpitched-mark\""))
        .expect("unpitched X mark");
    let (_, mark_y) = path_translation_f64(mark);
    assert_eq!(mark_y + 9.0, 68.0, "{mark}");
    assert!(mark.contains("M2 2L10 16M10 2L2 16"), "{mark}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_unpitched_events_example() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("unpitched-events.musicxml");
    fs::write(
        &input,
        include_str!("../examples/unpitched-events/unpitched-events.musicxml"),
    )
    .expect("write example score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "example render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("unpitched-events.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"unpitched-mark\"").count(), 4, "{svg}");
    let marks = svg
        .lines()
        .filter(|line| line.contains("class=\"unpitched-mark\""))
        .collect::<Vec<_>>();
    let positions = marks
        .iter()
        .map(|mark| path_translation_f64(mark))
        .collect::<Vec<_>>();
    assert_eq!(
        positions.iter().map(|(_, y)| *y).collect::<Vec<_>>(),
        [59.0, 49.0, 49.0, 29.0]
    );
    assert_ne!(
        positions[1].0, positions[2].0,
        "chord X marks must not overlap"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("display-step is ignored"));

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_unpitched_chord_members_and_rejects_mixed_chords() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("fixtures/minimal.musicxml");
    let first_note = "<note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let chord_note = "<note><chord/><unpitched/><duration>1</duration><voice>1</voice><type>quarter</type></note>";

    let unpitched_chord_input = directory.join("unpitched-chord.musicxml");
    let unpitched_chord = fixture
        .replace(
            first_note,
            "<note><unpitched/><duration>1</duration><voice>1</voice><type>quarter</type></note>",
        )
        .replace("</measure>", &format!("{chord_note}</measure>"));
    fs::write(&unpitched_chord_input, unpitched_chord).expect("write unpitched chord");
    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&unpitched_chord_input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "unpitched chord failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("unpitched-chord.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"unpitched-mark\"").count(), 2, "{svg}");

    let mixed_chord_input = directory.join("mixed-chord.musicxml");
    let mixed_chord = fixture.replace("</measure>", &format!("{chord_note}</measure>"));
    fs::write(&mixed_chord_input, mixed_chord).expect("write mixed chord");
    let mixed_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&mixed_chord_input)
        .output()
        .expect("run CLI");
    assert!(
        !mixed_output.status.success()
            && String::from_utf8_lossy(&mixed_output.stderr)
                .contains("chord may not mix pitched and unpitched events"),
        "mixed chord should be rejected: {}",
        String::from_utf8_lossy(&mixed_output.stderr)
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn accepts_x_as_the_unpitched_notehead_without_warning() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<pitch><step>C</step><octave>4</octave></pitch>",
        "<unpitched/>",
    );
    let plain_input = directory.join("unpitched-x.musicxml");
    fs::write(
        &plain_input,
        fixture.replace(
            "<type>quarter</type>",
            "<type>quarter</type><notehead>x</notehead>",
        ),
    )
    .expect("write unpitched X score");
    let plain_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&plain_input)
        .output()
        .expect("run CLI");
    assert!(
        plain_output.status.success(),
        "{}",
        String::from_utf8_lossy(&plain_output.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&plain_output.stderr).contains("notehead"),
        "plain X notehead should not warn: {}",
        String::from_utf8_lossy(&plain_output.stderr)
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn applies_unpitched_notehead_color_and_font_size() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<pitch><step>C</step><octave>4</octave></pitch>",
        "<unpitched/>",
    );
    let medium_input = directory.join("unpitched-medium.musicxml");
    fs::write(
        &medium_input,
        fixture.replace(
            "<type>quarter</type>",
            "<type>quarter</type><notehead color=\"#ff0000\">x</notehead>",
        ),
    )
    .expect("write medium X score");
    let medium_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&medium_input)
        .output()
        .expect("run CLI");
    assert!(
        medium_output.status.success(),
        "medium X notehead failed: {}",
        String::from_utf8_lossy(&medium_output.stderr)
    );
    let medium_svg =
        fs::read_to_string(directory.join("unpitched-medium.svg")).expect("read medium SVG");

    let styled_input = directory.join("unpitched-styled.musicxml");
    fs::write(
        &styled_input,
        fixture.replace(
            "<type>quarter</type>",
            "<type>quarter</type><notehead color=\"#ff0000\" font-size=\"large\">x</notehead>",
        ),
    )
    .expect("write styled unpitched score");
    let styled_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&styled_input)
        .output()
        .expect("run CLI");
    assert!(
        styled_output.status.success(),
        "styled X notehead failed: {}",
        String::from_utf8_lossy(&styled_output.stderr)
    );
    let styled_svg =
        fs::read_to_string(directory.join("unpitched-styled.svg")).expect("read styled SVG");
    let styled_mark = styled_svg
        .lines()
        .find(|line| line.contains("class=\"unpitched-mark\""))
        .expect("styled X mark");
    assert!(styled_mark.contains("stroke=\"#ff0000\""), "{styled_mark}");
    assert_ne!(
        styled_svg, medium_svg,
        "large X should render larger than medium X"
    );
    assert!(
        !String::from_utf8_lossy(&styled_output.stderr).contains("notehead"),
        "supported X styling should not warn: {}",
        String::from_utf8_lossy(&styled_output.stderr)
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn warns_and_falls_back_for_alternate_unpitched_notehead() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<pitch><step>C</step><octave>4</octave></pitch>",
        "<unpitched/>",
    );
    let alternate_input = directory.join("unpitched-alternate.musicxml");
    fs::write(
        &alternate_input,
        fixture.replace(
            "<type>quarter</type>",
            "<type>quarter</type><notehead>diamond</notehead>",
        ),
    )
    .expect("write alternate notehead score");
    let alternate_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&alternate_input)
        .output()
        .expect("run CLI");
    assert!(
        alternate_output.status.success(),
        "{}",
        String::from_utf8_lossy(&alternate_output.stderr)
    );
    assert!(String::from_utf8_lossy(&alternate_output.stderr).contains("unpitched notehead"));
    let alternate_svg =
        fs::read_to_string(directory.join("unpitched-alternate.svg")).expect("read alternate SVG");
    assert!(alternate_svg.contains("M2 2L10 16M10 2L2 16"));

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_malformed_unpitched_notehead_styles() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<pitch><step>C</step><octave>4</octave></pitch>",
        "<unpitched/>",
    );
    let malformed_input = directory.join("unpitched-malformed.musicxml");
    fs::write(
        &malformed_input,
        fixture.replace(
            "<type>quarter</type>",
            "<type>quarter</type><notehead filled=\"maybe\">x</notehead>",
        ),
    )
    .expect("write malformed notehead score");
    let malformed_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&malformed_input)
        .output()
        .expect("run CLI");
    assert!(!malformed_output.status.success());
    assert!(
        String::from_utf8_lossy(&malformed_output.stderr).contains("filled must be 'yes' or 'no'")
    );

    let invalid_font_size_input = directory.join("unpitched-invalid-font-size.musicxml");
    fs::write(
        &invalid_font_size_input,
        fixture.replace(
            "<type>quarter</type>",
            "<type>quarter</type><notehead font-size=\"0\">x</notehead>",
        ),
    )
    .expect("write invalid font-size score");
    let invalid_font_size_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&invalid_font_size_input)
        .output()
        .expect("run CLI");
    assert!(
        !invalid_font_size_output.status.success()
            && String::from_utf8_lossy(&invalid_font_size_output.stderr)
                .contains("invalid notehead font-size"),
        "non-positive notehead font-size should fail: {}",
        String::from_utf8_lossy(&invalid_font_size_output.stderr)
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn accepts_valid_unpitched_instrument_reference_silently() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<pitch><step>C</step><octave>4</octave></pitch>",
        "<unpitched/>",
    );
    let instrument_score = fixture
        .replace(
            "<part-name>Music</part-name>",
            "<part-name>Music</part-name><part-name-display><display-text>Music</display-text></part-name-display><part-abbreviation>Drums</part-abbreviation><part-abbreviation-display><display-text>Drums</display-text></part-abbreviation-display><score-instrument id=\"P1-I1\"><instrument-name>Drum</instrument-name><instrument-abbreviation>Dr</instrument-abbreviation><instrument-sound>drum.percussion</instrument-sound></score-instrument><player><player-name>Player</player-name><player-id>1</player-id></player><midi-instrument id=\"P1-I1\"><midi-channel>10</midi-channel><midi-program>1</midi-program></midi-instrument>",
        )
        .replace(
            "<duration>1</duration>",
            "<duration>1</duration><instrument id=\"P1-I1\"/>",
        );
    let instrument_input = directory.join("unpitched-instrument.musicxml");
    fs::write(&instrument_input, instrument_score).expect("write instrument score");
    let instrument_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&instrument_input)
        .output()
        .expect("run CLI");
    assert!(
        instrument_output.status.success(),
        "valid instrument reference failed: {}",
        String::from_utf8_lossy(&instrument_output.stderr)
    );
    assert!(
        instrument_output.stderr.is_empty(),
        "valid instrument reference should be silent: {}",
        String::from_utf8_lossy(&instrument_output.stderr)
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_ties_on_unpitched_events_contextually() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<pitch><step>C</step><octave>4</octave></pitch>",
        "<unpitched/>",
    );
    let tied_score = fixture.replace(
        "<duration>1</duration>",
        "<duration>1</duration><tie type=\"start\"/>",
    );
    let input = directory.join("unpitched-tie.musicxml");
    fs::write(&input, tied_score).expect("write tied unpitched score");
    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success()
            && String::from_utf8_lossy(&output.stderr)
                .contains("ties are unsupported on unpitched events"),
        "unpitched tie should be rejected contextually: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_unpitched_instrument_reference_without_declaration() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<pitch><step>C</step><octave>4</octave></pitch>",
        "<unpitched/>",
    );
    let score = fixture.replace(
        "<duration>1</duration>",
        "<duration>1</duration><instrument id=\"P1-I1\"/>",
    );
    let input = directory.join("unpitched-undeclared-instrument.musicxml");
    fs::write(&input, score).expect("write undeclared instrument score");
    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success()
            && String::from_utf8_lossy(&output.stderr).contains("has no matching score-instrument"),
        "undeclared instrument ID should fail: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_ornament_and_note_fermata_marks() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("ornament-fermata.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "</type>",
        "</type><notations><ornaments><trill-mark/><turn/><mordent/></ornaments><fermata type=\"upright\">normal</fermata></notations>",
    );
    fs::write(&input, fixture).expect("write ornament score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "ornament render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("ornament-fermata.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"ornament-mark\"").count(), 3, "{svg}");
    assert_eq!(svg.matches("class=\"fermata-mark\"").count(), 1, "{svg}");
    let tonehead = svg
        .lines()
        .find(|line| line.contains("id=\"tonehead\""))
        .expect("ornament anchor Tonehead");
    let (tonehead_x, _) = path_translation_f64(tonehead);
    for ornament in svg
        .lines()
        .filter(|line| line.contains("class=\"ornament-mark\""))
    {
        let (ornament_x, _) = path_translation_f64(ornament);
        assert_eq!(
            ornament_x + 10.0,
            tonehead_x + 6.0,
            "{ornament}\n{tonehead}"
        );
    }

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn centers_trill_above_staff_on_its_tonehead_at_barline_fermata_height() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("trill-placement.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(
            "</type>",
            "</type><notations><ornaments><trill-mark/></ornaments></notations>",
        )
        .replace(
            "</measure>",
            "<barline><fermata type=\"upright\">normal</fermata></barline></measure>",
        );
    fs::write(&input, fixture).expect("write trill placement score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "trill render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("trill-placement.svg")).expect("read SVG");
    let trill = svg
        .lines()
        .find(|line| line.contains("id=\"ornament-0-0\""))
        .expect("trill path");
    let tonehead = svg
        .lines()
        .find(|line| line.contains("id=\"tonehead\""))
        .expect("attached Tonehead path");
    let barline_fermata = svg
        .lines()
        .find(|line| line.contains("id=\"fermata-barline-0-0\""))
        .expect("barline fermata path");
    let (trill_x, trill_y) = path_translation_f64(trill);
    let (tonehead_x, _) = path_translation_f64(tonehead);
    let (_, fermata_y) = path_translation_f64(barline_fermata);
    assert_eq!(trill_x + 10.0, tonehead_x + 6.0, "{trill}\n{tonehead}");
    assert_eq!(trill_y, fermata_y, "{trill}\n{barline_fermata}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn raises_trill_when_a_high_tonehead_intrudes_on_its_default_position() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("high-trill.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<octave>4</octave>", "<octave>7</octave>")
        .replace(
            "</type>",
            "</type><notations><ornaments><trill-mark/></ornaments></notations>",
        );
    fs::write(&input, fixture).expect("write high trill score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "high trill render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("high-trill.svg")).expect("read SVG");
    let trill = svg
        .lines()
        .find(|line| line.contains("id=\"ornament-0-0\""))
        .expect("trill path");
    let tonehead = svg
        .lines()
        .find(|line| line.contains("id=\"tonehead\""))
        .expect("attached Tonehead path");
    let (trill_x, trill_y) = path_translation_f64(trill);
    let (tonehead_x, tonehead_y) = path_translation_f64(tonehead);
    assert_eq!(trill_x + 10.0, tonehead_x + 6.0, "{trill}\n{tonehead}");
    assert!(trill_y + 10.0 < tonehead_y, "{trill}\n{tonehead}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_fermata_marks_on_rests_and_barlines() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("rest-barline-fermata.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(
            "</type>",
            "</type><notations><fermata type=\"upright\">double-angled</fermata></notations>",
        )
        .replace(
            "</part>",
            "<measure number=\"2\"><note><rest/><duration>1</duration><voice>1</voice><type>quarter</type><notations><fermata type=\"inverted\">double-square</fermata></notations></note><barline><fermata type=\"upright\">angled</fermata><fermata type=\"upright\">square</fermata></barline></measure></part>",
        );
    fs::write(&input, fixture).expect("write Rest and barline score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "Rest/barline fermata render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("rest-barline-fermata.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"fermata-mark\"").count(), 4, "{svg}");
    let angled_fermata = svg
        .lines()
        .find(|line| {
            line.contains("id=\"fermata-barline-") && line.contains("class=\"fermata-mark\"")
        })
        .expect("angled barline fermata");
    let barline = svg
        .lines()
        .find(|line| line.contains("id=\"barline-2\""))
        .expect("fermata anchor barline");
    let (fermata_x, _) = path_translation_f64(angled_fermata);
    let barline_x = barline
        .split("d=\"M")
        .nth(1)
        .and_then(|path| path.split_whitespace().next())
        .and_then(|value| value.parse::<f64>().ok())
        .expect("barline path has an x coordinate");
    assert_eq!(fermata_x + 9.0, barline_x, "{angled_fermata}\n{barline}");
    assert!(
        angled_fermata.contains("M1.73 10C1.79 10"),
        "{angled_fermata}"
    );
    let square_fermata = svg
        .lines()
        .find(|line| line.contains("class=\"fermata-mark\"") && line.contains("M1.5 0C0.66 0"))
        .expect("square fermata with Bravura U-shaped contour");
    assert!(square_fermata.contains("ZM10.66 8.35"), "{square_fermata}");
    let double_angled_fermata = svg
        .lines()
        .find(|line| line.contains("class=\"fermata-mark\"") && line.contains("M1.63 10C1.69 10"))
        .expect("double-angled fermata with nested chevrons");
    assert!(
        double_angled_fermata.contains("ZM4.59 10"),
        "{double_angled_fermata}"
    );
    let double_square_fermata = svg
        .lines()
        .find(|line| line.contains("class=\"fermata-mark\"") && line.contains("M1.54 0C0.83 0"))
        .expect("double-square fermata with nested frames");
    assert!(
        double_square_fermata.contains("ZM3.89 3.38"),
        "{double_square_fermata}"
    );
    assert!(
        double_square_fermata.contains("ZM10.56 8.58"),
        "{double_square_fermata}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_articulation_ornament_fermata_example() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("articulation-ornament-fermata.musicxml");
    fs::write(
        &input,
        include_str!(
            "../examples/articulation-ornament-fermata/articulation-ornament-fermata.musicxml"
        ),
    )
    .expect("write example score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "example render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("articulation-ornament-fermata.svg"))
        .expect("read example SVG");
    for id in ["articulation-4-0"] {
        let path = svg
            .lines()
            .find(|line| line.contains(&format!("id=\"{id}\"")))
            .unwrap_or_else(|| panic!("missing {id} path in {svg}"));
        assert!(path.contains("fill=\"none\""), "{path}");
    }
    for id in ["ornament-6-0", "fermata-11-0"] {
        let path = svg
            .lines()
            .find(|line| line.contains(&format!("id=\"{id}\"")))
            .unwrap_or_else(|| panic!("missing {id} path in {svg}"));
        assert!(path.contains("fill-rule=\"nonzero\""), "{path}");
    }
    let staccato_line = svg
        .lines()
        .find(|line| line.contains("id=\"articulation-0-0\""))
        .expect("first note staccato path");
    let tonehead_line = svg
        .lines()
        .find(|line| line.contains("id=\"tonehead\""))
        .expect("first note Tonehead path");
    let (staccato_x, _) = path_translation(staccato_line);
    let (tonehead_x, _) = path_translation(tonehead_line);
    assert_eq!(
        staccato_x + 3,
        tonehead_x + 6,
        "{staccato_line}\n{tonehead_line}"
    );
    let fermata_line = svg
        .lines()
        .find(|line| line.contains("id=\"fermata-11-0\""))
        .expect("Bravura fermata path");
    let trill_line = svg
        .lines()
        .find(|line| line.contains("class=\"ornament-mark\"") && line.contains("M10.14 0C10.04 0"))
        .expect("Bravura trill path");
    let mordent_line = svg
        .lines()
        .find(|line| {
            line.contains("class=\"ornament-mark\"") && line.contains("M8.22 5.18 5.77 2.14")
        })
        .expect("Bravura mordent path");
    let turn_line = svg
        .lines()
        .find(|line| line.contains("M16.13 0C14.78 0") && line.contains("class=\"ornament-mark\""))
        .expect("Bravura ordinary turn path");
    let inverted_turn_line = svg
        .lines()
        .find(|line| {
            line.contains("id=\"ornament-")
                && line.contains("-bar\"")
                && line.contains("M10 0V9.48")
        })
        .expect("inverted turn path with centered bar");
    assert!(turn_line.contains("fill=\"#cc0000\""), "{turn_line}");
    assert!(
        inverted_turn_line.contains("stroke-width=\"1.5\""),
        "{inverted_turn_line}"
    );
    assert!(staccato_line.contains("A1 1"), "{staccato_line}");
    assert!(fermata_line.contains("fill=\"#00aa00\""), "{fermata_line}");
    assert!(trill_line.contains("fill-rule=\"nonzero\""), "{trill_line}");
    assert!(
        mordent_line.contains("fill-rule=\"nonzero\""),
        "{mordent_line}"
    );
    assert_eq!(
        svg.matches("class=\"articulation-mark\"").count(),
        6,
        "{svg}"
    );
    assert_eq!(svg.matches("class=\"ornament-mark\"").count(), 5, "{svg}");
    assert_eq!(svg.matches("class=\"fermata-mark\"").count(), 7, "{svg}");
    assert_eq!(svg.matches("class=\"fermata-dot\"").count(), 0, "{svg}");
    assert!(svg.contains("fill=\"#cc0000\""), "{svg}");
    assert!(svg.contains("fill=\"#00aa00\""), "{svg}");
    let mut marked_events = Vec::new();
    for line in svg.lines().filter(|line| {
        (line.contains("class=\"articulation-mark\"")
            || line.contains("class=\"ornament-mark\"")
            || line.contains("class=\"fermata-mark\""))
            && !line.contains("id=\"fermata-barline-")
    }) {
        let id = line
            .split("id=\"")
            .nth(1)
            .and_then(|value| value.split_once('"'))
            .map(|(id, _)| id)
            .expect("mark path has an id");
        let event_index = id
            .split('-')
            .nth(1)
            .expect("note mark id has an event index")
            .to_owned();
        assert!(
            !marked_events.contains(&event_index),
            "multiple mark categories on note event {event_index}: {line}"
        );
        marked_events.push(event_index);
    }
    assert_eq!(marked_events.len(), 17, "{svg}");
    assert!(svg.contains("scale(2)"), "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn warns_for_unsupported_marks_and_rejects_malformed_articulations() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let source = include_str!("fixtures/minimal.musicxml");
    let unsupported_input = directory.join("unsupported-marks.musicxml");
    let unsupported_fixture = source.replace(
        "</type>",
        "</type><notations><articulations><staccato/><detached-legato/><other-articulation>custom accent</other-articulation></articulations><ornaments><wavy-line type=\"start\"/><other-ornament>custom turn</other-ornament></ornaments></notations>",
    );
    fs::write(&unsupported_input, unsupported_fixture).expect("write unsupported score");
    let unsupported = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&unsupported_input)
        .output()
        .expect("run CLI");
    assert!(
        unsupported.status.success(),
        "valid unsupported marks should not prevent rendering: {}",
        String::from_utf8_lossy(&unsupported.stderr)
    );
    let warnings = String::from_utf8_lossy(&unsupported.stderr);
    assert!(
        warnings.contains("unsupported articulation <detached-legato>"),
        "{warnings}"
    );
    assert!(
        warnings.contains("unsupported ornament <wavy-line>"),
        "{warnings}"
    );
    assert!(
        warnings.contains("unsupported articulation <other-articulation>"),
        "{warnings}"
    );
    assert!(
        warnings.contains("unsupported ornament <other-ornament>"),
        "{warnings}"
    );
    let svg =
        fs::read_to_string(directory.join("unsupported-marks.svg")).expect("read warning output");
    assert_eq!(
        svg.matches("class=\"articulation-mark\"").count(),
        1,
        "{svg}"
    );
    assert!(!svg.contains("class=\"ornament-mark\""), "{svg}");

    let malformed_input = directory.join("malformed-articulation.musicxml");
    let malformed_fixture = source.replace(
        "</type>",
        "</type><notations><articulations><strong-accent type=\"side\"/></articulations></notations>",
    );
    fs::write(&malformed_input, malformed_fixture).expect("write malformed score");
    let malformed = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&malformed_input)
        .output()
        .expect("run CLI");
    let error = String::from_utf8_lossy(&malformed.stderr);
    assert!(!malformed.status.success(), "{error}");
    assert!(
        error.contains("strong-accent type must be 'up' or 'down'"),
        "{error}"
    );
    assert!(error.contains("XML path /score-partwise/part"), "{error}");
    assert!(
        !directory.join("malformed-articulation.svg").exists(),
        "fatal errors must not write partial output"
    );

    for (name, notation, expected_error) in [
        (
            "malformed-ornament",
            "<ornaments><wavy-line type=\"middle\"/></ornaments>",
            "wavy-line type must be 'start' or 'stop'",
        ),
        (
            "malformed-fermata",
            "<fermata type=\"side\">normal</fermata>",
            "invalid fermata orientation",
        ),
    ] {
        let input = directory.join(format!("{name}.musicxml"));
        let fixture = source.replace(
            "</type>",
            &format!("</type><notations>{notation}</notations>"),
        );
        fs::write(&input, fixture).expect("write malformed mark score");
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{error}");
        assert!(error.contains(expected_error), "{error}");
        assert!(
            !directory.join(format!("{name}.svg")).exists(),
            "fatal errors must not write partial output"
        );
    }

    let fallback_input = directory.join("fallback-fermata.musicxml");
    let fallback_fixture = source.replace(
        "</type>",
        "</type><notations><fermata>short</fermata></notations>",
    );
    fs::write(&fallback_input, fallback_fixture).expect("write fallback Fermata score");
    let fallback = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&fallback_input)
        .output()
        .expect("run CLI");
    assert!(
        fallback.status.success(),
        "{}",
        String::from_utf8_lossy(&fallback.stderr)
    );
    assert!(
        String::from_utf8_lossy(&fallback.stderr).contains("normal shape is used"),
        "{}",
        String::from_utf8_lossy(&fallback.stderr)
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn scales_marks_with_grace_and_cue_notes() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let source = include_str!("fixtures/minimal.musicxml");
    for (name, fixture) in [
        (
            "grace-marks",
            source
                .replace("<note>\n", "<note>\n        <grace slash=\"yes\"/>\n")
                .replace("<duration>1</duration>", "")
                .replace(
                    "</type>",
                    "</type><notations><articulations><staccato/></articulations><ornaments><turn/></ornaments><fermata>normal</fermata></notations>",
                ),
        ),
        (
            "cue-marks",
            source
                .replace("<note>\n", "<note>\n        <cue/>\n")
                .replace(
                    "</type>",
                    "</type><notations><articulations><staccato/></articulations><ornaments><turn/></ornaments><fermata>normal</fermata></notations>",
                ),
        ),
    ] {
        let input = directory.join(format!("{name}.musicxml"));
        fs::write(&input, fixture).expect("write scaled-mark score");
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "{name} render failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let svg = fs::read_to_string(directory.join(format!("{name}.svg")))
            .expect("read scaled-mark SVG");
        for class in ["articulation-mark", "ornament-mark", "fermata-mark"] {
            let mark = svg
                .lines()
                .find(|line| line.contains(&format!("class=\"{class}\"")))
                .unwrap_or_else(|| panic!("missing {class} in {svg}"));
            assert!(mark.contains("scale(0.6)"), "{mark}");
        }
    }

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_each_input_staff_to_its_own_svg() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("input-staff-routing.musicxml");
    fs::write(
        &input,
        include_str!("../examples/input-staff-routing/input-staff-routing.musicxml"),
    )
    .expect("write two-staff score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .arg("--input-staff-mode")
        .arg("separate")
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "separate Input Staff renders failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut meter_paths = Vec::new();
    for staff_number in 1..=2 {
        let svg = fs::read_to_string(
            directory.join(format!("input-staff-routing-staff-{staff_number}.svg")),
        )
        .expect("read separate Input Staff SVG");
        assert_eq!(svg.matches("class=\"tonehead\"").count(), 4, "{svg}");
        if staff_number == 1 {
            let tonehead_xs = svg
                .lines()
                .filter(|line| line.contains("class=\"tonehead\""))
                .map(|line| {
                    line.split("transform=\"translate(")
                        .nth(1)
                        .and_then(|transform| transform.split_whitespace().next())
                        .and_then(|x| x.parse::<f64>().ok())
                        .expect("numeric tonehead position")
                })
                .collect::<Vec<_>>();
            assert_eq!(
                tonehead_xs[1] - tonehead_xs[0],
                80.0,
                "filtered events must preserve source timing: {tonehead_xs:?}"
            );
        }
        let expected_text_glyphs = if staff_number == 1 { 6 } else { 11 };
        assert_eq!(
            svg.matches("class=\"text-glyph\"").count(),
            expected_text_glyphs,
            "shared and staff-targeted directions must route correctly: {svg}"
        );
        assert_eq!(
            svg.contains("id=\"chord-symbol-0-root\""),
            staff_number == 2,
            "staff-targeted Chord Symbol must route correctly: {svg}"
        );
        let meter_path = svg
            .lines()
            .find(|line| line.contains("id=\"meter-numerator\""))
            .expect("Input Staff meter path");
        meter_paths.push(meter_path.to_owned());
    }
    assert_ne!(
        meter_paths[0], meter_paths[1],
        "numbered meters must be scoped"
    );
    assert!(
        !directory.join("input-staff-routing.svg").exists(),
        "separate mode must not write the combined output"
    );

    let combined = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run combined CLI");
    assert!(
        combined.status.success(),
        "combined render failed: {}",
        String::from_utf8_lossy(&combined.stderr)
    );
    let combined_svg =
        fs::read_to_string(directory.join("input-staff-routing.svg")).expect("read combined SVG");
    assert_eq!(combined_svg.matches("class=\"tonehead\"").count(), 8);

    let explicit_combined_output = directory.join("explicit-combined.svg");
    let explicit_combined = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .arg("--input-staff-mode")
        .arg("combine")
        .arg("--output")
        .arg(&explicit_combined_output)
        .output()
        .expect("run explicit combined CLI");
    assert!(
        explicit_combined.status.success(),
        "explicit combined render failed: {}",
        String::from_utf8_lossy(&explicit_combined.stderr)
    );
    assert!(explicit_combined_output.exists());

    let explicit_output = directory.join("named-score.svg");
    let explicit = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .arg("--output")
        .arg(&explicit_output)
        .arg("--input-staff-mode")
        .arg("separate")
        .output()
        .expect("run explicit-output CLI");
    assert!(
        explicit.status.success(),
        "explicit separate output failed: {}",
        String::from_utf8_lossy(&explicit.stderr)
    );
    for staff_number in 1..=2 {
        assert!(directory
            .join(format!("named-score-staff-{staff_number}.svg"))
            .exists());
    }

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn scopes_numbered_transposition_to_its_input_staff() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("scoped-transposition.musicxml");
    let source = include_str!("../examples/input-staff-routing/input-staff-routing.musicxml");
    let fixture = source.replace(
        "<staves>2</staves>",
        "<staves>2</staves><transpose number=\"2\"><chromatic>12</chromatic></transpose>",
    );
    fs::write(&input, fixture).expect("write scoped-transposition score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .arg("--input-staff-mode")
        .arg("separate")
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "scoped transposition failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let first_staff = fs::read_to_string(directory.join("scoped-transposition-staff-1.svg"))
        .expect("read first staff SVG");
    let second_staff = fs::read_to_string(directory.join("scoped-transposition-staff-2.svg"))
        .expect("read second staff SVG");
    assert!(first_staff.contains("data-octave=\"5\""), "{first_staff}");
    assert!(second_staff.contains("data-octave=\"4\""), "{second_staff}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn applies_unnumbered_staff_state_to_every_input_staff() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("unnumbered-input-staff-state.musicxml");
    let source = include_str!("../examples/input-staff-routing/input-staff-routing.musicxml");
    let fixture = source
        .replace("<time number=\"2\">", "<time>")
        .replace("<key number=\"1\">", "<key>")
        .replace("<key number=\"2\">", "<key>")
        .replace("<clef number=\"1\">", "<clef>")
        .replace("<clef number=\"2\">", "<clef>")
        .replace(
            "<staves>2</staves>",
            "<staves>2</staves><transpose><chromatic>12</chromatic></transpose>",
        );
    fs::write(&input, fixture).expect("write unnumbered-state score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .arg("--input-staff-mode")
        .arg("separate")
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "unnumbered staff state failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let first_staff =
        fs::read_to_string(directory.join("unnumbered-input-staff-state-staff-1.svg"))
            .expect("read first staff SVG");
    let second_staff =
        fs::read_to_string(directory.join("unnumbered-input-staff-state-staff-2.svg"))
            .expect("read second staff SVG");
    assert!(first_staff.contains("data-octave=\"6\""), "{first_staff}");
    assert!(second_staff.contains("data-octave=\"4\""), "{second_staff}");
    let meter_path = |svg: &str| {
        svg.lines()
            .find(|line| line.contains("id=\"meter-numerator\""))
            .map(str::to_owned)
            .expect("meter numerator path")
    };
    assert_eq!(meter_path(&first_staff), meter_path(&second_staff));

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn keeps_divisions_part_wide_across_input_staffs() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("part-wide-divisions.musicxml");
    let source = include_str!("../examples/input-staff-routing/input-staff-routing.musicxml");
    let fixture = source
        .replace("<divisions>1</divisions>", "<divisions>2</divisions>")
        .replace("<duration>1</duration>", "<duration>2</duration>");
    fs::write(&input, fixture).expect("write Part-wide divisions score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .arg("--input-staff-mode")
        .arg("separate")
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "Part-wide divisions failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for staff_number in 1..=2 {
        let svg = fs::read_to_string(
            directory.join(format!("part-wide-divisions-staff-{staff_number}.svg")),
        )
        .expect("read staff SVG");
        assert_eq!(svg.matches("class=\"tonehead\"").count(), 4, "{svg}");
    }

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn splits_cross_input_staff_chords_only_in_separate_mode() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("cross-staff-chord.musicxml");
    let source = include_str!("../examples/input-staff-routing/input-staff-routing.musicxml");
    let fixture = source.replace(
        "<pitch><step>C</step><octave>3</octave></pitch>",
        "<chord/><pitch><step>C</step><octave>3</octave></pitch>",
    );
    fs::write(&input, fixture).expect("write cross-staff chord score");

    let combined = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run combined CLI");
    assert!(!combined.status.success());
    assert!(
        String::from_utf8_lossy(&combined.stderr).contains("a chord may not mix Input Staves"),
        "{}",
        String::from_utf8_lossy(&combined.stderr)
    );
    assert!(!directory.join("cross-staff-chord.svg").exists());

    let separate = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .arg("--input-staff-mode")
        .arg("separate")
        .output()
        .expect("run separate CLI");
    assert!(
        separate.status.success(),
        "{}",
        String::from_utf8_lossy(&separate.stderr)
    );
    for staff_number in 1..=2 {
        let svg = fs::read_to_string(
            directory.join(format!("cross-staff-chord-staff-{staff_number}.svg")),
        )
        .expect("read split chord SVG");
        assert_eq!(svg.matches("class=\"tonehead\"").count(), 4, "{svg}");
    }

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_notes_on_undeclared_input_staves_with_context() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("undeclared-input-staff.musicxml");
    let source = include_str!("../examples/input-staff-routing/input-staff-routing.musicxml");
    let fixture = source.replace(
        "<type>quarter</type><staff>2</staff>",
        "<type>quarter</type><staff>3</staff>",
    );
    fs::write(&input, fixture).expect("write undeclared-staff score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("Input Staff number must be between 1 and 2, not \"3\""),
        "{stderr}"
    );
    assert!(stderr.contains("line"), "{stderr}");
    assert!(!directory.join("undeclared-input-staff.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_key_and_clef_state_for_undeclared_input_staves() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let source = include_str!("../examples/input-staff-routing/input-staff-routing.musicxml");

    for (label, marker, replacement) in [
        ("key", "<key number=\"2\">", "<key number=\"3\">"),
        ("clef", "<clef number=\"2\">", "<clef number=\"3\">"),
    ] {
        let input = directory.join(format!("undeclared-{label}-staff.musicxml"));
        let fixture = source.replace(marker, replacement);
        fs::write(&input, fixture).expect("write undeclared state score");
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{label} staff was accepted");
        assert!(
            stderr.contains("Input Staff number must be between 1 and 2"),
            "{stderr}"
        );
        assert!(
            stderr.contains(&format!("/{label}[@number='3']")),
            "{stderr}"
        );
        assert!(!directory
            .join(format!("undeclared-{label}-staff.svg"))
            .exists());
    }

    fs::remove_dir_all(directory).expect("remove test directory");
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

fn text_path_y(svg: &str, id: &str) -> f64 {
    svg.lines()
        .find(|line| line.contains(&format!("id=\"{id}\"")))
        .and_then(|line| line.split("transform=\"translate(").nth(1))
        .and_then(|value| value.split(')').next())
        .and_then(|value| value.split_whitespace().nth(1))
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing translated path {id} in SVG: {svg}"))
}

fn svg_view_box_top(svg: &str) -> f64 {
    svg.lines()
        .next()
        .and_then(|line| line.split("viewBox=\"0 ").nth(1))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing SVG viewBox top: {svg}"))
}

fn svg_view_box_bottom(svg: &str) -> f64 {
    let (top, height) = svg
        .lines()
        .next()
        .and_then(|line| line.split("viewBox=\"0 ").nth(1))
        .and_then(|value| {
            let dimensions = value
                .split('"')
                .next()?
                .split_whitespace()
                .collect::<Vec<_>>();
            Some((
                dimensions[0].parse::<f64>().ok()?,
                dimensions[2].parse::<f64>().ok()?,
            ))
        })
        .unwrap_or_else(|| panic!("missing SVG viewBox dimensions: {svg}"));
    top + height
}

fn svg_view_box_left(svg: &str) -> f64 {
    svg.lines()
        .next()
        .and_then(|line| line.split("viewBox=\"").nth(1))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing SVG viewBox left: {svg}"))
}

fn text_path_x(svg: &str, id: &str) -> f64 {
    svg.lines()
        .find(|line| line.contains(&format!("id=\"{id}\"")))
        .and_then(|line| line.split("transform=\"translate(").nth(1))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing translated path {id} in SVG: {svg}"))
}

fn path_move_y(svg: &str, id: &str) -> f64 {
    svg.lines()
        .find(|line| line.contains(&format!("id=\"{id}\"")))
        .and_then(|line| line.split(" d=\"M").nth(1))
        .and_then(|value| value.split('H').next())
        .and_then(|value| value.split_whitespace().nth(1))
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing path move for {id} in SVG: {svg}"))
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
fn renders_words_and_rehearsal_as_ordered_svg_paths() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("words-rehearsal.musicxml");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml");
    fs::write(&input, fixture).expect("write text direction score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "text directions failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("words-rehearsal.svg")).expect("read SVG");
    assert!(
        !svg.contains("<text"),
        "text output must use glyph paths: {svg}"
    );
    assert_eq!(svg.matches("class=\"text-glyph\"").count(), 28, "{svg}");
    assert_eq!(svg.matches("class=\"text-enclosure\"").count(), 2, "{svg}");
    assert!(
        svg.find("id=\"text-0-enclosure\"") < svg.find("id=\"text-1-glyph-0\""),
        "rehearsal and words paths should retain source order: {svg}"
    );
    assert_ne!(
        text_path_y(&svg, "text-0-glyph-0"),
        text_path_y(&svg, "text-1-glyph-0"),
        "overlapping text on the same side should stack"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_multiple_parts_as_aligned_named_staves() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("aligned-parts.musicxml");
    let source = include_str!("../examples/aligned-parts/aligned-parts.musicxml");
    let first_part_start = source.find("  <part id=\"P1\">").expect("first Part");
    let second_part_start = source.find("  <part id=\"P2\">").expect("second Part");
    let first_part_end = first_part_start
        + source[first_part_start..]
            .find("  </part>")
            .expect("first Part end")
        + "  </part>".len();
    let second_part_end = second_part_start
        + source[second_part_start..]
            .find("  </part>")
            .expect("second Part end")
        + "  </part>".len();
    let reversed_parts = format!(
        "{}{}\n{}{}",
        &source[..first_part_start],
        &source[second_part_start..second_part_end],
        &source[first_part_start..first_part_end],
        &source[second_part_end..]
    );
    fs::write(&input, reversed_parts).expect("write aligned-parts score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "multiple Parts failed to render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("aligned-parts.svg")).expect("read SVG");
    assert!(
        !svg.contains("<text"),
        "score text must use glyph paths: {svg}"
    );
    assert_eq!(svg.matches("class=\"tonehead\"").count(), 26, "{svg}");
    assert_eq!(svg.matches("-barline-").count(), 4, "{svg}");
    assert!(
        svg.contains("id=\"part-name-0-glyph-0\""),
        "first Part name missing: {svg}"
    );
    assert!(
        svg.contains("id=\"part-name-1-glyph-0\""),
        "second Part name missing: {svg}"
    );
    assert!(
        svg.find("id=\"part-name-0-glyph-0\"") < svg.find("id=\"part-name-1-glyph-0\""),
        "Part names should follow part-list order: {svg}"
    );
    for part_name_id in ["part-name-0-glyph-0", "part-name-1-glyph-0"] {
        let glyph = svg
            .lines()
            .find(|line| line.contains(&format!("id=\"{part_name_id}\"")))
            .expect("Part-name glyph path");
        assert!(
            glyph.contains(" d=\"M"),
            "Part-name path must have glyph data: {glyph}"
        );
    }
    let first_part_tone = svg
        .lines()
        .find(|line| line.contains("id=\"part-0-tonehead\""))
        .expect("first Part tonehead");
    let second_part_tone = svg
        .lines()
        .find(|line| line.contains("id=\"part-1-tonehead\""))
        .expect("second Part tonehead");
    assert!(
        first_part_tone.contains("data-octave=\"5\""),
        "{first_part_tone}"
    );
    assert!(
        second_part_tone.contains("data-octave=\"3\""),
        "{second_part_tone}"
    );
    let beams = svg
        .lines()
        .filter(|line| line.contains("class=\"beam\""))
        .collect::<Vec<_>>();
    assert_eq!(
        beams.len(),
        8,
        "one primary and secondary beam per group: {svg}"
    );
    for beam in beams {
        let coordinates = beam
            .split(" d=\"M")
            .nth(1)
            .and_then(|path| path.split('"').next())
            .expect("beam path data");
        let endpoints = coordinates
            .split(['M', 'L'])
            .filter(|coordinate| !coordinate.is_empty())
            .take(2)
            .map(|coordinate| {
                coordinate
                    .split_whitespace()
                    .next()
                    .expect("beam x coordinate")
                    .parse::<f64>()
                    .expect("numeric beam x coordinate")
            })
            .collect::<Vec<_>>();
        assert_eq!(endpoints.len(), 2, "{beam}");
        assert!(
            (endpoints[1] - endpoints[0]).abs() > 30.0,
            "beam should continue across the group to its end declaration: {beam}"
        );
    }
    let beam_path = svg
        .lines()
        .find(|line| line.contains("id=\"part-1-beam-9-1\""))
        .expect("multi-octave primary beam path");
    let beam_points = beam_path
        .split(" d=\"M")
        .nth(1)
        .and_then(|path| path.split('"').next())
        .expect("beam path data")
        .split(['M', 'L'])
        .filter(|point| !point.is_empty())
        .take(2)
        .map(|point| {
            let mut coordinates = point.split_whitespace();
            (
                coordinates
                    .next()
                    .expect("beam x coordinate")
                    .parse::<f64>()
                    .expect("numeric beam x coordinate"),
                coordinates
                    .next()
                    .expect("beam y coordinate")
                    .parse::<f64>()
                    .expect("numeric beam y coordinate"),
            )
        })
        .collect::<Vec<_>>();
    for stem_id in ["part-1-stem-7", "part-1-stem-8"] {
        let stem_path = svg
            .lines()
            .find(|line| line.contains(&format!("id=\"{stem_id}\"")))
            .expect("continuing stem path");
        let stem_data = stem_path
            .split(" d=\"M")
            .nth(1)
            .and_then(|path| path.split('"').next())
            .expect("stem path data");
        let (stem_x, stem_y) = stem_data.split_once(' ').expect("stem move coordinates");
        let stem_x = stem_x.parse::<f64>().expect("numeric stem x coordinate");
        let stem_end_y = stem_y
            .split('V')
            .nth(1)
            .expect("stem endpoint")
            .parse::<f64>()
            .expect("numeric stem endpoint");
        let beam_y = beam_points[0].1
            + (beam_points[1].1 - beam_points[0].1) * (stem_x - beam_points[0].0)
                / (beam_points[1].0 - beam_points[0].0);
        assert!(
            (stem_end_y - beam_y).abs() < 0.1,
            "continuing stem should meet, not cross, the interpolated beam line: {stem_path}; {beam_path}"
        );
    }
    let barline_xs = svg
        .lines()
        .filter(|line| line.contains("-barline-"))
        .map(|line| {
            line.split(" d=\"M")
                .nth(1)
                .and_then(|path| path.split_whitespace().next())
                .expect("barline path has an x coordinate")
                .parse::<f64>()
                .expect("barline x coordinate is numeric")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        &barline_xs[..2],
        &barline_xs[2..],
        "ordinal barlines must align"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_invalid_part_identity_and_measure_counts() {
    let valid = include_str!("../examples/aligned-parts/aligned-parts.musicxml");
    let coda_start = valid
        .find("    <measure number=\"coda\">")
        .expect("coda measure");
    let coda_end = coda_start
        + valid[coda_start..]
            .find("    </measure>")
            .expect("coda end")
        + "    </measure>\n".len();
    let missing_coda = format!("{}{}", &valid[..coda_start], &valid[coda_end..]);
    let second_part_start = valid.find("  <part id=\"P2\">").expect("second Part");
    let second_part_end = second_part_start
        + valid[second_part_start..]
            .find("  </part>")
            .expect("second Part end")
        + "  </part>\n".len();
    let missing_part = format!(
        "{}{}",
        &valid[..second_part_start],
        &valid[second_part_end..]
    );
    let cases = [
        (
            valid.replace("<score-part id=\"P2\"", "<score-part id=\"P1\""),
            "duplicate score-part id",
        ),
        (
            valid.replace("<part id=\"P2\">", "<part id=\"P1\">"),
            "duplicate Part id",
        ),
        (
            valid.replace("<part id=\"P2\">", "<part id=\"P3\">"),
            "has no matching score-part",
        ),
        (missing_part, "has no matching Part"),
        (missing_coda, "measures; expected 2 to align"),
    ];
    for (index, (source, expected_error)) in cases.into_iter().enumerate() {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join(format!("invalid-{index}.musicxml"));
        fs::write(&input, source).expect("write invalid score");
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(
            !output.status.success(),
            "invalid Part structure was accepted"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_error),
            "expected {expected_error:?}, got: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::remove_dir_all(directory).expect("remove test directory");
    }
}

#[test]
fn moves_text_away_from_nearby_extreme_register_notation() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml");
    let normal_input = directory.join("normal-register.musicxml");
    fs::write(&normal_input, fixture).expect("write normal-register score");
    let high_fixture = fixture.replacen(
        "<step>C</step><octave>4</octave>",
        "<step>C</step><octave>8</octave>",
        1,
    );
    let high_input = directory.join("high-register.musicxml");
    fs::write(&high_input, high_fixture).expect("write high-register score");

    for input in [&normal_input, &high_input] {
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "score failed to render: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let normal_svg = fs::read_to_string(directory.join("normal-register.svg"))
        .expect("read normal-register SVG");
    let high_svg =
        fs::read_to_string(directory.join("high-register.svg")).expect("read high-register SVG");
    assert!(
        text_path_y(&high_svg, "text-0-glyph-0") < text_path_y(&normal_svg, "text-0-glyph-0"),
        "above-staff text should move away from extreme-register notation"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn moves_text_away_from_nearby_chord_symbols() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml");
    let normal_input = directory.join("without-chord.musicxml");
    fs::write(&normal_input, fixture).expect("write score without chord symbol");
    let chord_fixture = fixture.replacen(
        "</direction>\n      <note><pitch><step>C</step>",
        "</direction>\n      <harmony><root><root-step>C</root-step></root><kind>major</kind></harmony>\n      <note><pitch><step>C</step>",
        1,
    );
    let chord_input = directory.join("with-chord.musicxml");
    fs::write(&chord_input, chord_fixture).expect("write score with chord symbol");

    for input in [&normal_input, &chord_input] {
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "score failed to render: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let normal_svg = fs::read_to_string(directory.join("without-chord.svg"))
        .expect("read score without chord symbol");
    let chord_svg =
        fs::read_to_string(directory.join("with-chord.svg")).expect("read score with chord symbol");
    assert!(
        chord_svg.contains("class=\"chord-symbol-root\""),
        "chord symbol should render: {chord_svg}"
    );
    assert!(
        text_path_y(&chord_svg, "text-0-glyph-0") < text_path_y(&normal_svg, "text-0-glyph-0"),
        "above-staff text should move away from a nearby chord symbol"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn moves_long_text_away_from_notation_in_the_next_measure() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml").replace(
        "Andante, dolce",
        "A very long direction sentence that extends across the measure boundary",
    );
    let (prefix, second_measure) = fixture
        .split_once("<measure number=\"2\">")
        .expect("second measure exists");
    let high_fixture = format!(
        "{prefix}<measure number=\"2\">{}",
        second_measure.replacen(
            "<step>G</step><octave>4</octave>",
            "<step>G</step><octave>8</octave>",
            1,
        )
    );
    let normal_input = directory.join("normal-next-measure.musicxml");
    let high_input = directory.join("high-next-measure.musicxml");
    fs::write(&normal_input, fixture).expect("write normal score");
    fs::write(&high_input, high_fixture).expect("write high-register score");

    for input in [&normal_input, &high_input] {
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "score failed to render: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let normal_svg =
        fs::read_to_string(directory.join("normal-next-measure.svg")).expect("read normal score");
    let high_svg =
        fs::read_to_string(directory.join("high-next-measure.svg")).expect("read high score");
    assert!(
        text_path_y(&high_svg, "text-1-glyph-0") < text_path_y(&normal_svg, "text-1-glyph-0"),
        "long text should move away from notation in the next measure"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn includes_expanded_rehearsal_enclosure_in_svg_view_box() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("wide-rehearsal.musicxml");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml")
        .replace("<rehearsal>A</rehearsal>", "<rehearsal>WWWWWW</rehearsal>");
    fs::write(&input, fixture).expect("write wide rehearsal score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "wide rehearsal failed to render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("wide-rehearsal.svg")).expect("read SVG");
    let view_box_top = svg_view_box_top(&svg);
    let enclosure_top = path_move_y(&svg, "text-0-enclosure");
    assert!(
        view_box_top <= enclosure_top - 0.75,
        "viewBox top {view_box_top} must contain the enclosure stroke above {enclosure_top}"
    );
    let view_box_bottom = svg_view_box_bottom(&svg);
    assert!(
        view_box_bottom >= path_move_y(&svg, "text-0-enclosure") + 0.75,
        "viewBox bottom {view_box_bottom} must contain the enclosure stroke below"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn includes_negative_text_coordinates_in_svg_view_box() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("negative-text-coordinate.musicxml");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml").replace(
        "<words font-size=\"large\">Andante, dolce</words>",
        "<words default-x=\"-100\">Andante, dolce</words>",
    );
    fs::write(&input, fixture).expect("write negative-coordinate score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "negative text coordinate failed to render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("negative-text-coordinate.svg")).expect("read SVG");
    let view_box_left = svg_view_box_left(&svg);
    let text_left = text_path_x(&svg, "text-1-glyph-0");
    assert!(
        view_box_left <= text_left,
        "viewBox left {view_box_left} must include text path at x={text_left}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn moves_positioned_text_away_from_time_signature_glyphs() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("text-over-meter.musicxml");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml").replace(
        "<words font-size=\"large\">Andante, dolce</words>",
        "<words default-x=\"-10\" default-y=\"7\">Meter</words>",
    );
    fs::write(&input, fixture).expect("write positioned text score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "positioned text failed to render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("text-over-meter.svg")).expect("read SVG");
    assert!(
        text_path_y(&svg, "text-1-glyph-0") < 54.0,
        "text overlapping the opening meter should move above its authored y position"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn keeps_explicitly_positioned_text_on_its_coordinate_side_when_colliding() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml")
        .replace(
            "<words font-size=\"large\">Andante, dolce</words>",
            "<words default-x=\"-10\" default-y=\"7\">Meter</words>",
        )
        .replacen(
            "<direction placement=\"above\">\n        <direction-type><words",
            "<direction placement=\"below\">\n        <direction-type><words",
            1,
        );
    let input = directory.join("explicit-side-collision.musicxml");
    fs::write(&input, fixture).expect("write explicitly positioned score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "explicitly positioned text failed to render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("explicit-side-collision.svg")).expect("read SVG");
    assert!(
        text_path_y(&svg, "text-1-glyph-0") < 54.0,
        "text at default-y=7 should move upward away from the meter, regardless of direction placement"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_path_backed_symbol_directions() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("symbol-direction.musicxml");
    let symbols = [
        "restWhole",
        "restHalf",
        "restQuarter",
        "rest8th",
        "rest16th",
        "rest32nd",
        "rest64th",
        "flag8thUp",
        "flag8thDown",
        "flag16thUp",
        "flag16thDown",
        "flag32ndUp",
        "flag32ndDown",
        "flag64thUp",
        "flag64thDown",
    ]
    .map(|name| format!("<symbol>{name}</symbol>"))
    .join("");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml")
        .replace("<rehearsal>A</rehearsal>", &symbols);
    fs::write(&input, fixture).expect("write symbol direction score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "symbol direction failed to render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("<symbol> direction is not supported"),
        "supported symbol direction should not warn: {stderr}"
    );
    let svg = fs::read_to_string(directory.join("symbol-direction.svg")).expect("read SVG");
    assert!(
        svg.contains("id=\"direction-symbol-0\" class=\"direction-symbol\""),
        "symbol direction should render as a path: {svg}"
    );
    let first_symbol = svg
        .lines()
        .find(|line| line.contains("id=\"direction-symbol-0\""))
        .expect("first symbol path");
    assert!(
        first_symbol.contains("fill=\"#171717\"") && first_symbol.contains("fill-rule=\"nonzero\""),
        "symbol direction path should be filled: {first_symbol}"
    );
    assert_eq!(
        svg.matches("class=\"direction-symbol\"").count(),
        15,
        "every allowlisted Bravura symbol should render as a path"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_segno_and_coda_marks_in_source_order() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("segno-coda.musicxml");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml")
        .replace(
            "<rehearsal>A</rehearsal>",
            "<words>Start</words><segno smufl=\"segnoSerpent1\" color=\"#006699\" font-size=\"large\" default-x=\"7\" default-y=\"50\"/><words>Return</words><coda smufl=\"codaSquare\" color=\"#994400\"/><segno smufl=\"segnoUnlisted\"/>",
        )
        .replacen(
            "</direction-type>",
            "</direction-type><sound segno=\"jump\" coda=\"jump\"/>",
            1,
        );
    fs::write(&input, fixture).expect("write Segno and Coda score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "Segno and Coda marks failed to render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("segno-coda.svg")).expect("read SVG");
    let paths = [
        "id=\"text-0-glyph-0\"",
        "id=\"direction-segno-1\"",
        "id=\"text-2-glyph-0\"",
        "id=\"direction-coda-3\"",
    ];
    let positions = paths.map(|path| svg.find(path).expect("expected direction path"));
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "direction paths should preserve source order: {svg}"
    );
    let segno = svg
        .lines()
        .find(|line| line.contains("id=\"direction-segno-1\""))
        .expect("Segno path");
    assert!(
        segno.contains("fill=\"#006699\"") && segno.contains("scale(0.045)"),
        "Segno should honor color and font-size overrides: {segno}"
    );
    assert!(
        segno.contains("d=\"M202-1031")
            && text_path_y(&svg, "direction-segno-1") < 0.0
            && (100.0..115.0).contains(&text_path_x(&svg, "direction-segno-1")),
        "Segno should honor its glyph and explicit coordinates: {segno}"
    );
    let coda = svg
        .lines()
        .find(|line| line.contains("id=\"direction-coda-3\""))
        .expect("Coda path");
    assert!(
        coda.contains("fill=\"#994400\""),
        "Coda should honor its color override: {coda}"
    );
    assert!(
        coda.contains("d=\"M807-400") && coda.contains("fill=\"#994400\""),
        "Coda should honor its path-backed glyph and color overrides: {coda}"
    );
    assert!(
        svg.contains("id=\"direction-segno-4\" class=\"segno-mark\""),
        "valid unsupported override should fall back to the conventional mark"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("SMuFL glyph override \"segnoUnlisted\" is not supported"),
        "valid unsupported glyph override should warn and use the conventional path"
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("<sound>"),
        "playback navigation attributes should be silently ignored"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn aligns_coda_vertical_center_with_segno() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("aligned-navigation-marks.musicxml");
    let fixture = include_str!("../examples/segno-coda-directions/segno-coda-directions.musicxml");
    fs::write(&input, fixture).expect("write aligned navigation score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "navigation marks failed to render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("aligned-navigation-marks.svg")).expect("read SVG");
    let segno_center = text_path_y(&svg, "direction-segno-1") + 366.0 * 0.04;
    let coda_center = text_path_y(&svg, "direction-coda-5") + 370.0 * 0.04;
    assert!(
        (segno_center - coda_center).abs() < 0.01,
        "Segno center {segno_center} and Coda center {coda_center} should align"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn warns_and_preserves_siblings_for_valid_unsupported_symbols() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("unsupported-symbol.musicxml");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml").replace(
        "<rehearsal>A</rehearsal>",
        "<symbol>accidentalNatural</symbol><words>kept</words>",
    );
    fs::write(&input, fixture).expect("write unsupported symbol score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "valid unsupported symbol should warn and render siblings"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("SMuFL symbol \"accidentalNatural\" is not supported; omitted"),
        "unsupported symbol should produce a contextual warning"
    );
    let svg = fs::read_to_string(directory.join("unsupported-symbol.svg")).expect("read SVG");
    assert!(svg.contains("id=\"text-0-glyph-0\""));
    assert!(!svg.contains("id=\"direction-symbol-"));

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_malformed_symbol_and_mark_attributes_contextually() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    for (name, item, expected) in [
        (
            "symbol",
            "<symbol>bad name!</symbol>",
            "invalid SMuFL symbol name",
        ),
        ("segno", "<segno color=\"red\"/>", "symbol color must be"),
        (
            "coda",
            "<coda smufl=\"bad name\"/>",
            "invalid SMuFL glyph name",
        ),
    ] {
        let input = directory.join(format!("malformed-{name}.musicxml"));
        let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml")
            .replace("<rehearsal>A</rehearsal>", item);
        fs::write(&input, fixture).expect("write malformed direction score");
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "malformed {name} should fail: {stderr}"
        );
        assert!(
            stderr.contains(expected),
            "wrong {name} diagnostic: {stderr}"
        );
        assert!(
            !directory.join(format!("malformed-{name}.svg")).exists(),
            "malformed {name} must not emit partial SVG"
        );
    }

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn draws_bracket_enclosure_as_open_brackets() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml").replace(
        "<rehearsal>A</rehearsal>",
        "<rehearsal enclosure=\"bracket\">A</rehearsal>",
    );
    let input = directory.join("bracket-rehearsal.musicxml");
    fs::write(&input, fixture).expect("write bracket-enclosure score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "bracket enclosure failed to render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("bracket-rehearsal.svg")).expect("read SVG");
    let enclosure = svg
        .lines()
        .find(|line| line.contains("id=\"text-0-enclosure\""))
        .expect("bracket enclosure path");
    let path = enclosure
        .split(" d=\"")
        .nth(1)
        .expect("enclosure path data");
    assert!(
        !path.contains('Z'),
        "brackets should be open paths: {enclosure}"
    );
    assert_eq!(
        path.matches('M').count(),
        2,
        "expected two bracket strokes: {enclosure}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn scales_direction_offsets_consistently_with_divisions() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml")
        .replace("<divisions>1</divisions>", "<divisions>4</divisions>")
        .replace("<duration>1</duration>", "<duration>4</duration>");
    let with_offset = directory.join("with-offset.musicxml");
    let without_offset = directory.join("without-offset.musicxml");
    fs::write(&with_offset, &fixture).expect("write score with direction offset");
    fs::write(
        &without_offset,
        fixture.replace("<offset>1</offset>", "<offset>0</offset>"),
    )
    .expect("write score without direction offset");

    for input in [&with_offset, &without_offset] {
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "score failed to render: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let with_offset_svg =
        fs::read_to_string(directory.join("with-offset.svg")).expect("read score with offset");
    let without_offset_svg = fs::read_to_string(directory.join("without-offset.svg"))
        .expect("read score without offset");
    let offset_delta = text_path_x(&with_offset_svg, "text-2-glyph-0")
        - text_path_x(&without_offset_svg, "text-2-glyph-0");
    assert!(
        (offset_delta - 10.0).abs() < 0.01,
        "one division at divisions=4 should move text 10 SVG units, got {offset_delta}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn applies_relative_x_on_top_of_default_x() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml");
    let with_relative_x = fixture.replace(
        "font-size=\"large\"",
        "default-x=\"10\" relative-x=\"5\" font-size=\"large\"",
    );
    let without_relative_x = fixture.replace(
        "font-size=\"large\"",
        "default-x=\"10\" font-size=\"large\"",
    );
    let relative_input = directory.join("relative-x.musicxml");
    let default_input = directory.join("default-x.musicxml");
    fs::write(&relative_input, with_relative_x).expect("write relative-x score");
    fs::write(&default_input, without_relative_x).expect("write default-x score");

    for input in [&relative_input, &default_input] {
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "score failed to render: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let relative_svg =
        fs::read_to_string(directory.join("relative-x.svg")).expect("read relative-x SVG");
    let default_svg =
        fs::read_to_string(directory.join("default-x.svg")).expect("read default-x SVG");
    let x_delta =
        text_path_x(&relative_svg, "text-1-glyph-0") - text_path_x(&default_svg, "text-1-glyph-0");
    assert!(
        (x_delta - 10.0).abs() < 0.01,
        "relative-x=5 should add 10 SVG units on top of default-x, got {x_delta}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn separates_overlapping_text_with_opposite_authored_sides() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("opposite-side-text.musicxml");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml")
        .replace(
            "<rehearsal>A</rehearsal>",
            "<rehearsal default-y=\"-30\">A</rehearsal>",
        )
        .replace("font-size=\"large\"", "default-y=\"-30\"")
        .replacen(
            "</direction>\n      <note><pitch><step>C</step>",
            "</direction>\n      <direction placement=\"below\"><direction-type><words default-y=\"-30\">Below</words></direction-type></direction>\n      <note><pitch><step>C</step>",
            1,
        );
    fs::write(&input, fixture).expect("write opposite-side text score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "opposite-side text failed to render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("opposite-side-text.svg")).expect("read SVG");
    assert!(
        text_path_y(&svg, "text-2-glyph-0") > 128.0,
        "the later below-side item should move outward from its authored baseline"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn normalizes_text_to_nfc_before_glyph_lookup() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("normalized-text.musicxml");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml")
        .replace("Andante, dolce", "A\u{0301}");
    fs::write(&input, fixture).expect("write decomposed text score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "decomposed text failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("normalized-text.svg")).expect("read SVG");
    assert!(
        svg.contains("id=\"text-1-glyph-0\""),
        "normalized glyph missing: {svg}"
    );
    assert!(
        !svg.contains("id=\"text-1-glyph-1\""),
        "one NFC character should render as one glyph: {svg}"
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("unsupported text character"),
        "composable text should not warn: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn collapses_default_xml_whitespace_in_text() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("collapsed-whitespace.musicxml");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml")
        .replace("Andante, dolce", "  Andante \n\t dolce  ");
    fs::write(&input, fixture).expect("write whitespace text score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "default whitespace handling failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("collapsed-whitespace.svg")).expect("read SVG");
    assert_eq!(svg.matches("id=\"text-1-glyph-").count(), 12, "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn preserves_authored_whitespace_when_xml_space_is_preserve() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("preserved-whitespace.musicxml");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml").replace(
        "<words font-size=\"large\">Andante, dolce",
        "<words xml:space=\"preserve\">A \nB",
    );
    fs::write(&input, fixture).expect("write preserved whitespace score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "preserved whitespace handling failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("preserved-whitespace.svg")).expect("read SVG");
    assert!(
        svg.contains("id=\"text-1-glyph-3\""),
        "later line glyph missing: {svg}"
    );
    assert_ne!(
        text_path_y(&svg, "text-1-glyph-0"),
        text_path_y(&svg, "text-1-glyph-3"),
        "preserved newline should start a new line"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn omits_an_entire_valid_text_item_when_a_glyph_is_unsupported() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("unsupported-text.musicxml");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml")
        .replace("Andante, dolce", "A \u{1f600}");
    fs::write(&input, fixture).expect("write unsupported text score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "valid unsupported text should warn, not fail: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("unsupported-text.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"text-glyph\"").count(), 15, "{svg}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("U+1F600"),
        "missing glyph warning: {stderr}"
    );
    assert!(
        stderr.contains("XML path") && stderr.contains("direction-type/words"),
        "warning should identify the XML path: {stderr}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_malformed_text_formatting_without_writing_svg() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("malformed-text.musicxml");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml")
        .replace("font-size=\"large\"", "font-size=\"smaller\"");
    fs::write(&input, fixture).expect("write malformed text score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success(), "malformed font size must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid text font-size"), "{stderr}");
    assert!(
        stderr.contains("direction-type/words"),
        "diagnostic should include the words XML path: {stderr}"
    );
    assert!(
        !directory.join("malformed-text.svg").exists(),
        "fatal text errors must not write partial SVG"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn warns_and_falls_back_for_valid_unsupported_text_formatting() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("text-formatting.musicxml");
    let fixture = include_str!("../examples/words-rehearsal/words-rehearsal.musicxml").replace(
        "font-size=\"large\"",
        "font-family=\"Unbundled Face\" font-style=\"italic\" color=\"#AABBCC\" font-size=\"18pt\"",
    );
    fs::write(&input, fixture).expect("write formatted text score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "valid unsupported hints should warn and fall back: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("text-formatting.svg")).expect("read SVG");
    assert!(
        svg.contains("fill=\"#aabbcc\""),
        "supported color was lost: {svg}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("font-family"),
        "missing fallback warning: {stderr}"
    );
    assert!(stderr.contains("font-style is not supported"), "{stderr}");

    fs::remove_dir_all(directory).expect("remove test directory");
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
fn preserves_simultaneous_onsets_when_voice_geometry_overlaps() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("overlapping-voice-onsets.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let notes = "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>\n      <backup><duration>1</duration></backup>\n      <note><pitch><step>D</step><octave>5</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml").replace(original_note, notes);
    fs::write(&input, fixture).expect("write overlapping-onset score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "overlapping simultaneous Voices failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("overlapping-voice-onsets.svg")).expect("read SVG");
    let tonehead_x = |tone| {
        svg.lines()
            .find(|line| {
                line.contains("class=\"tonehead\"")
                    && line.contains(&format!("data-tone=\"{tone}\""))
            })
            .and_then(|line| line.split("transform=\"translate(").nth(1))
            .and_then(|transform| transform.split_whitespace().next())
            .and_then(|x| x.parse::<f64>().ok())
            .expect("tonehead position")
    };
    let tonehead_bounds = |tone| {
        let line = svg
            .lines()
            .find(|line| {
                line.contains("class=\"tonehead\"")
                    && line.contains(&format!("data-tone=\"{tone}\""))
            })
            .expect("tonehead path");
        let mut coordinates = line
            .split("transform=\"translate(")
            .nth(1)
            .and_then(|transform| transform.split(')').next())
            .expect("tonehead transform")
            .split_whitespace()
            .map(|coordinate| coordinate.parse::<f64>().expect("numeric coordinate"));
        let (x, y) = (
            coordinates.next().expect("tonehead x"),
            coordinates.next().expect("tonehead y"),
        );
        (x + 1.0, y + 2.0, x + 11.0, y + 16.0)
    };
    assert_eq!(
        tonehead_x(0),
        tonehead_x(2),
        "simultaneous onsets must align: {svg}"
    );
    for (stem_id, other_tone, own_tone, should_stem_up) in
        [("stem", 2, 0, false), ("stem-1", 0, 2, true)]
    {
        let line = svg
            .lines()
            .find(|line| line.contains(&format!("id=\"{stem_id}\"")))
            .expect("stem path");
        let stem_data = line
            .split(" d=\"M")
            .nth(1)
            .and_then(|path| path.split('"').next())
            .expect("stem path data");
        let (stem_x, stem_y) = stem_data.split_once(' ').expect("stem coordinates");
        let stem_x = stem_x.parse::<f64>().expect("numeric stem x");
        let (stem_start, stem_end) = stem_y.split_once('V').expect("stem endpoints");
        let stem_start = stem_start.parse::<f64>().expect("numeric stem start");
        let stem_end = stem_end.parse::<f64>().expect("numeric stem end");
        let (left, top, right, bottom) = tonehead_bounds(other_tone);
        let (_, own_top, _, own_bottom) = tonehead_bounds(own_tone);
        let stems_on_expected_side = if should_stem_up {
            stem_end < own_top
        } else {
            stem_start > own_bottom
        };
        assert!(
            stems_on_expected_side,
            "{stem_id} did not flip to the expected vertical side: {line}; {svg}"
        );
        assert!(
            stem_x < left
                || stem_x > right
                || stem_start.min(stem_end) > bottom
                || stem_start.max(stem_end) < top,
            "{stem_id} overlaps the other Voice's tonehead: {line}; {svg}"
        );
    }

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn later_events_clear_all_prior_simultaneous_voice_geometry() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("voice-onset-group-spacing.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let notes = "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>16th</type></note>\n      <backup><duration>1</duration></backup>\n      <note><pitch><step>G</step><octave>8</octave></pitch><duration>1</duration><voice>2</voice><type>16th</type></note>\n      <note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>2</voice><type>16th</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<divisions>1</divisions>", "<divisions>4</divisions>")
        .replace(original_note, notes);
    fs::write(&input, fixture).expect("write onset-group spacing score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "onset-group spacing failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg =
        fs::read_to_string(directory.join("voice-onset-group-spacing.svg")).expect("read SVG");
    let tonehead_x = |tone| {
        svg.lines()
            .find(|line| {
                line.contains("class=\"tonehead\"")
                    && line.contains(&format!("data-tone=\"{tone}\""))
            })
            .and_then(|line| line.split("transform=\"translate(").nth(1))
            .and_then(|transform| transform.split_whitespace().next())
            .and_then(|x| x.parse::<f64>().ok())
            .expect("tonehead position")
    };
    let first_onset_x = tonehead_x(0);
    assert_eq!(tonehead_x(7), first_onset_x, "{svg}");
    assert!(
        tonehead_x(4) > first_onset_x + 10.0,
        "later event overlaps the first onset group: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_parallel_voices_using_backup_cursor_movement() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("parallel-voices.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let notes = "      <note><pitch><step>C</step><octave>2</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>\n      <note><pitch><step>D</step><octave>2</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>\n      <backup><duration>2</duration></backup>\n      <note><pitch><step>G</step><octave>8</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type></note>\n      <note><pitch><step>A</step><octave>8</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml").replace(original_note, notes);
    fs::write(&input, fixture).expect("write parallel-voice score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "parallel Voices failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("parallel-voices.svg")).expect("read SVG");
    let tonehead_xs = |octave| {
        svg.lines()
            .filter(|line| {
                line.contains("class=\"tonehead\"")
                    && line.contains(&format!("data-octave=\"{octave}\""))
            })
            .map(|line| {
                line.split("transform=\"translate(")
                    .nth(1)
                    .and_then(|transform| transform.split_whitespace().next())
                    .and_then(|x| x.parse::<f64>().ok())
                    .expect("numeric tonehead position")
            })
            .collect::<Vec<_>>()
    };
    let first_voice_xs = tonehead_xs(2);
    let second_voice_xs = tonehead_xs(8);
    assert_eq!(first_voice_xs.len(), 2, "{svg}");
    assert_eq!(second_voice_xs.len(), 2, "{svg}");
    assert_eq!(first_voice_xs[0], second_voice_xs[0], "{svg}");
    assert_eq!(first_voice_xs[1], second_voice_xs[1], "{svg}");
    assert_eq!(first_voice_xs[1] - first_voice_xs[0], 40.0, "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_multiple_voices_example() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("multiple-voices.musicxml");
    fs::write(
        &input,
        include_str!("../examples/multiple-voices/multiple-voices.musicxml"),
    )
    .expect("write multiple-Voice example");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "multiple-Voice example failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("multiple-voices.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"tonehead\"").count(), 9, "{svg}");
    let first_barline_x = svg
        .lines()
        .find(|line| line.contains("id=\"barline-1\""))
        .and_then(|line| line.split("d=\"M").nth(1))
        .and_then(|path| path.split_whitespace().next())
        .and_then(|x| x.parse::<f64>().ok())
        .expect("first measure barline position");
    let second_measure_start_x = svg
        .lines()
        .find(|line| {
            line.contains("class=\"tonehead\"")
                && line.contains("data-tone=\"5\"")
                && line.contains("data-octave=\"4\"")
        })
        .and_then(|line| line.split("transform=\"translate(").nth(1))
        .and_then(|transform| transform.split_whitespace().next())
        .and_then(|x| x.parse::<f64>().ok())
        .expect("first note of second measure position");
    assert_eq!(second_measure_start_x, first_barline_x + 4.0, "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn keeps_interleaved_beam_groups_independent_by_voice() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("voice-beams.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let notes = "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type><beam>begin</beam></note>\n      <backup><duration>1</duration></backup>\n      <note><pitch><step>G</step><octave>8</octave></pitch><duration>1</duration><voice>2</voice><type>eighth</type><beam>begin</beam></note>\n      <note><pitch><step>A</step><octave>8</octave></pitch><duration>1</duration><voice>2</voice><type>eighth</type><beam>end</beam></note>\n      <backup><duration>1</duration></backup>\n      <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type><beam>end</beam></note>";
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<divisions>1</divisions>", "<divisions>2</divisions>")
        .replace(original_note, notes);
    fs::write(&input, fixture).expect("write interleaved-beam score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "interleaved Voice beams failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("voice-beams.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"beam\"").count(), 2, "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn does_not_match_ties_across_voices() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("cross-voice-tie.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let notes = "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><tie type=\"start\"/><voice>1</voice><type>quarter</type></note>\n      <backup><duration>1</duration></backup>\n      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><tie type=\"stop\"/><voice>2</voice><type>quarter</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml").replace(original_note, notes);
    fs::write(&input, fixture).expect("write cross-Voice-tie score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success(), "cross-Voice tie was accepted");
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("tie stop has no preceding tie start"));
    assert!(diagnostic.contains("/measure[@number='1']/note"));
    assert!(!directory.join("cross-voice-tie.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_forward_cursor_without_creating_an_event() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("forward-cursor.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let notes = "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>\n      <attributes><divisions>2</divisions></attributes>\n      <forward><duration>2</duration><voice>1</voice></forward>\n      <note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml").replace(original_note, notes);
    fs::write(&input, fixture).expect("write forward-cursor score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "forward cursor movement failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("forward-cursor.svg")).expect("read SVG");
    let tonehead_xs = svg
        .lines()
        .filter(|line| line.contains("class=\"tonehead\""))
        .map(|line| {
            line.split("transform=\"translate(")
                .nth(1)
                .and_then(|transform| transform.split_whitespace().next())
                .and_then(|x| x.parse::<f64>().ok())
                .expect("numeric tonehead position")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        tonehead_xs.len(),
        2,
        "forward must not create an event: {svg}"
    );
    assert_eq!(tonehead_xs[1] - tonehead_xs[0], 80.0, "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn accepts_a_single_voice_without_voice_identifiers() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("implicit-voice.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace("<voice>1</voice>", "");
    fs::write(&input, fixture).expect("write implicit-Voice score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "an omitted ID for a single Voice was rejected: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("implicit-voice.svg").is_file());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn requires_voice_identifiers_when_multiple_voices_exist() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("missing-voice-id.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let notes = "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note>\n      <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>\n      <backup><duration>2</duration></backup>\n      <note><pitch><step>G</step><octave>8</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml").replace(original_note, notes);
    fs::write(&input, fixture).expect("write missing-Voice-ID score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success(), "missing Voice ID was accepted");
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("multiple Voices require an explicit <voice>"));
    assert!(diagnostic.contains("/measure[@number='1']/note"));
    assert!(!directory.join("missing-voice-id.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_malformed_cursor_movement_without_output() {
    let cases = [
        (
            "backup-underflow",
            "<backup><duration>1</duration></backup>",
            "before the measure start",
        ),
        (
            "zero-forward",
            "<forward><duration>0</duration></forward>",
            "positive integer",
        ),
        (
            "missing-forward-duration",
            "<forward/>",
            "requires <duration>",
        ),
    ];
    for (name, movement, expected_message) in cases {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join(format!("{name}.musicxml"));
        let fixture = include_str!("fixtures/minimal.musicxml")
            .replace("      <note>", &format!("      {movement}\n      <note>"));
        fs::write(&input, fixture).expect("write malformed-cursor score");

        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(!output.status.success(), "{name} was accepted");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostic.contains(expected_message), "{diagnostic}");
        assert!(diagnostic.contains("/measure[@number='1']"), "{diagnostic}");
        assert!(!directory.join(format!("{name}.svg")).exists());

        fs::remove_dir_all(directory).expect("remove test directory");
    }
}

#[test]
fn renders_root_position_and_bass_tone_chord_symbols() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("chord-symbols.musicxml");
    fs::write(&input, include_str!("fixtures/chord-symbols.musicxml"))
        .expect("write chord-symbol fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "chord-symbol score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("chord-symbols.svg")).expect("read SVG");
    assert_eq!(
        svg.matches("class=\"chord-symbol-root\"").count(),
        2,
        "{svg}"
    );
    assert_eq!(
        svg.matches("class=\"chord-symbol-bass\"").count(),
        1,
        "{svg}"
    );
    assert_eq!(
        svg.matches("class=\"chord-symbol-tone\"").count(),
        5,
        "{svg}"
    );
    assert!(
        !svg.contains("data-chord-role=\"bass\" data-tone=\"0\""),
        "root-position chord must not repeat its root as a bass tone: {svg}"
    );
    let path_x = |id: &str| {
        svg.lines()
            .find(|line| line.contains(&format!("id=\"{id}\"")))
            .and_then(|line| line.split("translate(").nth(1))
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse::<f64>().ok())
            .expect("chord symbol x position")
    };
    assert!(
        path_x("chord-symbol-1-root") >= path_x("chord-symbol-0-tone-2") + 22.0,
        "measure boundary should leave clearance after the preceding chord symbol: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn warns_and_omits_valid_but_unsupported_harmony_kinds() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("unsupported-harmony.musicxml");
    let fixture = include_str!("fixtures/chord-symbols.musicxml")
        .replace("<kind>major-seventh</kind>", "<kind>minor-ninth</kind>");
    fs::write(&input, fixture).expect("write unsupported-harmony score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "valid unsupported harmony should not fail: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning"), "expected a warning: {stderr}");
    let svg = fs::read_to_string(directory.join("unsupported-harmony.svg")).expect("read SVG");
    assert!(
        !svg.contains("class=\"chord-symbol-root\""),
        "unsupported chord symbols should be omitted: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn applies_harmony_add_alter_and_subtract_degrees() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("harmony-degrees.musicxml");
    let degrees = "<degree><degree-value>9</degree-value><degree-alter>0</degree-alter><degree-type>add</degree-type></degree>\n        <degree><degree-value>5</degree-value><degree-alter>-1</degree-alter><degree-type>alter</degree-type></degree>\n        <degree><degree-value>3</degree-value><degree-alter>-1</degree-alter><degree-type>subtract</degree-type></degree>";
    let fixture = include_str!("fixtures/chord-symbols.musicxml")
        .replace(
            "</kind>\n      </harmony>",
            &format!("</kind>\n        {degrees}\n      </harmony>"),
        )
        .replace(
            "</bass>\n      </harmony>",
            &format!("</bass>\n        {degrees}\n      </harmony>"),
        );
    fs::write(&input, fixture).expect("write degree-modified harmony score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "degree-modified harmony failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("harmony-degrees.svg")).expect("read SVG");
    assert!(
        svg.contains("class=\"chord-symbol-tone\" data-chord-role=\"tone\" data-tone=\"2\""),
        "{svg}"
    );
    assert!(
        svg.contains("class=\"chord-symbol-tone\" data-chord-role=\"tone\" data-tone=\"6\""),
        "{svg}"
    );
    assert_eq!(
        svg.matches("data-tone=\"11\"").count(),
        2,
        "root-position + bass deduplication: {svg}"
    );
    assert_eq!(
        svg.matches("class=\"chord-symbol-tone\"").count(),
        5,
        "{svg}"
    );
    let first_symbol_tones = (0..3)
        .map(|index| {
            svg.lines()
                .find(|line| line.contains(&format!("id=\"chord-symbol-0-tone-{index}\"")))
                .and_then(|line| line.split("data-tone=\"").nth(1))
                .and_then(|value| value.split('"').next())
                .expect("tone pitch class")
                .parse::<u8>()
                .expect("numeric tone pitch class")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        first_symbol_tones,
        [6, 11, 2],
        "extensions follow interval order: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn maps_enharmonic_root_and_bass_to_the_same_tone() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("enharmonic-harmony.musicxml");
    let fixture = include_str!("fixtures/chord-symbols.musicxml")
        .replace(
            "<root-step>C</root-step>",
            "<root-step>D</root-step><root-alter>-1.0</root-alter>",
        )
        .replace("<kind>major-seventh</kind>", "<kind>major</kind>")
        .replace(
            "<bass-step>B</bass-step>",
            "<bass-step>C</bass-step><bass-alter>1.0</bass-alter>",
        );
    fs::write(&input, fixture).expect("write enharmonic-harmony score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "enharmonic harmony failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("enharmonic-harmony.svg")).expect("read SVG");
    assert_eq!(
        svg.matches("class=\"chord-symbol-root\"").count(),
        2,
        "{svg}"
    );
    assert!(!svg.contains("class=\"chord-symbol-bass\""), "{svg}");
    assert_eq!(svg.matches("data-tone=\"1\"").count(), 2, "{svg}");
    assert_eq!(
        svg.matches("class=\"chord-symbol-tone\"").count(),
        4,
        "{svg}"
    );
    assert!(
        svg.contains("class=\"chord-symbol-tone\" data-chord-role=\"tone\" data-tone=\"5\""),
        "{svg}"
    );
    assert!(
        svg.contains("class=\"chord-symbol-tone\" data-chord-role=\"tone\" data-tone=\"8\""),
        "{svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn positions_harmony_using_musicxml_offset() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("harmony-offset.musicxml");
    let fixture = include_str!("fixtures/chord-symbols.musicxml")
        .replace("</harmony>", "<offset>1</offset>\n      </harmony>");
    fs::write(&input, fixture).expect("write harmony-offset score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "harmony offset failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("harmony-offset.svg")).expect("read SVG");
    assert!(
        svg.contains("id=\"chord-symbol-0-root\" class=\"chord-symbol-root\" data-chord-role=\"root\" data-tone=\"0\" transform=\"translate(132.00 -20) scale(2)\""),
        "one division at divisions=1 should move the harmony 40 SVG units: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn honors_print_suppression_and_warns_for_implied_harmony() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("harmony-display-control.musicxml");
    let fixture = include_str!("fixtures/chord-symbols.musicxml")
        .replacen("<harmony>", "<harmony print-object=\"no\">", 1)
        .replacen("<harmony>", "<harmony type=\"implied\">", 1);
    fs::write(&input, fixture).expect("write display-control score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "valid harmony display controls failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("implied"),
        "expected implied warning: {stderr}"
    );
    assert_eq!(stderr.matches("warning").count(), 1, "{stderr}");
    let svg = fs::read_to_string(directory.join("harmony-display-control.svg")).expect("read SVG");
    assert!(!svg.contains("class=\"chord-symbol-root\""), "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn separates_multiple_harmonies_at_the_same_score_position() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("multiple-harmonies.musicxml");
    let fixture = include_str!("fixtures/chord-symbols.musicxml").replacen(
        "</harmony>",
        "</harmony>\n      <harmony><root><root-step>D</root-step></root><kind>minor</kind></harmony>",
        1,
    );
    fs::write(&input, fixture).expect("write multiple-harmony score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "multiple harmonies failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("multiple-harmonies.svg")).expect("read SVG");
    let root_x = |index| {
        svg.lines()
            .find(|line| line.contains(&format!("id=\"chord-symbol-{index}-root\"")))
            .and_then(|line| line.split("translate(").nth(1))
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse::<f64>().ok())
            .expect("root x position")
    };
    assert_eq!(
        svg.matches("class=\"chord-symbol-root\"").count(),
        3,
        "{svg}"
    );
    assert!(root_x(1) - root_x(0) >= 72.0, "symbols overlap: {svg}");
    assert!(
        svg.find("id=\"chord-symbol-0-root\"") < svg.find("id=\"chord-symbol-1-root\""),
        "same-position symbols should preserve MusicXML order: {svg}"
    );

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
fn warns_for_valid_kind_display_options_and_rejects_malformed_values() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("kind-display.musicxml");
    let fixture = include_str!("fixtures/chord-symbols.musicxml").replace(
        "<kind>major-seventh</kind>",
        "<kind text=\"M7\" use-symbols=\"no\">major-seventh</kind>",
    );
    fs::write(&input, fixture).expect("write kind-display score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "valid kind display attributes failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("display attributes"));
    let svg = fs::read_to_string(directory.join("kind-display.svg")).expect("read SVG");
    assert_eq!(
        svg.matches("class=\"chord-symbol-root\"").count(),
        2,
        "{svg}"
    );

    let malformed = directory.join("malformed-kind-display.musicxml");
    fs::write(
        &malformed,
        include_str!("fixtures/chord-symbols.musicxml").replace(
            "<kind>major-seventh</kind>",
            "<kind use-symbols=\"sometimes\">major-seventh</kind>",
        ),
    )
    .expect("write malformed kind-display score");
    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&malformed)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success(),
        "invalid use-symbols value was accepted"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("use-symbols must be 'yes' or 'no'"),
        "{stderr}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn warns_and_omits_numeral_harmony_forms() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("numeral-harmony.musicxml");
    let fixture = include_str!("fixtures/chord-symbols.musicxml").replace(
        "<root><root-step>C</root-step></root>",
        "<numeral><numeral-root>5</numeral-root></numeral>",
    );
    fs::write(&input, fixture).expect("write numeral-harmony score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "valid numeral harmony should not fail: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("<numeral> harmony"));
    let svg = fs::read_to_string(directory.join("numeral-harmony.svg")).expect("read SVG");
    assert!(!svg.contains("class=\"chord-symbol-root\""), "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn warns_and_omits_function_harmony_forms() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("function-harmony.musicxml");
    let fixture = include_str!("fixtures/chord-symbols.musicxml").replace(
        "<root><root-step>C</root-step></root>",
        "<function>V</function>",
    );
    fs::write(&input, fixture).expect("write function-harmony score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "valid function harmony should not fail: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("<function> harmony"));
    let svg = fs::read_to_string(directory.join("function-harmony.svg")).expect("read SVG");
    assert!(!svg.contains("class=\"chord-symbol-root\""), "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_malformed_function_harmony_content() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("malformed-function-harmony.musicxml");
    let fixture = include_str!("fixtures/chord-symbols.musicxml").replace(
        "<root><root-step>C</root-step></root>",
        "<function><unexpected/></function>",
    );
    fs::write(&input, fixture).expect("write malformed function-harmony score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success(),
        "malformed function harmony was accepted"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("<function>"), "{stderr}");
    assert!(stderr.contains("unexpected"), "{stderr}");
    assert!(!directory.join("malformed-function-harmony.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_numeral_harmony_without_required_root() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("malformed-numeral-harmony.musicxml");
    let fixture = include_str!("fixtures/chord-symbols.musicxml").replace(
        "<root><root-step>C</root-step></root>",
        "<numeral></numeral>",
    );
    fs::write(&input, fixture).expect("write malformed numeral-harmony score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success(),
        "malformed numeral harmony was accepted"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("<numeral>"), "{stderr}");
    assert!(stderr.contains("numeral-root"), "{stderr}");
    assert!(!directory.join("malformed-numeral-harmony.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn routes_numbered_harmony_and_warns_about_position_hints() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("harmony-hints.musicxml");
    let fixture = include_str!("fixtures/chord-symbols.musicxml")
        .replace(
            "<attributes><divisions>1</divisions></attributes>",
            "<attributes><divisions>1</divisions><staves>2</staves></attributes>",
        )
        .replacen(
            "<harmony>",
            "<harmony placement=\"below\" default-x=\"12\" default-y=\"4\" relative-x=\"1\" relative-y=\"-1\">",
            1,
        )
        .replacen("</harmony>", "<staff>2</staff></harmony>", 1);
    fs::write(&input, fixture).expect("write harmony-hints score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .arg("--input-staff-mode")
        .arg("separate")
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "valid position/staff hints failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("placement"), "{stderr}");
    assert!(stderr.contains("default-x"), "{stderr}");
    assert!(stderr.contains("default-y"), "{stderr}");
    assert!(stderr.contains("relative-x"), "{stderr}");
    assert!(stderr.contains("relative-y"), "{stderr}");
    assert!(!stderr.contains("harmony staff"), "{stderr}");
    let first_staff =
        fs::read_to_string(directory.join("harmony-hints-staff-1.svg")).expect("read SVG");
    let second_staff =
        fs::read_to_string(directory.join("harmony-hints-staff-2.svg")).expect("read SVG");
    assert_eq!(
        first_staff.matches("class=\"chord-symbol-root\"").count(),
        1
    );
    assert_eq!(
        second_staff.matches("class=\"chord-symbol-root\"").count(),
        2
    );
    assert!(
        second_staff.contains("transform=\"translate(92 -20) scale(2)\""),
        "{second_staff}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_fractional_harmony_alterations() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("fractional-harmony.musicxml");
    let fixture = include_str!("fixtures/chord-symbols.musicxml").replace(
        "<root-step>C</root-step>",
        "<root-step>C</root-step><root-alter>0.5</root-alter>",
    );
    fs::write(&input, fixture).expect("write fractional-harmony score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success(),
        "fractional pitch alteration was accepted"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("pitch alteration must be an integer"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let fractional_bass = directory.join("fractional-bass.musicxml");
    fs::write(
        &fractional_bass,
        include_str!("fixtures/chord-symbols.musicxml").replace(
            "<bass-step>B</bass-step>",
            "<bass-step>B</bass-step><bass-alter>0.5</bass-alter>",
        ),
    )
    .expect("write fractional-bass score");
    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&fractional_bass)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success(),
        "fractional bass alteration was accepted"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("pitch alteration must be an integer"));

    let fractional_degree = directory.join("fractional-degree.musicxml");
    let degree = "<degree><degree-value>9</degree-value><degree-alter>0.5</degree-alter><degree-type>add</degree-type></degree>";
    fs::write(
        &fractional_degree,
        include_str!("fixtures/chord-symbols.musicxml")
            .replace("</kind>", &format!("</kind>{degree}")),
    )
    .expect("write fractional-degree score");
    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&fractional_degree)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success(),
        "fractional degree alteration was accepted"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("degree alteration must be an integer")
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn warns_and_omits_chords_with_absent_or_conflicting_degrees() {
    for degree in [
        "<degree><degree-value>9</degree-value><degree-alter>0</degree-alter><degree-type>alter</degree-type></degree>",
        "<degree><degree-value>4</degree-value><degree-alter>0</degree-alter><degree-type>add</degree-type></degree><degree><degree-value>4</degree-value><degree-alter>1</degree-alter><degree-type>alter</degree-type></degree>",
    ] {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join("invalid-degree.musicxml");
        let fixture = include_str!("fixtures/chord-symbols.musicxml")
            .replace("</kind>", &format!("</kind>{degree}"));
        fs::write(&input, fixture).expect("write invalid-degree score");

        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "unsupported degree should warn, not fail: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("warning"));
        let svg = fs::read_to_string(directory.join("invalid-degree.svg")).expect("read SVG");
        assert!(!svg.contains("class=\"chord-symbol-root\""), "{svg}");
        fs::remove_dir_all(directory).expect("remove test directory");
    }
}

#[test]
fn supports_the_agreed_musicxml_harmony_kinds() {
    for kind in [
        "major",
        "minor",
        "augmented",
        "diminished",
        "dominant",
        "major-seventh",
        "minor-seventh",
        "half-diminished",
        "diminished-seventh",
        "suspended-second",
        "suspended-fourth",
    ] {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join("supported-kind.musicxml");
        let fixture =
            include_str!("fixtures/chord-symbols.musicxml").replace("major-seventh", kind);
        fs::write(&input, fixture).expect("write supported-kind score");

        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "supported kind {kind:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "supported kind {kind:?} should not warn: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let svg = fs::read_to_string(directory.join("supported-kind.svg")).expect("read SVG");
        assert_eq!(
            svg.matches("class=\"chord-symbol-root\"").count(),
            2,
            "{kind}: {svg}"
        );
        fs::remove_dir_all(directory).expect("remove test directory");
    }
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
fn grace_and_cue_notes_follow_their_distinct_cursor_rules() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("grace-cue-timing.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let notes = r#"      <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>quarter</type></note>
    <note><grace slash="yes"/><pitch><step>D</step><octave>4</octave></pitch><voice>1</voice></note>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>quarter</type></note>
      <note><cue/><pitch><step>F</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type size="cue">quarter</type></note>
      <note><grace/><cue/><pitch><step>G</step><octave>4</octave></pitch><voice>1</voice><type size="grace-cue">eighth</type></note>
      <note><pitch><step>A</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>B</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>eighth</type></note>"#;
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(original_note, notes)
        .replace("<divisions>1</divisions>", "<divisions>4</divisions>")
        .replace(
            "<attributes><divisions>4</divisions></attributes>",
            "<attributes><divisions>4</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>",
        );
    fs::write(&input, &fixture).expect("write grace-cue timing score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "grace-cue timing score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("underfull") && !stderr.contains("overfull"),
        "grace/cue cursor handling should fill 4/4 exactly: {stderr}"
    );
    let svg = fs::read_to_string(directory.join("grace-cue-timing.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"tonehead\"").count(), 7, "{svg}");
    let tonehead = |tone: u8| {
        svg.lines()
            .find(|line| {
                line.contains("class=\"tonehead\"")
                    && line.contains(&format!("data-tone=\"{tone}\""))
            })
            .expect("tonehead path")
    };
    assert!(
        tonehead(2).contains("scale(0.6)"),
        "Grace Note size: {}",
        tonehead(2)
    );
    assert!(
        tonehead(5).contains("scale(0.6)"),
        "Cue note size: {}",
        tonehead(5)
    );
    assert!(
        tonehead(7).contains("scale(0.6)"),
        "Grace+Cue size: {}",
        tonehead(7)
    );
    assert!(
        !tonehead(0).contains("scale("),
        "full-size note: {}",
        tonehead(0)
    );
    let tonehead_x = |line: &str| {
        let start = line.find("translate(").expect("tonehead transform") + 10;
        let end = line[start..].find(' ').expect("x/y separator") + start;
        line[start..end]
            .parse::<f64>()
            .expect("tonehead x coordinate")
    };
    assert!(
        tonehead_x(tonehead(2)) < tonehead_x(tonehead(4)),
        "Grace Note should precede its principal note: {} then {}",
        tonehead(2),
        tonehead(4)
    );
    assert!(
        svg.contains("id=\"flag-1\" class=\"flag\""),
        "Grace default flag: {svg}"
    );
    assert!(svg.contains("class=\"grace-slash\""), "Grace slash: {svg}");

    let tied_path = directory.join("grace-tie.musicxml");
    let tied_source = fixture
        .replace(
            "<note><grace slash=\"yes\"/><pitch><step>D</step><octave>4</octave></pitch><voice>1</voice></note>",
            "<note><grace slash=\"yes\"/><pitch><step>D</step><octave>4</octave></pitch><tie type=\"start\"/><voice>1</voice><type>eighth</type><notations><tied type=\"start\"/></notations></note>",
        )
        .replace(
            "<note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>quarter</type></note>",
            "<note><pitch><step>D</step><octave>4</octave></pitch><duration>4</duration><tie type=\"stop\"/><voice>1</voice><type>quarter</type><notations><tied type=\"stop\"/></notations></note>",
        );
    fs::write(&tied_path, tied_source).expect("write Grace tie score");
    let tied_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&tied_path)
        .output()
        .expect("run CLI for Grace tie");
    assert!(
        tied_output.status.success(),
        "Grace tie failed: {}",
        String::from_utf8_lossy(&tied_output.stderr)
    );
    let tied_svg = fs::read_to_string(directory.join("grace-tie.svg")).expect("read Grace tie SVG");
    assert!(
        tied_svg.contains("class=\"tie\""),
        "visual tie missing: {tied_svg}"
    );

    let cue_rest_input = directory.join("cue-rest.musicxml");
    let cue_rest_source = fixture.replace(
        "<cue/><pitch><step>F</step><octave>4</octave></pitch>",
        "<cue/><rest/>",
    );
    fs::write(&cue_rest_input, cue_rest_source).expect("write Cue Rest score");
    let cue_rest_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&cue_rest_input)
        .output()
        .expect("run CLI for Cue Rest");
    assert!(
        cue_rest_output.status.success(),
        "Cue Rest failed: {}",
        String::from_utf8_lossy(&cue_rest_output.stderr)
    );
    let cue_rest_svg =
        fs::read_to_string(directory.join("cue-rest.svg")).expect("read Cue Rest SVG");
    assert!(
        cue_rest_svg
            .lines()
            .any(|line| { line.contains("class=\"rest\"") && line.contains("scale(0.6)") }),
        "Cue Rest should use Cue scale: {cue_rest_svg}"
    );

    let cue_unpitched_input = directory.join("cue-unpitched.musicxml");
    let cue_unpitched_source = fixture.replace(
        "<cue/><pitch><step>F</step><octave>4</octave></pitch>",
        "<cue/><unpitched><display-step>F</display-step><display-octave>5</display-octave></unpitched>",
    );
    fs::write(&cue_unpitched_input, cue_unpitched_source).expect("write Cue Unpitched score");
    let cue_unpitched_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&cue_unpitched_input)
        .output()
        .expect("run CLI for Cue Unpitched");
    assert!(
        cue_unpitched_output.status.success(),
        "Cue Unpitched failed: {}",
        String::from_utf8_lossy(&cue_unpitched_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&cue_unpitched_output.stderr).contains("display-step is ignored"),
        "Cue Unpitched should warn about display-step"
    );
    let cue_unpitched_svg =
        fs::read_to_string(directory.join("cue-unpitched.svg")).expect("read Cue Unpitched SVG");
    assert!(
        cue_unpitched_svg.contains("class=\"unpitched-mark\"")
            && cue_unpitched_svg.contains("scale(0.6)"),
        "Cue Unpitched should use the scaled X mark: {cue_unpitched_svg}"
    );

    let grace_unpitched_input = directory.join("grace-unpitched.musicxml");
    let grace_unpitched_source = fixture.replace(
        "<grace slash=\"yes\"/><pitch><step>D</step><octave>4</octave></pitch>",
        "<grace slash=\"yes\"/><unpitched><display-octave>5</display-octave></unpitched>",
    );
    fs::write(&grace_unpitched_input, grace_unpitched_source).expect("write Grace Unpitched score");
    let grace_unpitched_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&grace_unpitched_input)
        .output()
        .expect("run CLI for Grace Unpitched");
    assert!(
        grace_unpitched_output.status.success(),
        "Grace Unpitched failed: {}",
        String::from_utf8_lossy(&grace_unpitched_output.stderr)
    );
    let grace_unpitched_svg = fs::read_to_string(directory.join("grace-unpitched.svg"))
        .expect("read Grace Unpitched SVG");
    assert!(
        grace_unpitched_svg.lines().any(|line| {
            line.contains("class=\"unpitched-mark\"") && line.contains("scale(0.6)")
        }),
        "Grace Unpitched should use Grace scale: {grace_unpitched_svg}"
    );
    assert!(grace_unpitched_svg.contains("class=\"grace-slash\""));

    let grace_rest_input = directory.join("grace-rest.musicxml");
    let grace_rest_source = fixture.replace(
        "<grace slash=\"yes\"/><pitch><step>D</step><octave>4</octave></pitch>",
        "<grace slash=\"yes\"/><rest/>",
    );
    fs::write(&grace_rest_input, grace_rest_source).expect("write Grace Rest score");
    let grace_rest_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&grace_rest_input)
        .output()
        .expect("run CLI for Grace Rest");
    assert!(
        !grace_rest_output.status.success()
            && String::from_utf8_lossy(&grace_rest_output.stderr)
                .contains("Grace Notes may not be Rests"),
        "Grace Rest should be rejected: {}",
        String::from_utf8_lossy(&grace_rest_output.stderr)
    );

    let note_size_input = directory.join("cue-note-size-default.musicxml");
    let note_size_source = fixture.replace(
        "<score-partwise version=\"4.0\">",
        "<score-partwise version=\"4.0\"><defaults><appearance><note-size type=\"cue\">80</note-size></appearance></defaults>",
    );
    fs::write(&note_size_input, note_size_source).expect("write note-size defaults score");
    let note_size_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&note_size_input)
        .output()
        .expect("run CLI for note-size defaults");
    assert!(
        note_size_output.status.success(),
        "note-size defaults failed: {}",
        String::from_utf8_lossy(&note_size_output.stderr)
    );
    let note_size_svg = fs::read_to_string(directory.join("cue-note-size-default.svg"))
        .expect("read note-size defaults SVG");
    let cue_notehead = note_size_svg
        .lines()
        .find(|line| line.contains("class=\"tonehead\"") && line.contains("data-tone=\"5\""))
        .expect("Cue Tonehead with note-size default");
    assert!(cue_notehead.contains("scale(0.8)"), "{cue_notehead}");

    for (category, expected_scale) in [("full", None), ("large", Some("scale(1.25)"))] {
        let variant_input = directory.join(format!("cue-{category}.musicxml"));
        let variant_source = fixture.replace("size=\"cue\"", &format!("size=\"{category}\""));
        fs::write(&variant_input, variant_source).expect("write Cue size variant");
        let variant_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&variant_input)
            .output()
            .expect("run CLI for Cue size variant");
        assert!(
            variant_output.status.success(),
            "Cue size {category:?} failed: {}",
            String::from_utf8_lossy(&variant_output.stderr)
        );
        let variant_svg = fs::read_to_string(directory.join(format!("cue-{category}.svg")))
            .expect("read Cue size SVG");
        let cue_tonehead = variant_svg
            .lines()
            .find(|line| line.contains("class=\"tonehead\"") && line.contains("data-tone=\"5\""))
            .expect("Cue Tonehead");
        match expected_scale {
            Some(scale) => assert!(cue_tonehead.contains(scale), "{cue_tonehead}"),
            None => assert!(!cue_tonehead.contains("scale("), "{cue_tonehead}"),
        }
    }

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
fn engraves_supported_note_durations_and_one_or_two_dots() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("duration-forms.musicxml");
    let forms = [
        ("whole", 64, 0),
        ("half", 32, 0),
        ("quarter", 16, 0),
        ("eighth", 8, 0),
        ("16th", 4, 0),
        ("32nd", 2, 0),
        ("64th", 1, 0),
        ("quarter", 24, 1),
        ("half", 48, 1),
        ("quarter", 28, 2),
        ("half", 56, 2),
    ];
    let events = forms
        .iter()
        .map(|(note_type, ticks, dots)| {
            let dot = "<dot/>".repeat(*dots);
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
    assert_eq!(svg.matches("class=\"stem\"").count(), 10, "{svg}");
    assert_eq!(svg.matches("class=\"flag\"").count(), 4, "{svg}");
    assert_eq!(svg.matches("class=\"duration-mark\"").count(), 12, "{svg}");
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
    let double_dotted_quarter = ["duration-mark-9-0", "duration-mark-9-1"].map(|id| {
        svg.lines()
            .find(|line| line.contains(&format!("id=\"{id}\"")))
            .and_then(|line| line.split(" d=\"M").nth(1))
            .and_then(|path| path.split('a').next())
            .and_then(|point| point.split_once(' '))
            .map(|(x, y)| {
                (
                    x.parse::<i32>().expect("dot x"),
                    y.parse::<i32>().expect("dot y"),
                )
            })
            .expect("double-dotted quarter mark")
    });
    assert_eq!(double_dotted_quarter[0].1, double_dotted_quarter[1].1);
    assert_eq!(double_dotted_quarter[1].0 - double_dotted_quarter[0].0, 7);
    assert!(
        svg.contains("id=\"duration-mark-10-3\""),
        "double-dotted half marks missing: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn applies_multiple_dots_before_time_modification_for_event_spacing() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("dotted-tuplet-duration.musicxml");
    let notes = "      <note><pitch><step>C</step><octave>0</octave></pitch><duration>42</duration><voice>1</voice><type>quarter</type><dot/><dot/></note>\n      <note><pitch><step>D</step><octave>5</octave></pitch><duration>24</duration><voice>1</voice><type>quarter</type><dot/><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification></note>\n      <note><pitch><step>E</step><octave>9</octave></pitch><duration>24</duration><voice>1</voice><type>quarter</type></note>\n      <note><rest/><duration>36</duration><voice>1</voice><type>quarter</type><dot/></note>\n      <note><rest/><duration>84</duration><voice>1</voice><type>half</type><dot/><dot/></note>";
    let source = include_str!("fixtures/minimal.musicxml");
    let note_start = source.find("      <note>").expect("canonical note start");
    let note_end = source.find("      </note>").expect("canonical note end") + 13;
    let fixture = format!("{}{}{}", &source[..note_start], notes, &source[note_end..])
        .replace("<divisions>1</divisions>", "<divisions>24</divisions>");
    fs::write(&input, fixture).expect("write dotted tuplet score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "dotted tuplet score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("dotted-tuplet-duration.svg"))
        .expect("read dotted tuplet SVG");
    let tonehead_x = |octave| {
        svg.lines()
            .find(|line| {
                line.contains("class=\"tonehead\"")
                    && line.contains(&format!("data-octave=\"{octave}\""))
            })
            .and_then(|line| line.split("transform=\"translate(").nth(1))
            .and_then(|transform| transform.split_whitespace().next())
            .and_then(|x| x.parse::<f64>().ok())
            .expect("tonehead x position")
    };
    let first_x = tonehead_x(0);
    let second_x = tonehead_x(5);
    let third_x = tonehead_x(9);
    assert_eq!(
        second_x - first_x,
        70.0,
        "double-dotted quarter onset: {svg}"
    );
    assert_eq!(
        third_x - second_x,
        40.0,
        "dotted triplet quarter onset: {svg}"
    );
    assert_eq!(svg.matches("class=\"duration-mark\"").count(), 6, "{svg}");
    let duration_mark_position = |id| {
        let line = svg
            .lines()
            .find(|line| line.contains(&format!("id=\"{id}\"")))
            .expect("duration mark path");
        let coordinates = line
            .split(" d=\"M")
            .nth(1)
            .and_then(|path| path.split('a').next())
            .expect("duration mark coordinates");
        let (x, y) = coordinates.split_once(' ').expect("duration mark x/y");
        (
            x.parse::<i32>().expect("numeric x"),
            y.parse::<i32>().expect("numeric y"),
        )
    };
    assert_ne!(
        duration_mark_position("duration-mark-0-0"),
        duration_mark_position("duration-mark-0-1"),
        "double dots must have distinct positions: {svg}"
    );
    assert!(
        svg.contains("id=\"duration-mark-3-0\""),
        "dotted-quarter Rest mark missing: {svg}"
    );
    assert!(
        svg.contains("id=\"duration-mark-4-1\""),
        "double-dotted-half Rest marks missing: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn display_only_tuplet_group_uses_encoded_durations() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("display-only-tuplet.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let notes = "      <note><pitch><step>C</step><octave>0</octave></pitch><duration>12</duration><voice>1</voice><type>quarter</type><notations><tuplet type=\"start\"/></notations></note>\n      <note><pitch><step>D</step><octave>5</octave></pitch><duration>12</duration><voice>1</voice><type>quarter</type><notations><tuplet type=\"stop\"/></notations></note>\n      <note><pitch><step>E</step><octave>9</octave></pitch><duration>16</duration><voice>1</voice><type>quarter</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(original_note, notes)
        .replace("<divisions>1</divisions>", "<divisions>16</divisions>");
    fs::write(&input, fixture).expect("write display-only tuplet score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "display-only tuplet score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("display-only-tuplet.svg"))
        .expect("read display-only tuplet SVG");
    let tonehead_x = |octave| {
        svg.lines()
            .find(|line| {
                line.contains("class=\"tonehead\"")
                    && line.contains(&format!("data-octave=\"{octave}\""))
            })
            .and_then(|line| line.split("transform=\"translate(").nth(1))
            .and_then(|transform| transform.split_whitespace().next())
            .and_then(|x| x.parse::<f64>().ok())
            .expect("tonehead x position")
    };
    let first_x = tonehead_x(0);
    let second_x = tonehead_x(5);
    let third_x = tonehead_x(9);
    assert_eq!(second_x - first_x, 30.0, "encoded tuplet duration: {svg}");
    assert_eq!(third_x - second_x, 30.0, "encoded tuplet duration: {svg}");
    assert!(
        !svg.contains("class=\"tuplet\""),
        "tuplet graphics are out of scope: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_dotted_tuplets_example() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("dotted-tuplets.musicxml");
    fs::write(
        &input,
        include_str!("../examples/dotted-tuplets/dotted-tuplets.musicxml"),
    )
    .expect("write dotted-tuplets example");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "dotted-tuplets example failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg =
        fs::read_to_string(directory.join("dotted-tuplets.svg")).expect("read dotted-tuplets SVG");
    assert_eq!(svg.matches("class=\"tonehead\"").count(), 5, "{svg}");
    assert_eq!(svg.matches("class=\"duration-mark\"").count(), 2, "{svg}");
    let number = svg
        .lines()
        .find(|line| line.contains("class=\"tuplet-number\""))
        .expect("triplet number path");
    assert!(number.contains("data-number=\"3\""), "{number}");
    assert!(
        number.contains("scale(0.015)"),
        "tuplet number should be small: {number}"
    );
    let number_x = number
        .split("data-center-x=\"")
        .nth(1)
        .and_then(|center| center.split('"').next())
        .and_then(|x| x.parse::<f64>().ok())
        .expect("tuplet number x position");
    let stem_x = |id: &str| {
        svg.lines()
            .find(|line| line.contains(&format!("id=\"{id}\"")))
            .and_then(|line| line.split(" d=\"M").nth(1))
            .and_then(|path| path.split_whitespace().next())
            .and_then(|x| x.parse::<f64>().ok())
            .expect("tuplet stem x position")
    };
    assert_eq!(number_x, (stem_x("stem-2") + stem_x("stem-4")) / 2.0);
    let bracket = svg
        .lines()
        .find(|line| line.contains("class=\"tuplet-bracket\""))
        .expect("tuplet bracket path");
    assert!(bracket.contains("fill=\"none\""), "{bracket}");
    let bracket_path = bracket
        .split(" d=\"")
        .nth(1)
        .and_then(|path| path.split('"').next())
        .expect("tuplet bracket geometry");
    let bracket_y = bracket_path
        .split_once('V')
        .and_then(|(_, remainder)| remainder.split_once('H'))
        .and_then(|(y, _)| y.parse::<f64>().ok())
        .expect("tuplet bracket horizontal position");
    let number_y = number
        .split("data-center-y=\"")
        .nth(1)
        .and_then(|center| center.split('"').next())
        .and_then(|y| y.parse::<f64>().ok())
        .expect("tuplet number vertical center");
    assert_eq!(number_y, bracket_y);
    let stem_tip_y = |id: &str| {
        svg.lines()
            .find(|line| line.contains(&format!("id=\"{id}\"")))
            .and_then(|line| line.split(" d=\"").nth(1))
            .and_then(|path| path.split('V').next_back())
            .and_then(|y| y.split('"').next())
            .and_then(|y| y.parse::<f64>().ok())
            .expect("tuplet stem tip y position")
    };
    let bracket_corner_y = |start: bool| {
        let coordinate = if start {
            bracket_path.split_once('V').map(|(start, _)| start)
        } else {
            bracket_path.rsplit_once('V').map(|(_, end)| end)
        }
        .expect("tuplet bracket corner");
        coordinate
            .split_whitespace()
            .last()
            .and_then(|y| y.parse::<f64>().ok())
            .expect("tuplet bracket corner y position")
    };
    assert!((bracket_corner_y(true) - stem_tip_y("stem-2")).abs() >= 2.0);
    assert!((bracket_corner_y(false) - stem_tip_y("stem-4")).abs() >= 2.0);
    assert_eq!((bracket_y - bracket_corner_y(true)).abs(), 7.5);
    assert_eq!((bracket_y - bracket_corner_y(false)).abs(), 7.5);

    let no_bracket_input = directory.join("dotted-tuplets-no-bracket.musicxml");
    let no_bracket_source = include_str!("../examples/dotted-tuplets/dotted-tuplets.musicxml")
        .replace(
            "<tuplet type=\"start\" number=\"1\">",
            "<tuplet type=\"start\" number=\"1\" bracket=\"no\">",
        );
    fs::write(&no_bracket_input, no_bracket_source).expect("write unbracketed tuplet score");
    let no_bracket_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&no_bracket_input)
        .output()
        .expect("run CLI without tuplet bracket");
    assert!(
        no_bracket_output.status.success(),
        "unbracketed tuplet score failed: {}",
        String::from_utf8_lossy(&no_bracket_output.stderr)
    );
    let no_bracket_svg = fs::read_to_string(directory.join("dotted-tuplets-no-bracket.svg"))
        .expect("read unbracketed tuplet SVG");
    assert_eq!(no_bracket_svg.matches("class=\"tuplet-number\"").count(), 1);
    assert!(!no_bracket_svg.contains("class=\"tuplet-bracket\""));

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_malformed_tuplet_ratios_and_spans_contextually() {
    let make_note = |duration: u32, suffix: &str| {
        format!(
            "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>{duration}</duration><voice>1</voice><type>quarter</type>{suffix}</note>\n"
        )
    };
    let note_start = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let nested_notes = format!(
        "{}{}",
        make_note(24, "<notations><tuplet type=\"start\"/></notations>"),
        make_note(24, "<notations><tuplet type=\"start\"/></notations>")
    );
    let number_mismatch_notes = format!(
        "{}{}",
        make_note(
            24,
            "<notations><tuplet type=\"start\" number=\"1\"/></notations>"
        ),
        make_note(
            24,
            "<notations><tuplet type=\"stop\" number=\"2\"/></notations>"
        )
    );
    let cases = [
        (
            "zero-actual",
            make_note(
                24,
                "<time-modification><actual-notes>0</actual-notes><normal-notes>2</normal-notes></time-modification>",
            ),
            "<actual-notes> must be a positive integer",
        ),
        (
            "zero-normal",
            make_note(
                24,
                "<time-modification><actual-notes>3</actual-notes><normal-notes>0</normal-notes></time-modification>",
            ),
            "<normal-notes> must be a positive integer",
        ),
        (
            "missing-normal",
            make_note(
                24,
                "<time-modification><actual-notes>3</actual-notes></time-modification>",
            ),
            "requires <actual-notes> followed by <normal-notes>",
        ),
        (
            "ratio-duration-mismatch",
            make_note(
                24,
                "<time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification>",
            ),
            "duration 24 does not match <type>",
        ),
        (
            "dotted-duration-mismatch",
            make_note(24, "<dot/><dot/>"),
            "duration 24 does not match <type>",
        ),
        (
            "too-many-dots",
            make_note(24, "<dot/><dot/><dot/>"),
            "at most two augmentation dots are supported",
        ),
        (
            "unmatched-stop",
            make_note(24, "<notations><tuplet type=\"stop\"/></notations>"),
            "tuplet stop has no matching start",
        ),
        (
            "unclosed-group",
            make_note(24, "<notations><tuplet type=\"start\"/></notations>"),
            "tuplet group does not end before the measure boundary",
        ),
        (
            "nested-group",
            nested_notes,
            "nested tuplet groups are unsupported",
        ),
        (
            "number-mismatch",
            number_mismatch_notes,
            "does not match active tuplet",
        ),
    ];
    for (name, notes, expected_error) in cases {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join(format!("{name}.musicxml"));
        let fixture = include_str!("fixtures/minimal.musicxml")
            .replace(note_start, &notes)
            .replace("<divisions>1</divisions>", "<divisions>24</divisions>");
        fs::write(&input, fixture).expect("write invalid tuplet score");

        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "{name} was accepted: {diagnostic}"
        );
        assert!(diagnostic.contains(expected_error), "{name}: {diagnostic}");
        assert!(!directory.join(format!("{name}.svg")).exists());

        fs::remove_dir_all(directory).expect("remove test directory");
    }
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
fn chord_stem_direction_follows_the_midpoint_of_the_center_line() {
    for (octaves, should_extend_up, attachment_octave) in [
        (&[5, 6, 4][..], false, 4),
        (&[4, 3, 5][..], true, 5),
        (&[5, 4][..], true, 5),
        (&[5, 5, 4][..], true, 5),
    ] {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join("chord-stem-direction.musicxml");
        let steps = ["C", "D", "E"];
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
fn aligns_chord_stem_with_horizontal_midpoint_of_toneheads() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("chord-stem-midpoint.musicxml");
    let notes = ["C", "D", "E"]
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let chord_marker = if index == 0 { "" } else { "<chord/>" };
            format!(
                "<note>{chord_marker}<pitch><step>{step}</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>"
            )
        })
        .collect::<String>();
    let first_note = "<note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let fixture = include_str!("fixtures/minimal.musicxml").replace(first_note, &notes);
    fs::write(&input, fixture).expect("write chord score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "chord render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("chord-stem-midpoint.svg")).expect("read SVG");
    let tonehead_xs = svg
        .lines()
        .filter(|line| line.contains("class=\"tonehead\""))
        .filter_map(|line| {
            line.split("transform=\"translate(")
                .nth(1)?
                .split_whitespace()
                .next()?
                .parse::<i32>()
                .ok()
        })
        .collect::<Vec<_>>();
    assert_eq!(tonehead_xs.len(), 3, "{svg}");
    let leftmost_tonehead_x = *tonehead_xs.iter().min().expect("leftmost tonehead");
    let rightmost_tonehead_x = *tonehead_xs.iter().max().expect("rightmost tonehead");
    let expected_stem_x = (leftmost_tonehead_x + rightmost_tonehead_x) / 2 + 6;
    let stem = svg
        .lines()
        .find(|line| line.contains("id=\"stem\""))
        .expect("chord stem");
    let stem_x = stem
        .split(" d=\"M")
        .nth(1)
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<i32>().ok())
        .expect("stem x position");
    assert_eq!(stem_x, expected_stem_x, "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
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
fn renders_repeat_barlines_and_numbered_endings() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("repeat-endings.musicxml");
    fs::write(
        &input,
        include_str!("../examples/repeat-endings/repeat-endings.musicxml"),
    )
    .expect("write repeat-endings example");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "repeat-endings score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("repeat-endings.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"repeat-barline\"").count(), 4, "{svg}");
    assert_eq!(svg.matches("class=\"repeat-dot\"").count(), 4, "{svg}");
    assert_eq!(svg.matches("class=\"ending-bracket\"").count(), 2, "{svg}");
    for bracket in svg
        .lines()
        .filter(|line| line.contains("class=\"ending-bracket\""))
    {
        assert!(bracket.contains("fill=\"none\""), "{bracket}");
    }
    assert!(svg.contains("d=\"M0 27V0H84V27\""), "{svg}");
    assert!(svg.contains("d=\"M0 27V0H84\""), "{svg}");
    assert!(svg.contains("transform=\"translate(208 -4)\""), "{svg}");
    assert!(svg.contains("transform=\"translate(296 -4)\""), "{svg}");
    assert!(svg.contains("translate(210 19)"), "{svg}");
    assert!(svg.contains("translate(298 19)"), "{svg}");
    assert_eq!(svg.matches("stroke-width=\"3\"").count(), 2, "{svg}");
    assert!(
        svg.contains("id=\"meter-numerator\" transform=\"translate(58 30) scale(2)\""),
        "{svg}"
    );
    assert!(svg.contains("id=\"repeat-barline-0-104\""), "{svg}");
    assert!(svg.contains("transform=\"translate(108 62)\""), "{svg}");
    assert!(svg.contains("transform=\"translate(108 74)\""), "{svg}");
    assert!(svg.contains("id=\"repeat-barline-1-200\""), "{svg}");
    assert!(svg.contains("transform=\"translate(192 62)\""), "{svg}");
    assert!(svg.contains("transform=\"translate(192 74)\""), "{svg}");
    assert!(
        svg.find("class=\"repeat-barline\"").unwrap() < svg.find("class=\"ending-label\"").unwrap()
            && svg.find("class=\"ending-label\"").unwrap()
                < svg.find("class=\"ending-bracket\"").unwrap(),
        "repeat and ending marks are out of score order: {svg}"
    );
    for label in ["1", "2"] {
        assert!(
            svg.contains(&format!("data-ending-label=\"{label}\"")),
            "missing ending label {label}: {svg}"
        );
    }
    assert!(
        !svg.contains("<text"),
        "ending labels must be glyph paths: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_malformed_ending_spans_with_score_context() {
    let source = include_str!("../examples/repeat-endings/repeat-endings.musicxml");
    let cases = [
        (
            source.replace(
                "<ending number=\"2\" type=\"discontinue\"/>",
                "<ending number=\"3\" type=\"discontinue\"/>",
            ),
            "does not match active ending",
        ),
        (
            source.replace("<ending number=\"2\" type=\"start\"/>", ""),
            "has no matching start",
        ),
        (
            source.replace("<ending number=\"1\" type=\"stop\"/>", ""),
            "ending starts overlap",
        ),
        (
            source.replace(
                "<ending number=\"1\" type=\"start\"/>",
                "<ending number=\"1\" type=\"start\"/><ending number=\"3\" type=\"start\"/>",
            ),
            "multiple ending starts",
        ),
    ];

    for (index, (fixture, expected_diagnostic)) in cases.into_iter().enumerate() {
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join(format!("malformed-ending-{index}.musicxml"));
        fs::write(&input, fixture).expect("write malformed ending score");
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(!output.status.success(), "malformed ending was accepted");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostic.contains(expected_diagnostic), "{diagnostic}");
        assert!(diagnostic.contains("XML path"), "{diagnostic}");
        assert!(
            !directory
                .join(format!("malformed-ending-{index}.svg"))
                .exists(),
            "malformed ending emitted partial SVG"
        );
        fs::remove_dir_all(directory).expect("remove test directory");
    }
}

#[test]
fn applies_part_edge_repeat_and_dozenal_ending_label_fallbacks() {
    let fixture = include_str!("../examples/repeat-endings/repeat-endings.musicxml")
        .replace("<repeat direction=\"forward\"/>", "")
        .replace("ending number=\"1\"", "ending number=\"10\"");
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("implicit-repeat.musicxml");
    fs::write(&input, fixture).expect("write implicit-repeat score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "implicit-repeat score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("implicit-repeat.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"repeat-barline\"").count(), 4, "{svg}");
    assert_eq!(svg.matches("class=\"repeat-dot\"").count(), 4, "{svg}");
    assert!(svg.contains("data-ending-label=\"↊\""), "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn pairs_custom_ending_text_by_number() {
    let fixture = include_str!("../examples/repeat-endings/repeat-endings.musicxml").replace(
        "<ending number=\"1\" type=\"start\"/>",
        "<ending number=\"1\" type=\"start\" text=\"First &amp; Second\"/>",
    );
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("custom-ending-text.musicxml");
    fs::write(&input, fixture).expect("write custom-label score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "custom-label score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("custom-ending-text.svg")).expect("read SVG");
    assert!(
        svg.contains("data-ending-label=\"First &amp; Second\""),
        "{svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn positions_middle_repeat_at_the_following_event_anchor() {
    let fixture = include_str!("../examples/repeat-endings/repeat-endings.musicxml")
        .replace(
            "<barline location=\"right\"><bar-style>light-heavy</bar-style><repeat direction=\"backward\"/></barline>",
            "<barline location=\"middle\"><bar-style>light-heavy</bar-style><repeat direction=\"backward\"/></barline>",
        )
        .replace(
            "</measure>\n    <measure number=\"3\">",
            "<note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>\n    </measure>\n    <measure number=\"3\">",
        );
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("middle-repeat.musicxml");
    fs::write(&input, fixture).expect("write middle-repeat score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "middle-repeat score failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("middle-repeat.svg")).expect("read SVG");
    assert!(svg.contains("id=\"repeat-barline-1-204\""), "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_every_supported_repeat_bar_style() {
    let source = include_str!("../examples/repeat-endings/repeat-endings.musicxml");
    let cases = [
        ("regular", "class=\"repeat-barline\""),
        ("dotted", "stroke-dasharray=\"1 3\""),
        ("dashed", "stroke-dasharray=\"5 3\""),
        ("heavy", "stroke-width=\"3\""),
        ("light-light", "class=\"repeat-barline\""),
        ("light-heavy", "class=\"repeat-barline\""),
        ("heavy-light", "class=\"repeat-barline\""),
        ("heavy-heavy", "class=\"repeat-barline\""),
        ("tick", "M0 27.5V45.5"),
        ("short", "M0 40V96"),
        ("none", "class=\"repeat-barline\""),
    ];

    for (index, (style, expected_svg)) in cases.into_iter().enumerate() {
        let fixture = source.replace(
            "<bar-style>heavy-light</bar-style>",
            &format!("<bar-style>{style}</bar-style>"),
        );
        let directory = scratch_dir();
        fs::create_dir_all(&directory).expect("create test directory");
        let input = directory.join(format!("repeat-style-{index}.musicxml"));
        fs::write(&input, fixture).expect("write repeat-style score");
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "{style} repeat style failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let svg = fs::read_to_string(directory.join(format!("repeat-style-{index}.svg")))
            .expect("read SVG");
        if style == "none" {
            assert!(!svg.contains("id=\"repeat-barline-0-"), "{svg}");
        } else {
            assert!(svg.contains(expected_svg), "{style}: {svg}");
        }
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
fn rejects_music_in_the_first_measure_of_a_multiple_rest_span() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("multiple-rest-with-music.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<attributes><divisions>1</divisions></attributes>",
        "<attributes><divisions>1</divisions><measure-style><multiple-rest>1</multiple-rest></measure-style></attributes>",
    );
    fs::write(&input, fixture).expect("write conflicting multiple-rest score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success(),
        "music inside a multi-measure Rest must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("music is not allowed inside a multiple-rest span"),
        "{stderr}"
    );
    assert!(!directory.join("multiple-rest-with-music.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn substitutes_matching_meter_symbol_for_multiple_rest() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("multiple-rest-symbol.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(original_note, "")
        .replace(
            "<attributes><divisions>1</divisions></attributes>",
            "<attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><measure-style><multiple-rest use-symbols=\"yes\">1</multiple-rest></measure-style></attributes>",
        );
    fs::write(&input, fixture).expect("write symbol multiple-rest score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "matching multiple-rest symbol should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    let svg = fs::read_to_string(directory.join("multiple-rest-symbol.svg")).expect("read SVG");
    assert!(svg.contains("class=\"multiple-rest\""), "{svg}");
    assert!(svg.contains("data-duration=\"whole\""), "{svg}");
    assert!(
        !svg.contains("multiple-rest-count-"),
        "symbol form has no count label: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_multiple_rest_as_one_two_measure_span() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("multiple-rest-span.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(original_note, "")
        .replace(
            "<attributes><divisions>1</divisions></attributes>",
            "<attributes><divisions>1</divisions><measure-style><multiple-rest>2</multiple-rest></measure-style></attributes>",
        )
        .replace(
            "  </part>",
            "    <measure number=\"2\"></measure>\n  </part>",
        );
    fs::write(&input, fixture).expect("write two-measure multiple-rest score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "multiple-rest span should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("multiple-rest-span.svg")).expect("read SVG");
    assert!(
        svg.contains("class=\"multiple-rest\" data-count=\"2\""),
        "{svg}"
    );
    assert!(
        !svg.contains("id=\"barline-1\""),
        "internal barline must be suppressed: {svg}"
    );
    assert!(
        svg.contains("id=\"barline-2\""),
        "span-end barline must remain: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_counted_multiple_rest() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("multiple-rest.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(original_note, "")
        .replace(
            "<attributes><divisions>1</divisions></attributes>",
            "<attributes><divisions>1</divisions><measure-style><multiple-rest>1</multiple-rest></measure-style></attributes>",
        );
    fs::write(&input, fixture).expect("write multiple-rest score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "multiple Rest should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("multiple-rest.svg")).expect("read SVG");
    assert!(svg.contains("class=\"multiple-rest\""), "{svg}");
    assert!(svg.contains("data-count=\"1\""), "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_single_measure_rest_centered_at_double_width() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("single-measure-rest.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let measure_rest = "<note><rest measure=\"yes\"/><duration>2</duration><voice>1</voice><type>whole</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(original_note, measure_rest)
        .replace(
            "<attributes><divisions>1</divisions></attributes>",
            "<attributes><divisions>1</divisions><time><beats>6</beats><beat-type>8</beat-type></time></attributes>",
        );
    fs::write(&input, fixture).expect("write single-measure Rest score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "single-measure Rest should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("single-measure Rest encoded duration"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("measure duration is underfull"),
        "{stderr}"
    );
    let svg = fs::read_to_string(directory.join("single-measure-rest.svg")).expect("read SVG");
    let rest = svg
        .lines()
        .find(|line| line.contains("class=\"rest\""))
        .expect("single-measure Rest glyph");
    assert!(
        rest.contains("scale(2 1)"),
        "Rest should be twice as wide: {rest}"
    );
    assert!(
        svg.contains("id=\"barline-1\""),
        "explicit barline must remain: {svg}"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_breve_long_and_maxima_rests() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("extended-rests.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let rests = "<note><rest/><duration>8</duration><voice>1</voice><type>breve</type></note>\n      <note><rest/><duration>16</duration><voice>1</voice><type>long</type></note>\n      <note><rest/><duration>32</duration><voice>1</voice><type>maxima</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml").replace(original_note, rests);
    fs::write(&input, fixture).expect("write extended-rest score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "extended Rests should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("extended-rests.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"rest\"").count(), 3, "{svg}");

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
fn renders_measure_repeat_slashes_between_start_and_stop() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("measure-repeat.musicxml");
    let repeated_note = "<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "  </part>",
        &format!(
            "    <measure number=\"2\"><attributes><measure-style><measure-repeat type=\"start\" slashes=\"2\">1</measure-repeat></measure-style></attributes>{repeated_note}</measure>\n    <measure number=\"3\"><attributes><measure-style><measure-repeat type=\"stop\"/></measure-style></attributes>{repeated_note}</measure>\n  </part>"
        ),
    );
    fs::write(&input, fixture).expect("write measure-repeat score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "measure repeat should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("measure-repeat.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"measure-repeat\"").count(), 2, "{svg}");
    assert_eq!(
        svg.matches("class=\"measure-repeat-dot\"").count(),
        2,
        "each measure-repeat symbol has exactly two dots: {svg}"
    );
    let slash_lines = svg
        .lines()
        .filter(|line| line.contains("class=\"measure-repeat\""))
        .collect::<Vec<_>>();
    assert_eq!(slash_lines.len(), 2, "{svg}");
    let upper_dot_line = svg
        .lines()
        .find(|line| line.contains("id=\"measure-repeat-dot-1-0\""))
        .expect("measure-repeat symbol has an upper-left dot");
    let lower_dot_line = svg
        .lines()
        .find(|line| line.contains("id=\"measure-repeat-dot-1-1\""))
        .expect("measure-repeat symbol has a lower-right dot");
    let first_slash_position = path_translation(slash_lines[0]);
    let last_slash_position = path_translation(slash_lines[1]);
    assert_eq!(
        last_slash_position.0,
        first_slash_position.0 + 6,
        "measure-repeat slash spacing matches beat-repeat"
    );
    assert_eq!(
        path_translation(upper_dot_line),
        (first_slash_position.0, first_slash_position.1 - 8),
        "upper-left dot top and left edges align with the first slash"
    );
    assert_eq!(
        path_translation(lower_dot_line),
        (last_slash_position.0 + 4, last_slash_position.1 + 4),
        "lower-right dot bottom and right edges align with the last slash"
    );
    assert!(upper_dot_line.contains("d=\"M2 0A2 2 0 0 1 2 4A2 2 0 0 1 2 0Z\""));
    assert!(lower_dot_line.contains("d=\"M2 0A2 2 0 0 1 2 4A2 2 0 0 1 2 0Z\""));
    assert_eq!(svg.matches("class=\"tonehead\"").count(), 2, "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn measure_repeat_does_not_hide_events_in_other_parts() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("measure-repeat-parts.musicxml");
    let fixture = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
    <part-list>
        <score-part id="P1"><part-name>Repeat</part-name></score-part>
        <score-part id="P2"><part-name>Unaffected</part-name></score-part>
    </part-list>
    <part id="P1">
        <measure number="1"><attributes><divisions>1</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note></measure>
        <measure number="2"><attributes><measure-style><measure-repeat type="start">1</measure-repeat></measure-style></attributes><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note></measure>
        <measure number="3"><attributes><measure-style><measure-repeat type="stop"/></measure-style></attributes><note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note></measure>
    </part>
    <part id="P2">
        <measure number="1"><attributes><divisions>1</divisions></attributes><note><pitch><step>F</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note></measure>
        <measure number="2"><note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note></measure>
        <measure number="3"><note><pitch><step>A</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note></measure>
    </part>
</score-partwise>
"#;
    fs::write(&input, fixture).expect("write multi-Part repeat score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "multi-Part repeat should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("measure-repeat-parts.svg"))
        .expect("read multi-Part SVG");
    assert_eq!(svg.matches("class=\"measure-repeat\"").count(), 1, "{svg}");
    assert_eq!(svg.matches("class=\"tonehead\"").count(), 5, "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_incomplete_measure_repeat_cycles() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("incomplete-measure-repeat.musicxml");
    let note = "<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "  </part>",
        &format!(
            "    <measure number=\"2\">{note}</measure>\n    <measure number=\"3\"><attributes><measure-style><measure-repeat type=\"start\">2</measure-repeat></measure-style></attributes>{note}</measure>\n    <measure number=\"4\"><attributes><measure-style><measure-repeat type=\"stop\"/></measure-style></attributes>{note}</measure>\n  </part>"
        ),
    );
    fs::write(&input, fixture).expect("write incomplete measure-repeat score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success(),
        "incomplete repeat cycle must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("complete source-pattern cycles"),
        "{stderr}"
    );
    assert!(!directory.join("incomplete-measure-repeat.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_measure_repeat_without_preceding_source() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("measure-repeat-without-source.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<attributes><divisions>1</divisions></attributes>",
        "<attributes><divisions>1</divisions><measure-style><measure-repeat type=\"start\">1</measure-repeat></measure-style></attributes>",
    );
    fs::write(&input, fixture).expect("write measure-repeat without source");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success(), "missing source must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("source pattern does not exist"), "{stderr}");
    assert!(!directory.join("measure-repeat-without-source.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn measure_repeat_start_can_continue_to_part_end() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("measure-repeat-to-end.musicxml");
    let note = "<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "  </part>",
        &format!(
            "    <measure number=\"2\"><attributes><measure-style><measure-repeat type=\"start\">1</measure-repeat></measure-style></attributes>{note}</measure>\n  </part>"
        ),
    );
    fs::write(&input, fixture).expect("write open-ended measure-repeat score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "open-ended repeat should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("measure-repeat-to-end.svg"))
        .expect("read open-ended repeat SVG");
    assert_eq!(svg.matches("class=\"measure-repeat\"").count(), 1, "{svg}");
    assert_eq!(
        svg.matches("class=\"measure-repeat-dot\"").count(),
        2,
        "one-slash symbol has exactly two dots: {svg}"
    );
    let slash_line = svg
        .lines()
        .find(|line| line.contains("class=\"measure-repeat\""))
        .expect("measure-repeat slash exists");
    let upper_dot_line = svg
        .lines()
        .find(|line| line.contains("id=\"measure-repeat-dot-1-0\""))
        .expect("measure-repeat symbol has an upper-left dot");
    let lower_dot_line = svg
        .lines()
        .find(|line| line.contains("id=\"measure-repeat-dot-1-1\""))
        .expect("measure-repeat symbol has a lower-right dot");
    let slash_position = path_translation(slash_line);
    assert_eq!(
        path_translation(upper_dot_line),
        (slash_position.0, slash_position.1 - 8),
        "single-slash upper-left dot aligns with the slash bounds"
    );
    assert_eq!(
        path_translation(lower_dot_line),
        (slash_position.0 + 4, slash_position.1 + 4),
        "single-slash lower-right dot aligns with the slash bounds"
    );
    assert!(upper_dot_line.contains("d=\"M2 0A2 2 0 0 1 2 4A2 2 0 0 1 2 0Z\""));
    assert!(lower_dot_line.contains("d=\"M2 0A2 2 0 0 1 2 4A2 2 0 0 1 2 0Z\""));

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn routes_measure_repeat_to_its_input_staff_in_separate_mode() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("staff-scoped-measure-repeat.musicxml");
    let source_note = |step: &str, staff: u32| {
        format!(
            "<note><pitch><step>{step}</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>{staff}</staff></note>"
        )
    };
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace("<divisions>1</divisions>", "<divisions>1</divisions><staves>2</staves>")
        .replace(
            "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>",
            &format!("{}{}", source_note("C", 1), source_note("E", 2)),
        )
        .replace(
            "  </part>",
            &format!(
                "    <measure number=\"2\"><attributes><measure-style number=\"2\"><measure-repeat type=\"start\">1</measure-repeat></measure-style></attributes>{}{}</measure>\n    <measure number=\"3\"><attributes><measure-style number=\"2\"><measure-repeat type=\"stop\"/></measure-style></attributes>{}{}</measure>\n  </part>",
                source_note("D", 1), source_note("F", 2), source_note("G", 1), source_note("A", 2)
            ),
        );
    fs::write(&input, fixture).expect("write staff-scoped measure-repeat score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .arg("--input-staff-mode")
        .arg("separate")
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "staff-scoped measure repeat should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let staff_one = fs::read_to_string(directory.join("staff-scoped-measure-repeat-staff-1.svg"))
        .expect("read Staff 1 SVG");
    let staff_two = fs::read_to_string(directory.join("staff-scoped-measure-repeat-staff-2.svg"))
        .expect("read Staff 2 SVG");
    assert!(
        !staff_one.contains("class=\"measure-repeat\""),
        "{staff_one}"
    );
    assert!(
        staff_two.contains("class=\"measure-repeat\""),
        "{staff_two}"
    );

    let combined_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run combined-staff CLI");
    assert!(
        !combined_output.status.success(),
        "conflicting Input Staff content must fail in combined mode"
    );
    let stderr = String::from_utf8_lossy(&combined_output.stderr);
    assert!(
        stderr.contains("conflicts with measure-repeat notation"),
        "{stderr}"
    );
    assert!(!directory.join("staff-scoped-measure-repeat.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn input_staff_target_ends_before_measure_repeat_stop_measure() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("measure-repeat-stop-staff.musicxml");
    let fixture = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Repeat scope</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1"><attributes><divisions>1</divisions><staves>2</staves></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>2</staff></note></measure>
    <measure number="2"><attributes><measure-style number="2"><measure-repeat type="start">1</measure-repeat></measure-style></attributes><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>2</staff></note></measure>
    <measure number="3"><attributes><measure-style number="2"><measure-repeat type="stop"/></measure-style></attributes><note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note></measure>
  </part>
</score-partwise>
"#;
    fs::write(&input, fixture).expect("write stop-measure Input Staff score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "stop measure is outside repeat scope: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(directory.join("measure-repeat-stop-staff.svg"))
        .expect("read stop-measure scope SVG");
    assert_eq!(svg.matches("class=\"measure-repeat\"").count(), 1);
    assert_eq!(svg.matches("class=\"tonehead\"").count(), 2);

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_beat_repeats_using_meter_denominator_units() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("beat-repeat.musicxml");
    let note = |step: &str, duration: u32, note_type: &str| {
        format!(
            "<note><pitch><step>{step}</step><octave>4</octave></pitch><duration>{duration}</duration><voice>1</voice><type>{note_type}</type></note>"
        )
    };
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(
            "<measure number=\"1\">",
            "<measure number=\"1\" implicit=\"yes\">",
        )
        .replace(
            "<attributes><divisions>1</divisions></attributes>",
            "<attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>",
        )
        .replace("<divisions>1</divisions>", "<divisions>2</divisions>")
        .replace(
            "<duration>1</duration><voice>1</voice><type>quarter</type>",
            "<duration>2</duration><voice>1</voice><type>quarter</type>",
        )
        .replace(
            "  </part>",
            &format!(
                "    <measure number=\"2\"><attributes><time><beats>4</beats><beat-type>4</beat-type></time><measure-style><beat-repeat type=\"start\" slashes=\"2\" use-dots=\"yes\" slash-type=\"16th\">1</beat-repeat></measure-style></attributes>{}{}{}{}</measure>\n    <measure number=\"3\"><attributes><time><beats>6</beats><beat-type>8</beat-type></time></attributes>{}{}{}{}{}{}</measure>\n    <measure number=\"4\"><attributes><measure-style><beat-repeat type=\"stop\"/></measure-style></attributes>{}{}{}{}{}{}</measure>\n  </part>",
                note("C", 2, "quarter"), note("D", 2, "quarter"),
                note("E", 2, "quarter"), note("F", 2, "quarter"),
                note("G", 1, "eighth"), note("A", 1, "eighth"),
                note("B", 1, "eighth"), note("C", 1, "eighth"),
                note("D", 1, "eighth"), note("E", 1, "eighth"),
                note("F", 1, "eighth"), note("G", 1, "eighth"),
                note("A", 1, "eighth"), note("B", 1, "eighth"),
                note("C", 1, "eighth"), note("D", 1, "eighth")
            ),
        );
    fs::write(&input, fixture).expect("write beat-repeat score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "beat repeat should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    let svg = fs::read_to_string(directory.join("beat-repeat.svg")).expect("read SVG");
    assert_eq!(svg.matches("class=\"beat-repeat\"").count(), 20, "{svg}");
    assert_eq!(svg.matches("id=\"beat-repeat-dot-").count(), 20, "{svg}");
    assert_eq!(svg.matches("id=\"beat-repeat-detail-").count(), 40, "{svg}");
    assert_eq!(svg.matches("class=\"tonehead\"").count(), 7, "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn renders_self_closing_beat_repeat_markers_at_attribute_positions() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("positioned-beat-repeat.musicxml");
    let note = |step: &str| {
        format!(
            "<note><pitch><step>{step}</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>"
        )
    };
    let fixture = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<score-partwise version=\"4.0\">\n\
  <part-list><score-part id=\"P1\"><part-name>Positioned repeat</part-name></score-part></part-list>\n\
  <part id=\"P1\">\n\
    <measure number=\"1\" implicit=\"yes\"><attributes><divisions>2</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>{}\
</measure>\n\
    <measure number=\"2\">{}<attributes><measure-style><beat-repeat type=\"start\"/></measure-style></attributes>{}\
<attributes><measure-style><beat-repeat type=\"stop\"/></measure-style></attributes>{}{}\
</measure>\n\
  </part>\n\
</score-partwise>\n",
        note("C"),
        note("D"),
        note("E"),
        note("F"),
        note("G"),
    );
    fs::write(&input, fixture).expect("write positioned beat-repeat score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "self-closing positioned beat-repeat markers should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    let svg = fs::read_to_string(directory.join("positioned-beat-repeat.svg"))
        .expect("read positioned beat-repeat SVG");
    assert_eq!(svg.matches("class=\"beat-repeat\"").count(), 1, "{svg}");
    assert_eq!(svg.matches("class=\"tonehead\"").count(), 4, "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn finds_beat_repeat_source_across_denominator_change() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("beat-repeat-source-denominator-change.musicxml");
    let note = |step: &str, duration: u32, note_type: &str| {
        format!(
            "<note><pitch><step>{step}</step><octave>4</octave></pitch><duration>{duration}</duration><voice>1</voice><type>{note_type}</type></note>"
        )
    };
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(
            "<attributes><divisions>1</divisions></attributes>",
            "<attributes><divisions>2</divisions><time><beats>8</beats><beat-type>8</beat-type></time></attributes>",
        )
        .replace(
            "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>",
            &[
                note("C", 1, "eighth"),
                note("D", 1, "eighth"),
                note("E", 1, "eighth"),
                note("F", 1, "eighth"),
                note("G", 1, "eighth"),
                note("A", 1, "eighth"),
                note("B", 1, "eighth"),
                note("C", 1, "eighth"),
            ]
            .join(""),
        )
        .replace(
            "  </part>",
            &format!(
                "    <measure number=\"2\"><attributes><time><beats>4</beats><beat-type>4</beat-type></time><measure-style><beat-repeat type=\"start\">6</beat-repeat></measure-style></attributes>{}{}{}{}</measure>\n    <measure number=\"3\"><attributes><time><beats>2</beats><beat-type>4</beat-type></time></attributes>{}{}</measure>\n    <measure number=\"4\"><attributes><measure-style><beat-repeat type=\"stop\"/></measure-style></attributes>{}{}</measure>\n  </part>",
                note("D", 2, "quarter"),
                note("E", 2, "quarter"),
                note("F", 2, "quarter"),
                note("G", 2, "quarter"),
                note("A", 2, "quarter"),
                note("B", 2, "quarter"),
                note("C", 2, "quarter"),
                note("D", 2, "quarter")
            ),
        );
    fs::write(&input, fixture).expect("write denominator-change source score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "valid prior source should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    let svg = fs::read_to_string(directory.join("beat-repeat-source-denominator-change.svg"))
        .expect("read denominator-change source SVG");
    assert_eq!(svg.matches("class=\"beat-repeat\"").count(), 1, "{svg}");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_beat_repeat_without_preceding_source() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("beat-repeat-without-source.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(
            "<attributes><divisions>1</divisions></attributes>",
            "<attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><measure-style><beat-repeat type=\"start\">1</beat-repeat></measure-style></attributes>",
        )
        .replace(
            "  </part>",
            "    <measure number=\"2\"><attributes><measure-style><beat-repeat type=\"stop\"/></measure-style></attributes></measure>\n  </part>",
        );
    fs::write(&input, fixture).expect("write beat-repeat without source");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success(), "missing source must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("beat-repeat source pattern does not exist"),
        "{stderr}"
    );
    assert!(!directory.join("beat-repeat-without-source.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_partial_beat_repeat_cycle() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("partial-beat-repeat.musicxml");
    let note = "<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>";
    let fixture = include_str!("fixtures/minimal.musicxml")
        .replace(
            "<attributes><divisions>1</divisions></attributes>",
            "<attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>",
        )
        .replace(
            "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>",
            &format!("{note}{note}"),
        )
        .replace(
            "  </part>",
            &format!(
                "    <measure number=\"2\"><attributes><measure-style><beat-repeat type=\"start\">2</beat-repeat></measure-style></attributes>{note}{note}{note}</measure>\n    <measure number=\"3\"><attributes><measure-style><beat-repeat type=\"stop\"/></measure-style></attributes>{note}</measure>\n  </part>"
            ),
        );
    fs::write(&input, fixture).expect("write partial beat-repeat score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success(), "partial cycle must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("complete source-pattern cycles"),
        "{stderr}"
    );
    assert!(!directory.join("partial-beat-repeat.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_beat_repeat_stop_without_start() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("beat-repeat-without-start.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "  </part>",
        "    <measure number=\"2\"><attributes><measure-style><beat-repeat type=\"stop\"/></measure-style></attributes><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note></measure>\n  </part>",
    );
    fs::write(&input, fixture).expect("write beat-repeat without start");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        !output.status.success(),
        "unmatched beat-repeat stop must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("stop has no active start"), "{stderr}");
    assert!(!directory.join("beat-repeat-without-start.svg").exists());

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_measure_repeat_stop_without_start() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("measure-repeat-without-start.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "  </part>",
        "    <measure number=\"2\"><attributes><measure-style><measure-repeat type=\"stop\"/></measure-style></attributes><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note></measure>\n  </part>",
    );
    fs::write(&input, fixture).expect("write measure-repeat without start");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(!output.status.success(), "unmatched stop must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("stop has no active start"), "{stderr}");
    assert!(!directory.join("measure-repeat-without-start.svg").exists());

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
        svg.contains("id=\"meter-numerator\" transform=\"translate(58 30) scale(2)\""),
        "time-signature numerator should fill the octave 5/6 lanes: {svg}"
    );
    assert!(
        svg.contains("id=\"meter-denominator\" transform=\"translate(58 70) scale(2)\""),
        "time-signature denominator should fill the octave 3/4 lanes: {svg}"
    );
    assert!(!svg.contains("<text"), "SVG glyphs must not rely on fonts");

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn warns_for_underfull_simple_meter_and_still_writes_svg() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("underfull-simple-meter.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<attributes><divisions>1</divisions></attributes>",
        "<attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>",
    );
    fs::write(&input, &fixture).expect("write underfull score");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "underfull score should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("underfull"), "missing diagnostic: {stderr}");
    assert!(
        directory.join("underfull-simple-meter.svg").exists(),
        "meter warning must not prevent SVG output"
    );

    let overfull_input = directory.join("overfull-simple-meter.musicxml");
    let overfull_fixture = fixture.replace(
        "</measure>",
        "<note><pitch><step>D</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note></measure>",
    );
    fs::write(&overfull_input, overfull_fixture).expect("write overfull score");
    let overfull_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&overfull_input)
        .output()
        .expect("run CLI");
    assert!(
        overfull_output.status.success(),
        "overfull score should render: {}",
        String::from_utf8_lossy(&overfull_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&overfull_output.stderr).contains("overfull"),
        "missing overfull diagnostic: {}",
        String::from_utf8_lossy(&overfull_output.stderr)
    );

    let pickup_input = directory.join("implicit-pickup.musicxml");
    let pickup_fixture = fixture.replace(
        "<measure number=\"1\">",
        "<measure number=\"0\" implicit=\"yes\">",
    );
    fs::write(&pickup_input, pickup_fixture).expect("write implicit pickup score");
    let pickup_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&pickup_input)
        .output()
        .expect("run CLI");
    assert!(
        pickup_output.status.success(),
        "implicit pickup should render: {}",
        String::from_utf8_lossy(&pickup_output.stderr)
    );
    let pickup_stderr = String::from_utf8_lossy(&pickup_output.stderr);
    assert!(
        !pickup_stderr.contains("underfull") && !pickup_stderr.contains("overfull"),
        "implicit first-measure pickup should not warn: {pickup_stderr}"
    );

    let voices_input = directory.join("multi-voice-measure-duration.musicxml");
    let original_note = "      <note>\n        <pitch><step>C</step><octave>4</octave></pitch>\n        <duration>1</duration><voice>1</voice><type>quarter</type>\n      </note>";
    let voice_events = "<note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note><backup><duration>4</duration></backup><note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type></note>";
    let voices_fixture = fixture.replace(original_note, voice_events);
    fs::write(&voices_input, voices_fixture).expect("write multiple-Voice score");
    let voices_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&voices_input)
        .output()
        .expect("run CLI");
    assert!(
        voices_output.status.success(),
        "multiple-Voice score should render: {}",
        String::from_utf8_lossy(&voices_output.stderr)
    );
    let voices_stderr = String::from_utf8_lossy(&voices_output.stderr);
    assert!(
        !voices_stderr.contains("underfull") && !voices_stderr.contains("overfull"),
        "meter duration should use the furthest Voice cursor: {voices_stderr}"
    );

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
        (2, 68, 39),
        (3, 48, 19),
        (4, 48, 19),
        (5, 68, 39),
        (6, 88, 59),
        (7, 28, -1),
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
        (3, 1, -0.75),
        (5, 1, 0.75),
        (7, 1, -0.75),
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
        let slope = (end_end_y - start_end_y) / (end_x - start_x);
        let level_offset = f64::from(level - 1) * (4.0 * slope.hypot(1.0)).round() * beam_direction;
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
    let secondary = path_points("beam-11-2");
    let beam_slope = (primary[1].1 - primary[0].1) / (primary[1].0 - primary[0].0);
    let start_edge_gap = (primary[3].1 - secondary[0].1).abs() / beam_slope.hypot(1.0);
    let end_edge_gap = (primary[2].1 - secondary[1].1).abs() / beam_slope.hypot(1.0);
    assert!(
        (0.5..2.0).contains(&start_edge_gap) && (0.5..2.0).contains(&end_edge_gap),
        "angled beam edge gaps should remain wider than the original and below the earlier wide spacing (start={start_edge_gap:.2}, end={end_edge_gap:.2})"
    );
    let hook = path_points("beam-hook-11-3");
    assert_eq!(
        hook.len(),
        4,
        "hook should also be a parallelogram: {hook:?}"
    );
    let stem_x = stem_anchors[11].0;
    assert!((hook[0].0 - (stem_x - 9.0 + 0.75)).abs() < 0.01);
    assert!((hook[1].0 - (stem_x + 0.75)).abs() < 0.01);
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
fn warns_and_ignores_valid_note_appearance_hints() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let input = directory.join("note-appearance-hints.musicxml");
    let fixture = include_str!("fixtures/minimal.musicxml").replace(
        "<duration>1</duration><voice>1</voice><type>quarter</type>",
        "<duration>1</duration><voice>1</voice><type>quarter</type><accidental cautionary=\"yes\" editorial=\"yes\" parentheses=\"yes\" bracket=\"yes\" size=\"cue\" default-x=\"1.5\" color=\"#123456\">sharp</accidental><stem>up</stem><notehead>diamond</notehead>",
    );
    fs::write(&input, fixture).expect("write score with note appearance hints");

    let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&input)
        .output()
        .expect("run CLI");
    assert!(
        output.status.success(),
        "valid note appearance hints failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let warnings = String::from_utf8_lossy(&output.stderr);
    assert!(
        warnings.contains("accidental"),
        "missing accidental warning: {warnings}"
    );
    assert!(
        warnings.contains("disagrees with the encoded pitch"),
        "missing accidental/pitch disagreement warning: {warnings}"
    );
    assert!(
        warnings.contains("stem"),
        "missing stem warning: {warnings}"
    );
    assert!(
        warnings.contains("notehead"),
        "missing notehead warning: {warnings}"
    );

    let expected_input = directory.join("unhinted.musicxml");
    fs::write(&expected_input, include_str!("fixtures/minimal.musicxml"))
        .expect("write unhinted score");
    let expected_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&expected_input)
        .output()
        .expect("run CLI for unhinted score");
    assert!(
        expected_output.status.success(),
        "unhinted score failed: {}",
        String::from_utf8_lossy(&expected_output.stderr)
    );
    let hinted_svg =
        fs::read_to_string(directory.join("note-appearance-hints.svg")).expect("read hinted SVG");
    let expected_svg =
        fs::read_to_string(directory.join("unhinted.svg")).expect("read unhinted SVG");
    assert_eq!(
        hinted_svg, expected_svg,
        "appearance hints must not alter engraving"
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn rejects_malformed_note_appearance_values_without_writing_svg() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let cases = [
        ("unsupported-stem", "<stem>double</stem>", "stem direction"),
        ("stem-none", "<stem>none</stem>", "stem direction"),
        (
            "invalid-stem",
            "<stem>sideways</stem>",
            "invalid stem direction",
        ),
        (
            "invalid-notehead",
            "<notehead>made-up</notehead>",
            "invalid notehead value",
        ),
        (
            "invalid-notehead-filled",
            "<notehead filled=\"maybe\">diamond</notehead>",
            "filled must be",
        ),
        (
            "invalid-notehead-color",
            "<notehead color=\"#GGGGGG\">diamond</notehead>",
            "notehead color",
        ),
        (
            "invalid-notehead-font-size",
            "<notehead font-size=\"giant\">diamond</notehead>",
            "invalid notehead font-size",
        ),
        (
            "invalid-accidental",
            "<accidental>unicorn</accidental>",
            "invalid accidental value",
        ),
        (
            "invalid-accidental-modifier",
            "<accidental cautionary=\"maybe\">sharp</accidental>",
            "cautionary must be",
        ),
        (
            "invalid-accidental-smufl",
            "<accidental smufl=\"noteheadBlack\">sharp</accidental>",
            "invalid accidental SMuFL glyph name",
        ),
    ];

    for (name, hint, diagnostic_fragment) in cases {
        let input = directory.join(format!("{name}.musicxml"));
        let fixture = include_str!("fixtures/minimal.musicxml").replace(
            "<duration>1</duration><voice>1</voice><type>quarter</type>",
            &format!("<duration>1</duration><voice>1</voice><type>quarter</type>{hint}"),
        );
        fs::write(&input, fixture).expect("write score with malformed note hint");

        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(!output.status.success(), "{name} was accepted");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains(diagnostic_fragment),
            "{name} missing diagnostic {diagnostic_fragment:?}: {diagnostic}"
        );
        assert!(
            diagnostic.contains("/score-partwise/part[@id='P1']/measure[@number='1']/note/"),
            "{name} missing note source context: {diagnostic}"
        );
        assert!(
            !directory.join(format!("{name}.svg")).exists(),
            "{name} wrote partial SVG"
        );
    }

    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn default_notehead_modifiers_are_silent_and_nondefault_modifiers_are_ignored() {
    let directory = scratch_dir();
    fs::create_dir_all(&directory).expect("create test directory");
    let baseline_input = directory.join("baseline.musicxml");
    fs::write(&baseline_input, include_str!("fixtures/minimal.musicxml"))
        .expect("write baseline score");
    let baseline_output = Command::new(env!("CARGO_BIN_EXE_domunor"))
        .arg(&baseline_input)
        .output()
        .expect("run CLI for baseline score");
    assert!(
        baseline_output.status.success(),
        "baseline score failed: {}",
        String::from_utf8_lossy(&baseline_output.stderr)
    );
    let baseline_svg =
        fs::read_to_string(directory.join("baseline.svg")).expect("read baseline SVG");

    let cases = [
        (
            "default-notehead",
            "<notehead filled=\"yes\" parentheses=\"no\" font-style=\"normal\" font-weight=\"normal\" font-size=\"medium\" color=\"#000000\">normal</notehead>",
            false,
        ),
        (
            "styled-notehead",
            "<notehead filled=\"no\" parentheses=\"yes\" color=\"#ff0000\">normal</notehead>",
            true,
        ),
    ];
    for (name, notehead, expect_warning) in cases {
        let input = directory.join(format!("{name}.musicxml"));
        let fixture = include_str!("fixtures/minimal.musicxml").replace(
            "<duration>1</duration><voice>1</voice><type>quarter</type>",
            &format!("<duration>1</duration><voice>1</voice><type>quarter</type>{notehead}"),
        );
        fs::write(&input, fixture).expect("write score with notehead modifiers");
        let output = Command::new(env!("CARGO_BIN_EXE_domunor"))
            .arg(&input)
            .output()
            .expect("run CLI");
        assert!(
            output.status.success(),
            "{name} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let warnings = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            warnings.contains("notehead presentation"),
            expect_warning,
            "unexpected notehead warning for {name}: {warnings}"
        );
        let svg =
            fs::read_to_string(directory.join(format!("{name}.svg"))).expect("read notehead SVG");
        assert_eq!(svg, baseline_svg, "notehead modifiers changed the Tonehead");
    }

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
        .truncate(false)
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
