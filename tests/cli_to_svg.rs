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
    assert_eq!(svg.matches("id=\"staff-line-").count(), 3);
    assert!(svg.contains("id=\"octave-4\""));
    assert!(svg.contains("id=\"octave-5\""));
    assert!(svg.contains("id=\"tonehead\""));
    assert!(svg.contains("d=\"M6 2C1 2 1 16 6 16C11 16 11 2 6 2Z\""));
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
