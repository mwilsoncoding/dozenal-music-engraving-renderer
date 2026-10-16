use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process;

mod bravura_glyphs;
mod noto_sans_glyphs;
mod text_normalization;
mod xml;
use xml::Element;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tone {
    octave: u8,
    value: u8,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct VoiceId(Option<String>);

type VoiceState<T> = Vec<(VoiceId, T)>;
type TiePath = (Tone, i32, i32);
type BeamStart = (i32, i32);
type BeamedStem = (String, i32, i32);
type BeamEventPosition = (usize, usize);
type VoiceTiePaths = VoiceState<Vec<TiePath>>;
type VoiceBeamStates = VoiceState<[Option<BeamStart>; 5]>;
type VoiceBeamedStems = VoiceState<Vec<BeamedStem>>;
type VoiceBeamEvents = VoiceState<Vec<BeamEventPosition>>;

#[derive(Clone)]
struct Event {
    tones: Vec<ToneEvent>,
    duration_units: f64,
    onset_units: f64,
    duration_name: &'static str,
    dots: u8,
    beams: Vec<BeamMark>,
    voice_id: VoiceId,
    input_staff: u32,
    is_spacer: bool,
}

#[derive(Clone)]
struct ToneEvent {
    tone: Tone,
    tie_start: bool,
    tie_stop: bool,
    input_staff: u32,
}

struct ParsedNote {
    event: Event,
    chord_marker: Option<usize>,
    tuplet_boundary: Option<TupletBoundary>,
}

enum TupletBoundary {
    Start(String),
    Stop(String),
}

struct TupletRatio {
    actual: u32,
    normal: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BeamMark {
    level: u8,
    state: BeamState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BeamState {
    Begin,
    Continue,
    End,
    ForwardHook,
    BackwardHook,
}

#[derive(Clone)]
struct Measure {
    events: Vec<Event>,
    harmonies: Vec<PositionedHarmony>,
    texts: Vec<PositionedText>,
    meter: Option<Meter>,
    input_staff_states: Vec<InputStaffState>,
    barlines: Vec<MeasureBarline>,
}

#[derive(Clone, Default)]
struct MeasureBarline {
    location: String,
    event_index: usize,
    style: Option<String>,
    repeat_start: bool,
    repeat_end: bool,
    ending_start: Option<EndingMark>,
    ending_end: Option<EndingMark>,
}

#[derive(Clone)]
struct EndingMark {
    number: String,
    label: String,
    discontinue: bool,
}

#[derive(Clone)]
struct Part {
    name: String,
    input_staff_count: u32,
    measures: Vec<Measure>,
}

struct SvgViewBox {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

struct RenderedPart {
    svg: String,
    view_box: SvgViewBox,
    barline_positions: Vec<i32>,
}

#[derive(Clone)]
struct PositionedText {
    event_index: usize,
    input_staff: Option<u32>,
    offset_x: f64,
    text: String,
    placement: TextPlacement,
    font_size: f64,
    color: String,
    enclosure: Option<String>,
    default_x: Option<f64>,
    default_y: Option<f64>,
    relative_x: Option<f64>,
    relative_y: Option<f64>,
    path_glyph: Option<bravura_glyphs::Glyph>,
    path_id_prefix: &'static str,
    path_scale: f64,
    path_y_offset: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TextPlacement {
    Above,
    Below,
}

#[derive(Clone)]
struct ChordSymbol {
    root: u8,
    bass: Option<u8>,
    tones: Vec<ChordTone>,
}

#[derive(Clone)]
struct ChordTone {
    interval: i32,
    tone: u8,
}

struct ChordDegree {
    number: u32,
    interval: i32,
}

impl ChordDegree {
    fn new(number: u32, interval: i32) -> Self {
        Self { number, interval }
    }
}

struct DegreeChange {
    number: u32,
    alteration: i32,
    operation: DegreeOperation,
}

#[derive(Clone, Copy)]
enum DegreeOperation {
    Add,
    Alter,
    Subtract,
}

impl DegreeOperation {
    fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Alter => "alter",
            Self::Subtract => "subtract",
        }
    }
}

#[derive(Clone)]
struct PositionedHarmony {
    event_index: usize,
    input_staff: Option<u32>,
    offset_x: f64,
    symbol: ChordSymbol,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Meter {
    beats: u8,
    beat_type: u8,
}

#[derive(Clone, Default)]
struct InputStaffState {
    key_signature: Option<KeySignature>,
    clef: Option<InputClef>,
    meter: Option<Meter>,
    transposition: i32,
}

#[derive(Clone)]
struct KeySignature {
    fifths: i8,
    cancel: Option<u8>,
    mode: Option<String>,
}

#[derive(Clone)]
struct InputClef {
    sign: String,
    line: Option<u8>,
    octave_change: i8,
}

struct Score {
    parts: Vec<Part>,
    warnings: Vec<String>,
}

struct CliOptions {
    input_path: PathBuf,
    output_path: Option<PathBuf>,
    input_staff_mode: InputStaffMode,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputStaffMode {
    Combine,
    Separate,
}

fn main() {
    if let Err(message) = run() {
        eprintln!("domunor: {message}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let options = parse_args()?;
    let source = xml::read_file(&options.input_path)
        .map_err(|error| format!("cannot read {}: {error}", options.input_path.display()))?;
    let score = parse_score_with_mode(&source, options.input_staff_mode)
        .map_err(|error| format!("{}: {error}", options.input_path.display()))?;
    let mut warnings = score.warnings.clone();
    let output_path = options.output_path.unwrap_or_else(|| {
        let mut path = options.input_path.clone();
        path.set_extension("svg");
        path
    });
    if output_path
        .extension()
        .is_none_or(|extension| extension != "svg")
    {
        return Err("output path must have the .svg extension".to_owned());
    }

    let rendered_outputs = match options.input_staff_mode {
        InputStaffMode::Combine => {
            let svg = render_svg(&score, &mut warnings);
            vec![(output_path, svg)]
        }
        InputStaffMode::Separate => {
            let staff_count = score
                .parts
                .iter()
                .map(|part| part.input_staff_count)
                .max()
                .unwrap_or(1);
            let mut outputs = Vec::new();
            for staff_number in 1..=staff_count {
                let path = separate_staff_output_path(&output_path, staff_number);
                let staff_score = score_for_input_staff(&score, staff_number);
                let svg = render_svg(&staff_score, &mut warnings);
                outputs.push((path, svg));
            }
            outputs
        }
    };

    for warning in &warnings {
        eprintln!(
            "domunor: warning: {}: {warning}",
            options.input_path.display()
        );
    }
    for (path, svg) in rendered_outputs {
        fs::write(&path, svg)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }
    Ok(())
}

fn parse_args() -> Result<CliOptions, String> {
    let mut args = env::args_os().skip(1);
    let mut input = None;
    let mut output = None;
    let mut input_staff_mode = None;

    while let Some(argument) = args.next() {
        if argument == "--output" {
            if output.is_some() {
                return Err("--output may only be specified once".to_owned());
            }
            output = Some(PathBuf::from(
                args.next()
                    .ok_or_else(|| "--output requires a path".to_owned())?,
            ));
        } else if argument == "--input-staff-mode" {
            if input_staff_mode.is_some() {
                return Err("--input-staff-mode may only be specified once".to_owned());
            }
            input_staff_mode = Some(
                match args
                    .next()
                    .ok_or_else(|| "--input-staff-mode requires combine or separate".to_owned())?
                    .to_string_lossy()
                    .as_ref()
                {
                    "combine" => InputStaffMode::Combine,
                    "separate" => InputStaffMode::Separate,
                    value => {
                        return Err(format!(
                            "--input-staff-mode must be 'combine' or 'separate', not {value:?}"
                        ));
                    }
                },
            );
        } else if argument.to_string_lossy().starts_with('-') {
            return Err(format!("unsupported option {}", argument.to_string_lossy()));
        } else if input.replace(PathBuf::from(argument)).is_some() {
            return Err("provide exactly one input file".to_owned());
        }
    }

    let input = input.ok_or_else(|| {
        "usage: domunor INPUT [--output FILE.svg] [--input-staff-mode combine|separate]".to_owned()
    })?;
    Ok(CliOptions {
        input_path: input,
        output_path: output,
        input_staff_mode: input_staff_mode.unwrap_or(InputStaffMode::Combine),
    })
}

fn separate_staff_output_path(output_path: &std::path::Path, staff_number: u32) -> PathBuf {
    let stem = output_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("score");
    let mut path = output_path.to_path_buf();
    path.set_file_name(format!("{stem}-staff-{staff_number}.svg"));
    path
}

fn score_for_input_staff(score: &Score, staff_number: u32) -> Score {
    let parts = score
        .parts
        .iter()
        .map(|part| {
            let measures = part
                .measures
                .iter()
                .map(|measure| {
                    let staff_state = measure
                        .input_staff_states
                        .get(staff_number as usize - 1)
                        .map(|state| InputStaffState {
                            key_signature: state.key_signature.as_ref().map(|key| KeySignature {
                                fifths: key.fifths,
                                cancel: key.cancel,
                                mode: key.mode.clone(),
                            }),
                            clef: state.clef.as_ref().map(|clef| InputClef {
                                sign: clef.sign.clone(),
                                line: clef.line,
                                octave_change: clef.octave_change,
                            }),
                            meter: state.meter,
                            transposition: state.transposition,
                        })
                        .unwrap_or_default();
                    let mut events = Vec::new();
                    let mut event_index_map = Vec::with_capacity(measure.events.len() + 1);
                    for event in &measure.events {
                        event_index_map.push(events.len());
                        let mut selected = event.clone();
                        if selected.tones.is_empty() {
                            if selected.input_staff == staff_number {
                                events.push(selected);
                            } else {
                                selected.is_spacer = true;
                                selected.beams.clear();
                                events.push(selected);
                            }
                        } else {
                            selected
                                .tones
                                .retain(|tone| tone.input_staff == staff_number);
                            if !selected.tones.is_empty() {
                                events.push(selected);
                            } else {
                                selected.is_spacer = true;
                                selected.beams.clear();
                                events.push(selected);
                            }
                        }
                    }
                    event_index_map.push(events.len());
                    let remap_index = |index: usize| {
                        event_index_map
                            .get(index)
                            .copied()
                            .unwrap_or_else(|| *event_index_map.last().unwrap())
                    };
                    let harmonies = measure
                        .harmonies
                        .iter()
                        .filter(|harmony| {
                            harmony
                                .input_staff
                                .is_none_or(|staff| staff == staff_number)
                        })
                        .cloned()
                        .map(|mut harmony| {
                            harmony.event_index = remap_index(harmony.event_index);
                            harmony
                        })
                        .collect();
                    let texts = measure
                        .texts
                        .iter()
                        .filter(|text| text.input_staff.is_none_or(|staff| staff == staff_number))
                        .cloned()
                        .map(|mut text| {
                            text.event_index = remap_index(text.event_index);
                            text
                        })
                        .collect();
                    let barlines = measure
                        .barlines
                        .iter()
                        .cloned()
                        .map(|mut barline| {
                            barline.event_index = remap_index(barline.event_index);
                            barline
                        })
                        .collect();
                    Measure {
                        events,
                        harmonies,
                        texts,
                        meter: staff_state.meter,
                        input_staff_states: vec![InputStaffState {
                            key_signature: staff_state.key_signature,
                            clef: staff_state.clef,
                            meter: staff_state.meter,
                            transposition: staff_state.transposition,
                        }],
                        barlines,
                    }
                })
                .collect();
            Part {
                name: part.name.clone(),
                input_staff_count: part.input_staff_count,
                measures,
            }
        })
        .collect();
    Score {
        parts,
        warnings: score.warnings.clone(),
    }
}

#[cfg(test)]
fn parse_score(source: &[u8]) -> Result<Score, String> {
    parse_score_with_mode(source, InputStaffMode::Combine)
}

fn parse_score_with_mode(source: &[u8], input_staff_mode: InputStaffMode) -> Result<Score, String> {
    let elements = xml::parse(source).map_err(|error| error.to_string())?;
    let root = root_element(&elements)?;
    if !elements[root].namespace.is_empty() || elements[root].name != "score-partwise" {
        return Err(element_diagnostic(
            &elements[root],
            format!(
                "unsupported root <{}>; expected no-namespace <score-partwise>",
                elements[root].name
            ),
        ));
    }
    if let Some(element) = elements
        .iter()
        .find(|element| !element.namespace.is_empty())
    {
        return Err(element_diagnostic(
            element,
            "unsupported MusicXML namespace on an element; expected no namespace".to_owned(),
        ));
    }
    let mut value_errors = collect_value_errors(&elements);
    value_errors.extend(collect_voice_errors(&elements));
    value_errors.extend(collect_duration_errors(&elements));
    if !value_errors.is_empty() {
        return Err(value_errors.join("\n"));
    }
    require_attribute(&elements[root], "version", "4.0")?;
    require_container_text(&elements[root])?;

    let root_children = &elements[root].children;
    if root_children.len() < 2 || elements[root_children[0]].name != "part-list" {
        return Err(element_diagnostic(
            &elements[root],
            "unsupported shape: <score-partwise> must begin with <part-list> and contain Parts"
                .to_owned(),
        ));
    }
    let part_list = root_children[0];
    require_no_attributes(&elements[part_list])?;
    require_container_text(&elements[part_list])?;
    let mut score_parts = Vec::new();
    for score_part in &elements[part_list].children {
        let score_part_element = &elements[*score_part];
        if score_part_element.name != "score-part" {
            return Err(element_diagnostic(
                score_part_element,
                "unsupported shape: <part-list> may contain only <score-part>".to_owned(),
            ));
        }
        let id = required_attribute(score_part_element, "id")?.to_owned();
        if score_parts
            .iter()
            .any(|(existing_id, _): &(String, String)| existing_id == &id)
        {
            return Err(element_diagnostic(
                score_part_element,
                format!("duplicate score-part id {id:?}"),
            ));
        }
        require_container_text(score_part_element)?;
        let [part_name] = exact_children(&elements, *score_part, &["part-name"])?;
        let name = leaf_text(&elements[part_name])?.to_owned();
        if let Some(character) = name
            .chars()
            .find(|character| !noto_sans_glyphs::supports(*character))
        {
            return Err(element_diagnostic(
                &elements[part_name],
                format!("Part name contains unsupported character {character:?}"),
            ));
        }
        score_parts.push((id, name));
    }
    if score_parts.is_empty() {
        return Err(element_diagnostic(
            &elements[part_list],
            "unsupported score: <part-list> must contain at least one <score-part>".to_owned(),
        ));
    }
    let mut part_elements = Vec::new();
    for part in &root_children[1..] {
        let part_element = &elements[*part];
        if part_element.name != "part" {
            return Err(element_diagnostic(
                part_element,
                format!("unsupported score-partwise child <{}>", part_element.name),
            ));
        }
        let id = required_attribute(part_element, "id")?;
        if part_elements
            .iter()
            .any(|(existing_id, _): &(&str, usize)| existing_id == &id)
        {
            return Err(element_diagnostic(
                part_element,
                format!("duplicate Part id {id:?}"),
            ));
        }
        if !score_parts.iter().any(|(score_id, _)| score_id == id) {
            return Err(element_diagnostic(
                part_element,
                format!("Part id {id:?} has no matching score-part"),
            ));
        }
        part_elements.push((id, *part));
    }
    if part_elements.len() != score_parts.len() {
        let (missing_id, _) = score_parts
            .iter()
            .find(|(score_id, _)| !part_elements.iter().any(|(part_id, _)| part_id == score_id))
            .expect("different counts imply an unmatched score-part");
        return Err(element_diagnostic(
            &elements[part_list],
            format!("score-part id {missing_id:?} has no matching Part"),
        ));
    }
    let mut parts = Vec::new();
    let mut expected_measure_count = None;
    let mut warnings = Vec::new();
    for (score_part_id, name) in &score_parts {
        let (_, part) = part_elements
            .iter()
            .find(|(part_id, _)| part_id == score_part_id)
            .expect("Part IDs were checked against score-part IDs");
        let part = *part;
        if required_attribute(&elements[part], "id")? != score_part_id {
            unreachable!("Part was selected by matching ID");
        }
        require_container_text(&elements[part])?;
        let mut divisions = None;
        let mut input_staff_count = 1;
        let mut active_staff_states = vec![InputStaffState::default()];
        let mut pending_ties: VoiceState<Vec<Tone>> = Vec::new();
        let mut active_ending: Option<String> = None;
        let mut active_beams: VoiceState<[bool; 5]> = Vec::new();
        let mut measures = Vec::new();
        for measure_id in &elements[part].children {
            let measure_element = &elements[*measure_id];
            if measure_element.name != "measure" {
                return Err(element_diagnostic(
                    measure_element,
                    "unsupported shape: <part> may contain only <measure> elements".to_owned(),
                ));
            }
            required_attribute(measure_element, "number")?;
            require_container_text(measure_element)?;
            let mut events: Vec<Event> = Vec::new();
            let mut harmonies = Vec::new();
            let mut texts = Vec::new();
            let mut barlines = Vec::new();
            let mut event_sources = Vec::new();
            let mut cursor_units = 0.0;
            let mut chord_targets: VoiceState<usize> = Vec::new();
            let mut active_tuplets: VoiceState<Option<String>> = Vec::new();
            for child in &measure_element.children {
                let element = &elements[*child];
                match element.name.as_str() {
                    "attributes" => {
                        require_no_attributes(element)?;
                        require_container_text(element)?;
                        let declared_staves = element
                            .children
                            .iter()
                            .filter(|child| elements[**child].name == "staves")
                            .copied()
                            .collect::<Vec<_>>();
                        if declared_staves.len() > 1 {
                            return Err(element_diagnostic(
                                &elements[declared_staves[1]],
                                "attributes contain multiple <staves> declarations".to_owned(),
                            ));
                        }
                        if let Some(staves) = declared_staves.first() {
                            input_staff_count = leaf_text(&elements[*staves])?
                                .parse::<u32>()
                                .ok()
                                .filter(|value| *value > 0)
                                .ok_or_else(|| {
                                    element_diagnostic(
                                        &elements[*staves],
                                        "<staves> must be a positive integer".to_owned(),
                                    )
                                })?;
                            active_staff_states
                                .resize(input_staff_count as usize, InputStaffState::default());
                        }
                        let mut saw_divisions = false;
                        let mut saw_staves = false;
                        for attribute in &element.children {
                            let attribute_element = &elements[*attribute];
                            match attribute_element.name.as_str() {
                                "divisions" if !saw_divisions => {
                                    let value = leaf_text(attribute_element)?
                                        .parse::<u32>()
                                        .ok()
                                        .filter(|value| *value > 0)
                                        .ok_or_else(|| {
                                            element_diagnostic(
                                                attribute_element,
                                                "<divisions> must be a positive integer".to_owned(),
                                            )
                                        })?;
                                    divisions = Some(value);
                                    saw_divisions = true;
                                }
                                "staves" if !saw_staves => {
                                    saw_staves = true;
                                }
                                "transpose" => {
                                    let transposition = transpose_semitones(&elements, *attribute)?;
                                    if let Some(number) =
                                        optional_attribute(attribute_element, "number")
                                    {
                                        let staff_number = parse_input_staff_index(
                                            attribute_element,
                                            number,
                                            input_staff_count,
                                        )?;
                                        active_staff_states[staff_number].transposition =
                                            transposition;
                                    } else {
                                        for state in &mut active_staff_states {
                                            state.transposition = transposition;
                                        }
                                    }
                                }
                                "time" => {
                                    let meter = parse_time(&elements, *attribute)?;
                                    if let Some(number) =
                                        optional_attribute(attribute_element, "number")
                                    {
                                        let staff_number = parse_input_staff_index(
                                            attribute_element,
                                            number,
                                            input_staff_count,
                                        )?;
                                        active_staff_states[staff_number].meter = Some(meter);
                                    } else {
                                        for state in &mut active_staff_states {
                                            state.meter = Some(meter);
                                        }
                                    }
                                }
                                "key" => {
                                    let key_signature = parse_key_signature(&elements, *attribute)?;
                                    if let Some(number) =
                                        optional_attribute(attribute_element, "number")
                                    {
                                        let staff_index = parse_input_staff_index(
                                            attribute_element,
                                            number,
                                            input_staff_count,
                                        )?;
                                        active_staff_states[staff_index].key_signature =
                                            Some(key_signature);
                                    } else {
                                        for state in &mut active_staff_states {
                                            state.key_signature = Some(key_signature.clone());
                                        }
                                    }
                                }
                                "clef" => {
                                    let clef = parse_input_clef(&elements, *attribute)?;
                                    if let Some(number) =
                                        optional_attribute(attribute_element, "number")
                                    {
                                        let staff_index = parse_input_staff_index(
                                            attribute_element,
                                            number,
                                            input_staff_count,
                                        )?;
                                        active_staff_states[staff_index].clef = Some(clef);
                                    } else {
                                        for state in &mut active_staff_states {
                                            state.clef = Some(clef.clone());
                                        }
                                    }
                                }
                                _ => {
                                    return Err(element_diagnostic(
                                        attribute_element,
                                        "unsupported score attribute".to_owned(),
                                    ));
                                }
                            }
                        }
                        if element.children.is_empty() {
                            return Err(element_diagnostic(
                                element,
                                "unsupported shape: <attributes> may not be empty".to_owned(),
                            ));
                        }
                    }
                    "note" => {
                        let divisions = divisions.ok_or_else(|| {
                            element_diagnostic(
                                element,
                                "a positive <divisions> value is required before note events"
                                    .to_owned(),
                            )
                        })?;
                        let mut parsed = parse_note(
                            &elements,
                            *child,
                            divisions,
                            &active_staff_states,
                            input_staff_count,
                            &active_tuplets,
                            &mut warnings,
                        )?;
                        let tuplet_index =
                            voice_state_index(&mut active_tuplets, &parsed.event.voice_id);
                        match parsed.tuplet_boundary.take() {
                            Some(TupletBoundary::Start(number)) => {
                                if active_tuplets[tuplet_index].1.is_some() {
                                    return Err(element_diagnostic(
                                        element,
                                        "nested tuplet groups are unsupported".to_owned(),
                                    ));
                                }
                                active_tuplets[tuplet_index].1 = Some(number);
                            }
                            Some(TupletBoundary::Stop(number)) => {
                                match active_tuplets[tuplet_index].1.take() {
                                    Some(active_number) if active_number == number => {}
                                    Some(active_number) => {
                                        return Err(element_diagnostic(
                                            element,
                                            format!("tuplet stop number {number:?} does not match active tuplet {active_number:?}"),
                                        ));
                                    }
                                    None => {
                                        return Err(element_diagnostic(
                                            element,
                                            "tuplet stop has no matching start".to_owned(),
                                        ));
                                    }
                                }
                            }
                            None => {}
                        }
                        parsed.event.onset_units = cursor_units;
                        if let Some(chord_marker) = parsed.chord_marker {
                            let Some(target) = chord_targets
                                .iter()
                                .find(|(voice, _)| voice == &parsed.event.voice_id)
                                .map(|(_, target)| *target)
                            else {
                                return Err(element_diagnostic(
                                    &elements[chord_marker],
                                    "<chord/> marker requires a preceding pitched event".to_owned(),
                                ));
                            };
                            let event: &mut Event = &mut events[target];
                            parsed.event.onset_units = event.onset_units;
                            if event.voice_id != parsed.event.voice_id {
                                return Err(element_diagnostic(
                                    &elements[chord_marker],
                                    "chord members must share a Voice".to_owned(),
                                ));
                            }
                            if event.duration_units != parsed.event.duration_units
                                || event.duration_name != parsed.event.duration_name
                                || event.dots != parsed.event.dots
                            {
                                return Err(element_diagnostic(
                                    &elements[chord_marker],
                                    "chord tone duration must match the preceding event".to_owned(),
                                ));
                            }
                            if !same_beams(&event.beams, &parsed.event.beams) {
                                return Err(element_diagnostic(
                                    &elements[chord_marker],
                                    "chord tone beam declarations must match the preceding event"
                                        .to_owned(),
                                ));
                            }
                            if input_staff_mode == InputStaffMode::Combine
                                && event.input_staff != parsed.event.input_staff
                            {
                                return Err(element_diagnostic(
                                    &elements[chord_marker],
                                    "a chord may not mix Input Staves in combine mode".to_owned(),
                                ));
                            }
                            let mut chord_tones = parsed.event.tones;
                            let tone = chord_tones.pop().ok_or_else(|| {
                                element_diagnostic(
                                    &elements[chord_marker],
                                    "rest chords are unsupported".to_owned(),
                                )
                            })?;
                            if event
                                .tones
                                .iter()
                                .any(|existing| existing.tone == tone.tone)
                            {
                                return Err(element_diagnostic(
                                    &elements[chord_marker],
                                    "chord contains a duplicate concert pitch".to_owned(),
                                ));
                            }
                            event.tones.push(tone);
                        } else {
                            let duration_units = parsed.event.duration_units;
                            let voice_id = parsed.event.voice_id.clone();
                            let is_pitched = !parsed.event.tones.is_empty();
                            events.push(parsed.event);
                            event_sources.push(*child);
                            if is_pitched {
                                if let Some((_, target)) = chord_targets
                                    .iter_mut()
                                    .find(|(voice, _)| voice == &voice_id)
                                {
                                    *target = events.len() - 1;
                                } else {
                                    chord_targets.push((voice_id, events.len() - 1));
                                }
                            } else {
                                chord_targets.retain(|(voice, _)| voice != &voice_id);
                            }
                            cursor_units += duration_units;
                        }
                    }
                    "backup" | "forward" => {
                        let movement_units =
                            parse_cursor_movement(&elements, *child, divisions, input_staff_count)?;
                        if element.name == "backup" {
                            if cursor_units < movement_units {
                                return Err(element_diagnostic(
                                    element,
                                    "<backup> moves the Voice cursor before the measure start"
                                        .to_owned(),
                                ));
                            }
                            cursor_units -= movement_units;
                        } else {
                            cursor_units += movement_units;
                        }
                    }
                    "harmony" => {
                        if let Some((symbol, offset_x, input_staff)) = parse_chord_symbol(
                            &elements,
                            *child,
                            &mut warnings,
                            divisions,
                            input_staff_count,
                        )? {
                            harmonies.push(PositionedHarmony {
                                event_index: events.len(),
                                input_staff,
                                offset_x,
                                symbol,
                            });
                        }
                    }
                    "direction" => texts.extend(parse_direction(
                        &elements,
                        *child,
                        events.len(),
                        divisions,
                        input_staff_count,
                        &mut warnings,
                    )?),
                    "barline" => {
                        let parsed = parse_repeat_barline(&elements, *child, events.len())?;
                        if let Some(label) = &parsed.ending_start {
                            if active_ending.is_some() {
                                return Err(element_diagnostic(
                                    element,
                                    "ending starts overlap before the preceding ending ends"
                                        .to_owned(),
                                ));
                            }
                            active_ending = Some(label.number.clone());
                        }
                        if let Some(label) = &parsed.ending_end {
                            match active_ending.take() {
                                Some(active_label) if active_label == label.number => {}
                                Some(active_label) => {
                                    return Err(element_diagnostic(
                                        element,
                                        format!("ending endpoint {:?} does not match active ending {active_label:?}", label.number),
                                    ));
                                }
                                None => {
                                    return Err(element_diagnostic(
                                        element,
                                        "ending stop/discontinue has no matching start".to_owned(),
                                    ));
                                }
                            }
                        }
                        barlines.push(parsed);
                    }
                    _ => {
                        return Err(element_diagnostic(
                            element,
                            format!("unsupported measure element <{}>", element.name),
                        ));
                    }
                }
            }
            if active_tuplets.iter().any(|(_, active)| active.is_some()) {
                return Err(element_diagnostic(
                    measure_element,
                    "tuplet group does not end before the measure boundary".to_owned(),
                ));
            }
            if events.is_empty() {
                return Err(element_diagnostic(
                    measure_element,
                    "unsupported score: a measure must contain at least one event".to_owned(),
                ));
            }
            for (event, source) in events.iter().zip(event_sources) {
                let voice_index = voice_state_index(&mut active_beams, &event.voice_id);
                validate_beam_group(&elements[source], event, &mut active_beams[voice_index].1)?;
                validate_event_ties(&elements[source], event, &mut pending_ties)?;
            }
            measures.push(Measure {
                events,
                harmonies,
                texts,
                meter: active_staff_states.first().and_then(|state| state.meter),
                input_staff_states: active_staff_states.clone(),
                barlines,
            });
        }
        if measures.is_empty() {
            return Err(element_diagnostic(
                &elements[part],
                "unsupported score: <part> must contain at least one measure".to_owned(),
            ));
        }
        if pending_ties.iter().any(|(_, ties)| !ties.is_empty()) {
            return Err(element_diagnostic(
                &elements[part],
                "tie start has no matching tie stop".to_owned(),
            ));
        }
        if active_beams
            .iter()
            .any(|(_, active)| active.iter().skip(1).any(|active| *active))
        {
            return Err(element_diagnostic(
                &elements[part],
                "beam group is missing an end marker".to_owned(),
            ));
        }
        if let Some(label) = active_ending {
            return Err(element_diagnostic(
                &elements[part],
                format!("ending {label:?} start has no matching stop or discontinue"),
            ));
        }
        let has_repeat_start = measures
            .iter()
            .flat_map(|measure| &measure.barlines)
            .any(|barline| barline.repeat_start);
        let has_repeat_end = measures
            .iter()
            .flat_map(|measure| &measure.barlines)
            .any(|barline| barline.repeat_end);
        if has_repeat_end && !has_repeat_start {
            measures
                .first_mut()
                .expect("nonempty Part has at least one measure")
                .barlines
                .push(MeasureBarline {
                    location: "left".to_owned(),
                    event_index: 0,
                    repeat_start: true,
                    ..MeasureBarline::default()
                });
        } else if has_repeat_start && !has_repeat_end {
            measures
                .last_mut()
                .expect("nonempty Part has at least one measure")
                .barlines
                .push(MeasureBarline {
                    location: "right".to_owned(),
                    event_index: 0,
                    repeat_end: true,
                    ..MeasureBarline::default()
                });
        }

        if let Some(expected) = expected_measure_count {
            if measures.len() != expected {
                return Err(element_diagnostic(
                    &elements[part],
                    format!("Part {score_part_id:?} has {} measures; expected {expected} to align with the other Parts", measures.len()),
                ));
            }
        } else {
            expected_measure_count = Some(measures.len());
        }
        parts.push(Part {
            name: name.clone(),
            input_staff_count,
            measures,
        });
    }

    Ok(Score { parts, warnings })
}

fn parse_repeat_barline(
    elements: &[Element],
    barline: usize,
    event_index: usize,
) -> Result<MeasureBarline, String> {
    let element = &elements[barline];
    validate_attributes(element, &["location"])?;
    require_container_text(element)?;
    let location = optional_attribute(element, "location").unwrap_or("right");
    if !["left", "middle", "right"].contains(&location) {
        return Err(element_diagnostic(
            element,
            format!("invalid barline location {location:?}"),
        ));
    }

    let mut barline = MeasureBarline {
        location: location.to_owned(),
        event_index,
        ..MeasureBarline::default()
    };
    for child in &element.children {
        let child_element = &elements[*child];
        match child_element.name.as_str() {
            "bar-style" => {
                if barline.style.is_some() {
                    return Err(element_diagnostic(
                        child_element,
                        "barline contains multiple bar-style elements".to_owned(),
                    ));
                }
                require_no_attributes(child_element)?;
                let style = leaf_text(child_element)?;
                if ![
                    "regular",
                    "dotted",
                    "dashed",
                    "heavy",
                    "light-light",
                    "light-heavy",
                    "heavy-light",
                    "heavy-heavy",
                    "tick",
                    "short",
                    "none",
                ]
                .contains(&style)
                {
                    return Err(element_diagnostic(
                        child_element,
                        format!("invalid bar-style {style:?}"),
                    ));
                }
                barline.style = Some(style.to_owned());
            }
            "repeat" => {
                if barline.repeat_start || barline.repeat_end {
                    return Err(element_diagnostic(
                        child_element,
                        "barline contains multiple repeat elements".to_owned(),
                    ));
                }
                validate_attributes(child_element, &["direction", "times", "winged"])?;
                require_container_text(child_element)?;
                if !child_element.children.is_empty() {
                    return Err(element_diagnostic(
                        child_element,
                        "repeat may not contain child elements".to_owned(),
                    ));
                }
                match required_attribute(child_element, "direction")? {
                    "forward" if location == "left" => barline.repeat_start = true,
                    "backward" if location != "left" => barline.repeat_end = true,
                    "forward" => {
                        return Err(element_diagnostic(
                            child_element,
                            "forward repeat must be on a left barline".to_owned(),
                        ));
                    }
                    "backward" => {
                        return Err(element_diagnostic(
                            child_element,
                            "backward repeat must be on a middle or right barline".to_owned(),
                        ));
                    }
                    direction => {
                        return Err(element_diagnostic(
                            child_element,
                            format!("invalid repeat direction {direction:?}"),
                        ));
                    }
                }
                if let Some(times) = optional_attribute(child_element, "times") {
                    if times
                        .parse::<u32>()
                        .ok()
                        .filter(|times| *times > 0)
                        .is_none()
                    {
                        return Err(element_diagnostic(
                            child_element,
                            format!("repeat times must be a positive integer, not {times:?}"),
                        ));
                    }
                }
                if let Some(winged) = optional_attribute(child_element, "winged") {
                    if !["none", "straight", "curved", "double"].contains(&winged) {
                        return Err(element_diagnostic(
                            child_element,
                            format!("invalid repeat winged style {winged:?}"),
                        ));
                    }
                }
            }
            "ending" => {
                validate_attributes(child_element, &["number", "type", "text"])?;
                require_container_text(child_element)?;
                if !child_element.children.is_empty() {
                    return Err(element_diagnostic(
                        child_element,
                        "ending may not contain child elements".to_owned(),
                    ));
                }
                let number = optional_attribute(child_element, "number").ok_or_else(|| {
                    element_diagnostic(
                        child_element,
                        "ending requires a number attribute".to_owned(),
                    )
                })?;
                let label = optional_attribute(child_element, "text")
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .map(str::to_owned)
                    .or_else(|| dozenal_ending_label(number))
                    .ok_or_else(|| {
                        element_diagnostic(
                            child_element,
                            format!("invalid ending number list {number:?}"),
                        )
                    })?;
                if let Some(character) = label
                    .chars()
                    .find(|character| !noto_sans_glyphs::supports(*character))
                {
                    return Err(element_diagnostic(
                        child_element,
                        format!("ending label contains unsupported character {character:?}"),
                    ));
                }
                match optional_attribute(child_element, "type").ok_or_else(|| {
                    element_diagnostic(child_element, "ending requires a type attribute".to_owned())
                })? {
                    "start" if location != "right" => {
                        if barline.ending_start.is_some() {
                            return Err(element_diagnostic(
                                child_element,
                                "barline contains multiple ending starts".to_owned(),
                            ));
                        }
                        barline.ending_start = Some(EndingMark {
                            number: number.to_owned(),
                            label,
                            discontinue: false,
                        })
                    }
                    ending_type @ ("stop" | "discontinue") if location != "left" => {
                        if barline.ending_end.is_some() {
                            return Err(element_diagnostic(
                                child_element,
                                "barline contains multiple ending endpoints".to_owned(),
                            ));
                        }
                        barline.ending_end = Some(EndingMark {
                            number: number.to_owned(),
                            label,
                            discontinue: ending_type == "discontinue",
                        })
                    }
                    "start" => {
                        return Err(element_diagnostic(
                            child_element,
                            "ending start must be on a left or middle barline".to_owned(),
                        ));
                    }
                    ending_type => {
                        return Err(element_diagnostic(
                            child_element,
                            format!("invalid ending type/location {ending_type:?}/{location:?}"),
                        ));
                    }
                }
            }
            _ => {
                return Err(element_diagnostic(
                    child_element,
                    format!("unsupported barline child <{}>", child_element.name),
                ));
            }
        }
    }
    Ok(barline)
}

fn dozenal_ending_label(number: &str) -> Option<String> {
    let mut labels = Vec::new();
    for item in number.split(',') {
        let item = item.trim();
        if item.is_empty() {
            return None;
        }
        let value = item.parse::<u32>().ok()?;
        let label = if value == 0 {
            "0".to_owned()
        } else {
            let mut digits = Vec::new();
            let mut remaining = value;
            while remaining > 0 {
                digits.push(match remaining % 12 {
                    10 => '↊',
                    11 => '↋',
                    digit => char::from_digit(digit, 10)?,
                });
                remaining /= 12;
            }
            digits.iter().rev().collect()
        };
        labels.push(label);
    }
    Some(labels.join(", "))
}

fn parse_direction(
    elements: &[Element],
    direction: usize,
    event_index: usize,
    divisions: Option<u32>,
    input_staff_count: u32,
    warnings: &mut Vec<String>,
) -> Result<Vec<PositionedText>, String> {
    let element = &elements[direction];
    validate_attributes(element, &["placement", "directive", "system", "id"])?;
    require_container_text(element)?;
    let placement = match optional_attribute(element, "placement").unwrap_or("above") {
        "above" => TextPlacement::Above,
        "below" => TextPlacement::Below,
        value => {
            return Err(element_diagnostic(
                element,
                format!("invalid direction placement {value:?}"),
            ));
        }
    };
    for attribute in ["directive", "system"] {
        if let Some(value) = optional_attribute(element, attribute) {
            if !matches!(value, "yes" | "no") {
                return Err(element_diagnostic(
                    element,
                    format!("direction {attribute} must be 'yes' or 'no', not {value:?}"),
                ));
            }
            if value == "yes" {
                warnings.push(element_diagnostic(
                    element,
                    format!("direction {attribute} hint is not supported; ignored"),
                ));
            }
        }
    }

    let mut text_items = Vec::new();
    let mut saw_direction_type = false;
    let mut saw_offset = false;
    let mut offset_ticks = 0i32;
    let mut after_direction_types = false;
    let mut input_staff = None;
    for child in &element.children {
        let child_element = &elements[*child];
        match child_element.name.as_str() {
            "direction-type" if !after_direction_types => {
                require_no_attributes(child_element)?;
                require_container_text(child_element)?;
                if child_element.children.is_empty() {
                    return Err(element_diagnostic(
                        child_element,
                        "direction-type must contain at least one item".to_owned(),
                    ));
                }
                saw_direction_type = true;
                for item in &child_element.children {
                    let text_element = &elements[*item];
                    match text_element.name.as_str() {
                        "words" | "rehearsal" => {
                            if let Some(text) = parse_direction_text(
                                elements,
                                *item,
                                event_index,
                                placement,
                                text_element.name == "rehearsal",
                                warnings,
                            )? {
                                text_items.push(text);
                            }
                        }
                        "symbol" => {
                            if let Some(symbol) = parse_direction_symbol(
                                elements,
                                *item,
                                event_index,
                                placement,
                                warnings,
                            )? {
                                text_items.push(symbol);
                            }
                        }
                        "segno" | "coda" => {
                            text_items.push(parse_direction_mark(
                                elements,
                                *item,
                                event_index,
                                placement,
                                warnings,
                            )?);
                        }
                        "metronome" | "dynamics" | "wedge" | "bracket" | "dashes" | "pedal"
                        | "octave-shift" | "other-direction" => {
                            warnings.push(element_diagnostic(
                                text_element,
                                format!(
                                    "<{}> direction is not supported; omitted",
                                    text_element.name
                                ),
                            ));
                        }
                        _ => {
                            return Err(element_diagnostic(
                                text_element,
                                format!("unsupported direction-type item <{}>", text_element.name),
                            ));
                        }
                    }
                }
            }
            "offset" if !saw_offset && saw_direction_type => {
                after_direction_types = true;
                validate_attributes(child_element, &["sound"])?;
                if let Some(sound) = optional_attribute(child_element, "sound") {
                    if !matches!(sound, "yes" | "no") {
                        return Err(element_diagnostic(
                            child_element,
                            format!("offset sound must be 'yes' or 'no', not {sound:?}"),
                        ));
                    }
                }
                offset_ticks = leaf_text(child_element)?.parse::<i32>().map_err(|_| {
                    element_diagnostic(child_element, "offset must be a signed integer".to_owned())
                })?;
                saw_offset = true;
            }
            "voice" | "staff" if saw_direction_type => {
                after_direction_types = true;
                let value = leaf_text(child_element)?;
                if child_element.name == "staff" {
                    let staff_index =
                        parse_input_staff_index(child_element, value, input_staff_count)?;
                    input_staff = Some(staff_index as u32 + 1);
                }
            }
            "sound" if saw_direction_type => {
                after_direction_types = true;
            }
            "footnote" | "level" | "listening" if saw_direction_type => {
                after_direction_types = true;
                warnings.push(element_diagnostic(
                    child_element,
                    format!(
                        "<{}> direction content is not supported; ignored",
                        child_element.name
                    ),
                ));
            }
            "direction-type" | "offset" | "voice" | "staff" | "footnote" | "level" | "sound"
            | "listening" => {
                return Err(element_diagnostic(
                    child_element,
                    format!(
                        "malformed direction child order at <{}>",
                        child_element.name
                    ),
                ));
            }
            _ => {
                return Err(element_diagnostic(
                    child_element,
                    format!("unsupported direction child <{}>", child_element.name),
                ));
            }
        }
    }
    if !saw_direction_type {
        return Err(element_diagnostic(
            element,
            "direction must contain at least one direction-type".to_owned(),
        ));
    }
    let offset_x = if offset_ticks == 0 {
        0.0
    } else {
        let divisions = divisions.ok_or_else(|| {
            element_diagnostic(
                element,
                "a positive divisions value is required for direction offset".to_owned(),
            )
        })?;
        f64::from(offset_ticks) * 40.0 / f64::from(divisions)
    };
    for text in &mut text_items {
        text.offset_x = offset_x;
        text.input_staff = input_staff;
    }
    Ok(text_items)
}

fn parse_direction_symbol(
    elements: &[Element],
    symbol_id: usize,
    event_index: usize,
    placement: TextPlacement,
    warnings: &mut Vec<String>,
) -> Result<Option<PositionedText>, String> {
    let element = &elements[symbol_id];
    validate_attributes(element, DIRECTION_GLYPH_ATTRIBUTES)?;
    if !element.children.is_empty() {
        return Err(element_diagnostic(
            element,
            "<symbol> must contain a glyph name".to_owned(),
        ));
    }
    let name = collapse_xml_whitespace(&element.text);
    if !valid_smufl_glyph_name(&name) {
        return Err(element_diagnostic(
            element,
            format!("invalid SMuFL symbol name {name:?}"),
        ));
    }
    let (font_size, color, coordinates) = parse_direction_glyph_style(element, warnings)?;
    let Some(glyph) = bravura_glyphs::direction_symbol(&name) else {
        warnings.push(element_diagnostic(
            element,
            format!("SMuFL symbol {name:?} is not supported; omitted"),
        ));
        return Ok(None);
    };
    Ok(Some(PositionedText {
        event_index,
        input_staff: None,
        offset_x: 0.0,
        text: String::new(),
        placement,
        font_size,
        color,
        enclosure: None,
        default_x: coordinates[0],
        default_y: coordinates[1],
        relative_x: coordinates[2],
        relative_y: coordinates[3],
        path_glyph: Some(glyph),
        path_id_prefix: "symbol",
        path_scale: 1.0,
        path_y_offset: 0.0,
    }))
}

const DIRECTION_GLYPH_ATTRIBUTES: &[&str] = &[
    "id",
    "default-x",
    "default-y",
    "relative-x",
    "relative-y",
    "font-family",
    "font-style",
    "font-size",
    "font-weight",
    "color",
    "halign",
    "valign",
    "justify",
    "text-decoration",
    "text-rotation",
    "letter-spacing",
    "line-height",
    "xml:lang",
    "xml:space",
    "enclosure",
];

fn parse_direction_glyph_style(
    element: &Element,
    warnings: &mut Vec<String>,
) -> Result<(f64, String, [Option<f64>; 4]), String> {
    let font_size = match optional_attribute(element, "font-size") {
        Some(value) => parse_text_font_size(value)
            .ok_or_else(|| element_diagnostic(element, "invalid symbol font-size".to_owned()))?,
        None => 16.0,
    };
    let color = optional_attribute(element, "color").unwrap_or("#171717");
    if !valid_musicxml_color(color) {
        return Err(element_diagnostic(
            element,
            "symbol color must be a six-digit hexadecimal color".to_owned(),
        ));
    }
    if let Some(family) = optional_attribute(element, "font-family") {
        if family.trim().is_empty() {
            return Err(element_diagnostic(
                element,
                "font-family may not be empty".to_owned(),
            ));
        }
        warnings.push(element_diagnostic(
            element,
            "symbol font-family is not supported; bundled glyph is used".to_owned(),
        ));
    }
    for (attribute, allowed) in [
        ("font-style", &["normal", "italic", "oblique"][..]),
        ("font-weight", &["normal", "bold"][..]),
        ("halign", &["left", "center", "right"][..]),
        ("valign", &["top", "middle", "bottom", "baseline"][..]),
        ("justify", &["left", "center", "right"][..]),
        (
            "text-decoration",
            &["none", "underline", "overline", "line-through"][..],
        ),
    ] {
        if let Some(value) = optional_attribute(element, attribute) {
            if !allowed.contains(&value) {
                return Err(element_diagnostic(
                    element,
                    format!("invalid {attribute} value {value:?}"),
                ));
            }
            warnings.push(element_diagnostic(
                element,
                format!("symbol {attribute} is not supported; default behavior is used"),
            ));
        }
    }
    for attribute in ["text-rotation", "letter-spacing", "line-height"] {
        if let Some(value) = optional_attribute(element, attribute) {
            if value.parse::<f64>().is_err_and(|_| value != "normal")
                || value.parse::<f64>().is_ok_and(|number| !number.is_finite())
            {
                return Err(element_diagnostic(
                    element,
                    format!("text {attribute} must be a finite number"),
                ));
            }
            warnings.push(element_diagnostic(
                element,
                format!("symbol {attribute} is not supported; default behavior is used"),
            ));
        }
    }
    if let Some(value) = optional_attribute(element, "xml:space") {
        if !matches!(value, "default" | "preserve") {
            return Err(element_diagnostic(
                element,
                format!("invalid xml:space value {value:?}"),
            ));
        }
        warnings.push(element_diagnostic(
            element,
            "symbol xml:space is not supported; ignored".to_owned(),
        ));
    }
    if let Some(value) = optional_attribute(element, "xml:lang") {
        if value.is_empty() || value.chars().any(char::is_whitespace) {
            return Err(element_diagnostic(
                element,
                "invalid xml:lang value".to_owned(),
            ));
        }
        warnings.push(element_diagnostic(
            element,
            "symbol language hint is not supported; ignored".to_owned(),
        ));
    }
    if let Some(value) = optional_attribute(element, "enclosure") {
        if !matches!(
            value,
            "none"
                | "rectangle"
                | "square"
                | "oval"
                | "circle"
                | "bracket"
                | "triangle"
                | "diamond"
                | "pentagon"
                | "hexagon"
                | "heptagon"
                | "octagon"
                | "nonagon"
                | "decagon"
        ) {
            return Err(element_diagnostic(
                element,
                format!("invalid symbol enclosure {value:?}"),
            ));
        }
        if value != "none" {
            warnings.push(element_diagnostic(
                element,
                "symbol enclosure is not supported; omitted".to_owned(),
            ));
        }
    }
    let mut coordinates = [None; 4];
    for (index, attribute) in ["default-x", "default-y", "relative-x", "relative-y"]
        .into_iter()
        .enumerate()
    {
        if let Some(value) = optional_attribute(element, attribute) {
            coordinates[index] = Some(
                value
                    .parse::<f64>()
                    .ok()
                    .filter(|number| number.is_finite())
                    .ok_or_else(|| {
                        element_diagnostic(
                            element,
                            format!("symbol {attribute} must be a finite number"),
                        )
                    })?,
            );
        }
    }
    Ok((font_size, color.to_ascii_lowercase(), coordinates))
}

fn parse_direction_mark(
    elements: &[Element],
    mark_id: usize,
    event_index: usize,
    placement: TextPlacement,
    warnings: &mut Vec<String>,
) -> Result<PositionedText, String> {
    let element = &elements[mark_id];
    let is_segno = element.name == "segno";
    validate_attributes(
        element,
        &[
            "smufl",
            "id",
            "default-x",
            "default-y",
            "relative-x",
            "relative-y",
            "font-family",
            "font-style",
            "font-size",
            "font-weight",
            "color",
            "halign",
            "valign",
            "justify",
            "text-decoration",
            "text-rotation",
            "letter-spacing",
            "line-height",
            "xml:lang",
            "xml:space",
            "enclosure",
        ],
    )?;
    if !element.children.is_empty() || !element.text.trim().is_empty() {
        return Err(element_diagnostic(
            element,
            format!("<{}> must be empty", element.name),
        ));
    }
    let conventional_name = if is_segno { "segno" } else { "coda" };
    let mut glyph = bravura_glyphs::navigation_mark(conventional_name)
        .expect("conventional Segno and Coda glyphs are bundled");
    if let Some(name) = optional_attribute(element, "smufl") {
        if !valid_smufl_glyph_name(name) {
            return Err(element_diagnostic(
                element,
                format!("invalid SMuFL glyph name {name:?}"),
            ));
        }
        if let Some(override_glyph) = bravura_glyphs::navigation_mark(name) {
            glyph = override_glyph;
        } else {
            warnings.push(element_diagnostic(
                element,
                format!("SMuFL glyph override {name:?} is not supported; conventional glyph used"),
            ));
        }
    }
    let (font_size, color, coordinates) = parse_direction_glyph_style(element, warnings)?;
    let segno_bounds = bravura_glyphs::segno().bounds;
    let segno_center = (segno_bounds.1 + segno_bounds.3) / 2.0;
    let glyph_center = (glyph.bounds.1 + glyph.bounds.3) / 2.0;
    Ok(PositionedText {
        event_index,
        input_staff: None,
        offset_x: 0.0,
        text: String::new(),
        placement,
        font_size,
        color,
        enclosure: None,
        default_x: coordinates[0],
        default_y: coordinates[1],
        relative_x: coordinates[2],
        relative_y: coordinates[3],
        path_glyph: Some(glyph),
        path_id_prefix: if is_segno { "segno" } else { "coda" },
        path_scale: 0.04,
        path_y_offset: segno_center - glyph_center,
    })
}

fn parse_direction_text(
    elements: &[Element],
    text_id: usize,
    event_index: usize,
    placement: TextPlacement,
    is_rehearsal: bool,
    warnings: &mut Vec<String>,
) -> Result<Option<PositionedText>, String> {
    let element = &elements[text_id];
    validate_attributes(
        element,
        &[
            "id",
            "default-x",
            "default-y",
            "relative-x",
            "relative-y",
            "font-family",
            "font-style",
            "font-size",
            "font-weight",
            "color",
            "halign",
            "valign",
            "justify",
            "text-decoration",
            "text-rotation",
            "letter-spacing",
            "line-height",
            "xml:lang",
            "xml:space",
            "text-direction",
            "enclosure",
        ],
    )?;
    if !element.children.is_empty() {
        return Err(element_diagnostic(
            element,
            format!("<{}> must contain plain text", element.name),
        ));
    }

    let preserve_space = match optional_attribute(element, "xml:space").unwrap_or("default") {
        "default" => false,
        "preserve" => true,
        value => {
            return Err(element_diagnostic(
                element,
                format!("invalid xml:space value {value:?}"),
            ));
        }
    };
    let text = if preserve_space {
        element.text.clone()
    } else {
        collapse_xml_whitespace(&element.text)
    };
    let mut font_size = 16.0;
    if let Some(value) = optional_attribute(element, "font-size") {
        font_size = parse_text_font_size(value).ok_or_else(|| {
            element_diagnostic(element, format!("invalid text font-size {value:?}"))
        })?;
    }
    let mut color = "#171717".to_owned();
    if let Some(value) = optional_attribute(element, "color") {
        if !valid_musicxml_color(value) {
            return Err(element_diagnostic(
                element,
                "text color must be a six-digit hexadecimal color".to_owned(),
            ));
        }
        color = value.to_ascii_lowercase();
    }
    if let Some(family) = optional_attribute(element, "font-family") {
        if family.trim().is_empty() {
            return Err(element_diagnostic(
                element,
                "font-family may not be empty".to_owned(),
            ));
        }
        if !family.eq_ignore_ascii_case("Noto Sans") {
            warnings.push(element_diagnostic(
                element,
                format!("font-family {family:?} is not bundled; Noto Sans is used"),
            ));
        }
    }
    for (attribute, allowed) in [
        ("font-style", &["normal", "italic", "oblique"][..]),
        ("font-weight", &["normal", "bold"][..]),
        ("halign", &["left", "center", "right"][..]),
        ("valign", &["top", "middle", "bottom", "baseline"][..]),
        ("justify", &["left", "center", "right"][..]),
        ("text-direction", &["ltr", "rtl", "lro", "rlo"][..]),
    ] {
        if let Some(value) = optional_attribute(element, attribute) {
            if !allowed.contains(&value) {
                return Err(element_diagnostic(
                    element,
                    format!("invalid {attribute} value {value:?}"),
                ));
            }
            warnings.push(element_diagnostic(
                element,
                format!("text {attribute} is not supported; default behavior is used"),
            ));
        }
    }
    let mut default_x = None;
    let mut default_y = None;
    let mut relative_x = None;
    let mut relative_y = None;
    for (attribute, output) in [
        ("default-x", &mut default_x),
        ("default-y", &mut default_y),
        ("relative-x", &mut relative_x),
        ("relative-y", &mut relative_y),
    ] {
        if let Some(value) = optional_attribute(element, attribute) {
            *output = Some(
                value
                    .parse::<f64>()
                    .ok()
                    .filter(|number| number.is_finite())
                    .ok_or_else(|| {
                        element_diagnostic(
                            element,
                            format!("text {attribute} must be a finite number"),
                        )
                    })?,
            );
        }
    }
    for attribute in ["text-rotation", "letter-spacing", "line-height"] {
        if let Some(value) = optional_attribute(element, attribute) {
            if value.parse::<f64>().is_err_and(|_| value != "normal")
                || value.parse::<f64>().is_ok_and(|number| !number.is_finite())
            {
                return Err(element_diagnostic(
                    element,
                    format!("text {attribute} must be a finite number"),
                ));
            }
            warnings.push(element_diagnostic(
                element,
                format!("text {attribute} is not supported; default behavior is used"),
            ));
        }
    }
    if let Some(language) = optional_attribute(element, "xml:lang") {
        if language.is_empty() || language.chars().any(char::is_whitespace) {
            return Err(element_diagnostic(
                element,
                "invalid xml:lang value".to_owned(),
            ));
        }
        warnings.push(element_diagnostic(
            element,
            "text language hint is not supported; ignored".to_owned(),
        ));
    }
    let enclosure = match optional_attribute(element, "enclosure") {
        Some("none") => None,
        Some(value)
            if matches!(
                value,
                "rectangle"
                    | "square"
                    | "oval"
                    | "circle"
                    | "bracket"
                    | "triangle"
                    | "diamond"
                    | "pentagon"
                    | "hexagon"
                    | "heptagon"
                    | "octagon"
                    | "nonagon"
                    | "decagon"
            ) =>
        {
            Some(value.to_owned())
        }
        Some(value) => {
            return Err(element_diagnostic(
                element,
                format!("invalid text enclosure {value:?}"),
            ))
        }
        None if is_rehearsal => Some("square".to_owned()),
        None => None,
    };
    if optional_attribute(element, "text-decoration").is_some() {
        warnings.push(element_diagnostic(
            element,
            "text-decoration is not supported; decoration is omitted".to_owned(),
        ));
    }
    let text = if text.is_empty() {
        text
    } else {
        match normalize_text_nfc(&text) {
            Ok(normalized) => normalized,
            Err(character) => {
                warnings.push(element_diagnostic(
                    element,
                    format!(
                        "unsupported text character U+{:04X}; text item omitted",
                        u32::from(character)
                    ),
                ));
                return Ok(None);
            }
        }
    };
    Ok(Some(PositionedText {
        event_index,
        input_staff: None,
        offset_x: 0.0,
        text,
        placement,
        font_size,
        color,
        enclosure,
        default_x,
        default_y,
        relative_x,
        relative_y,
        path_glyph: None,
        path_id_prefix: "",
        path_scale: 1.0,
        path_y_offset: 0.0,
    }))
}

fn parse_text_font_size(value: &str) -> Option<f64> {
    let (number, multiplier) = if let Some(number) = value.strip_suffix("pt") {
        (number, 4.0 / 3.0)
    } else if let Some(number) = value.strip_suffix("px") {
        (number, 1.0)
    } else {
        match value {
            "xx-small" => return Some(9.0),
            "x-small" => return Some(10.0),
            "small" => return Some(13.0),
            "medium" => return Some(16.0),
            "large" => return Some(18.0),
            "x-large" => return Some(24.0),
            "xx-large" => return Some(32.0),
            _ => (value, 4.0 / 3.0),
        }
    };
    let size = number.parse::<f64>().ok()? * multiplier;
    (size.is_finite() && size > 0.0).then_some(size)
}

fn normalize_text_nfc(value: &str) -> Result<String, char> {
    let normalized = text_normalization::normalize(value);
    for character in normalized.chars() {
        if !matches!(character, '\t' | '\n') && !noto_sans_glyphs::supports(character) {
            return Err(character);
        }
    }
    Ok(normalized)
}

fn collapse_xml_whitespace(value: &str) -> String {
    let mut collapsed = String::new();
    let mut pending_space = false;
    for character in value.chars() {
        if matches!(character, ' ' | '\t' | '\n' | '\r') {
            pending_space = !collapsed.is_empty();
        } else {
            if pending_space {
                collapsed.push(' ');
                pending_space = false;
            }
            collapsed.push(character);
        }
    }
    collapsed
}

fn parse_chord_symbol(
    elements: &[Element],
    harmony: usize,
    warnings: &mut Vec<String>,
    divisions: Option<u32>,
    input_staff_count: u32,
) -> Result<Option<(ChordSymbol, f64, Option<u32>)>, String> {
    let element = &elements[harmony];
    validate_attributes(
        element,
        &[
            "type",
            "print-object",
            "print-frame",
            "placement",
            "default-x",
            "default-y",
            "relative-x",
            "relative-y",
            "font-family",
            "font-style",
            "font-size",
            "font-weight",
            "color",
            "halign",
            "valign",
            "id",
        ],
    )?;
    require_container_text(element)?;
    let harmony_type = optional_attribute(element, "type").unwrap_or("explicit");
    if !matches!(harmony_type, "explicit" | "implied" | "alternate") {
        return Err(element_diagnostic(
            element,
            format!("invalid harmony type {harmony_type:?}"),
        ));
    }
    let print_object = optional_attribute(element, "print-object").unwrap_or("yes");
    if !matches!(print_object, "yes" | "no") {
        return Err(element_diagnostic(
            element,
            format!("print-object must be 'yes' or 'no', not {print_object:?}"),
        ));
    }
    let print_frame = optional_attribute(element, "print-frame").unwrap_or("no");
    if !matches!(print_frame, "yes" | "no") {
        return Err(element_diagnostic(
            element,
            format!("print-frame must be 'yes' or 'no', not {print_frame:?}"),
        ));
    }
    if let Some(placement) = optional_attribute(element, "placement") {
        if !matches!(placement, "above" | "below") {
            return Err(element_diagnostic(
                element,
                format!("invalid harmony placement {placement:?}"),
            ));
        }
        warnings.push(element_diagnostic(
            element,
            "harmony placement is not supported; symbol defaults above the staff".to_owned(),
        ));
    }
    for attribute in ["default-x", "default-y", "relative-x", "relative-y"] {
        if let Some(value) = optional_attribute(element, attribute) {
            if !value.parse::<f64>().is_ok_and(f64::is_finite) {
                return Err(element_diagnostic(
                    element,
                    format!("{attribute} must be a finite number"),
                ));
            }
            warnings.push(element_diagnostic(
                element,
                format!("harmony {attribute} is not supported; ignored"),
            ));
        }
    }
    for (attribute, allowed) in [
        ("font-style", &["normal", "italic"][..]),
        ("font-weight", &["normal", "bold"][..]),
        ("halign", &["left", "center", "right"][..]),
        ("valign", &["top", "middle", "bottom", "baseline"][..]),
    ] {
        if let Some(value) = optional_attribute(element, attribute) {
            if !allowed.contains(&value) {
                return Err(element_diagnostic(
                    element,
                    format!("invalid harmony {attribute} value {value:?}"),
                ));
            }
        }
    }
    if optional_attribute(element, "color").is_some_and(|color| !valid_musicxml_color(color)) {
        return Err(element_diagnostic(
            element,
            "harmony color must be a six-digit hexadecimal color".to_owned(),
        ));
    }
    if optional_attribute(element, "id").is_some_and(str::is_empty) {
        return Err(element_diagnostic(
            element,
            "harmony id must not be empty".to_owned(),
        ));
    }
    if element.attributes.iter().any(|(name, _)| {
        matches!(
            name.as_str(),
            "font-family"
                | "font-style"
                | "font-size"
                | "font-weight"
                | "color"
                | "halign"
                | "valign"
                | "id"
        )
    }) {
        warnings.push(element_diagnostic(
            element,
            "harmony typography, color, alignment, and id attributes are not supported; ignored"
                .to_owned(),
        ));
    }
    if print_object == "no" {
        return Ok(None);
    }
    if harmony_type != "explicit" {
        warnings.push(element_diagnostic(
            element,
            format!("{harmony_type} harmony is not supported; omitted"),
        ));
        return Ok(None);
    }
    if print_frame == "yes" {
        warnings.push(element_diagnostic(
            element,
            "harmony frames are not supported; omitted".to_owned(),
        ));
    }
    let children = &element.children;
    if children.len() < 2 {
        return Err(element_diagnostic(
            element,
            "unsupported harmony shape; expected root-based <root> followed by <kind>".to_owned(),
        ));
    }
    if matches!(elements[children[0]].name.as_str(), "numeral" | "function") {
        let form = &elements[children[0]];
        if elements[children[1]].name != "kind" {
            return Err(element_diagnostic(
                &elements[children[1]],
                "numeral/function harmony requires <kind> after its form".to_owned(),
            ));
        }
        if form.name == "function" {
            if let Some(child) = form.children.first() {
                return Err(element_diagnostic(
                    &elements[*child],
                    "elements are not allowed inside MusicXML <function>".to_owned(),
                ));
            }
            if form.text.trim().is_empty() {
                return Err(element_diagnostic(
                    form,
                    "MusicXML <function> content must not be empty".to_owned(),
                ));
            }
        } else {
            let Some(numeral_root) = form.children.first() else {
                return Err(element_diagnostic(
                    form,
                    "MusicXML <numeral> requires <numeral-root>".to_owned(),
                ));
            };
            let numeral_root = &elements[*numeral_root];
            if numeral_root.name != "numeral-root"
                || !numeral_root.children.is_empty()
                || numeral_root.text.trim().is_empty()
            {
                return Err(element_diagnostic(
                    numeral_root,
                    "MusicXML <numeral> requires a plain, nonempty <numeral-root>".to_owned(),
                ));
            }
        }
        warnings.push(element_diagnostic(
            form,
            format!("MusicXML <{}> harmony is not supported; omitted", form.name),
        ));
        return Ok(None);
    }
    if elements[children[0]].name != "root" {
        return Err(element_diagnostic(
            &elements[children[0]],
            "unsupported harmony shape; expected root-based <root>".to_owned(),
        ));
    }
    if elements[children[1]].name != "kind" {
        return Err(element_diagnostic(
            &elements[children[1]],
            "root-based harmony requires <kind> after <root>".to_owned(),
        ));
    }
    let root = parse_harmony_pitch(elements, children[0], "root-step", "root-alter")?;
    let kind_element = &elements[children[1]];
    validate_attributes(
        kind_element,
        &[
            "use-symbols",
            "text",
            "stack-degrees",
            "parentheses-degrees",
            "bracket-degrees",
            "font-family",
            "font-style",
            "font-size",
            "font-weight",
            "color",
            "halign",
            "valign",
        ],
    )?;
    if !kind_element.children.is_empty() || kind_element.text.trim().is_empty() {
        return Err(element_diagnostic(
            kind_element,
            "<kind> must contain a plain MusicXML kind value".to_owned(),
        ));
    }
    for attribute in [
        "use-symbols",
        "stack-degrees",
        "parentheses-degrees",
        "bracket-degrees",
    ] {
        if let Some(value) = optional_attribute(kind_element, attribute) {
            if !matches!(value, "yes" | "no") {
                return Err(element_diagnostic(
                    kind_element,
                    format!("{attribute} must be 'yes' or 'no', not {value:?}"),
                ));
            }
        }
    }
    if !kind_element.attributes.is_empty() {
        warnings.push(element_diagnostic(
            kind_element,
            "MusicXML kind display attributes are not supported; ignored".to_owned(),
        ));
    }
    let kind = kind_element.text.trim();
    let mut degrees: Vec<ChordDegree> = match kind {
        "major" => vec![
            ChordDegree::new(1, 0),
            ChordDegree::new(3, 4),
            ChordDegree::new(5, 7),
        ],
        "minor" => vec![
            ChordDegree::new(1, 0),
            ChordDegree::new(3, 3),
            ChordDegree::new(5, 7),
        ],
        "augmented" => vec![
            ChordDegree::new(1, 0),
            ChordDegree::new(3, 4),
            ChordDegree::new(5, 8),
        ],
        "diminished" => vec![
            ChordDegree::new(1, 0),
            ChordDegree::new(3, 3),
            ChordDegree::new(5, 6),
        ],
        "dominant" => vec![
            ChordDegree::new(1, 0),
            ChordDegree::new(3, 4),
            ChordDegree::new(5, 7),
            ChordDegree::new(7, 10),
        ],
        "major-seventh" => vec![
            ChordDegree::new(1, 0),
            ChordDegree::new(3, 4),
            ChordDegree::new(5, 7),
            ChordDegree::new(7, 11),
        ],
        "minor-seventh" => vec![
            ChordDegree::new(1, 0),
            ChordDegree::new(3, 3),
            ChordDegree::new(5, 7),
            ChordDegree::new(7, 10),
        ],
        "half-diminished" => vec![
            ChordDegree::new(1, 0),
            ChordDegree::new(3, 3),
            ChordDegree::new(5, 6),
            ChordDegree::new(7, 10),
        ],
        "diminished-seventh" => vec![
            ChordDegree::new(1, 0),
            ChordDegree::new(3, 3),
            ChordDegree::new(5, 6),
            ChordDegree::new(7, 9),
        ],
        "suspended-second" => vec![
            ChordDegree::new(1, 0),
            ChordDegree::new(2, 2),
            ChordDegree::new(5, 7),
        ],
        "suspended-fourth" => vec![
            ChordDegree::new(1, 0),
            ChordDegree::new(4, 5),
            ChordDegree::new(5, 7),
        ],
        "augmented-seventh" | "dominant-11th" | "dominant-13th" | "dominant-ninth" | "French"
        | "German" | "Italian" | "major-11th" | "major-13th" | "major-minor" | "major-ninth"
        | "major-sixth" | "minor-11th" | "minor-13th" | "minor-ninth" | "minor-sixth"
        | "Neapolitan" | "none" | "other" | "pedal" | "power" | "Tristan" => {
            warnings.push(element_diagnostic(
                &elements[children[1]],
                format!("valid MusicXML harmony kind {kind:?} is not supported; omitted"),
            ));
            return Ok(None);
        }
        _ => {
            return Err(element_diagnostic(
                &elements[children[1]],
                format!("invalid MusicXML harmony kind {kind:?}"),
            ));
        }
    };
    let mut next_child = 2;
    if children
        .get(next_child)
        .is_some_and(|child| elements[*child].name == "inversion")
    {
        let inversion = children[next_child];
        let value = leaf_text(&elements[inversion])?
            .parse::<u32>()
            .map_err(|_| {
                element_diagnostic(
                    &elements[inversion],
                    "inversion must be a nonnegative integer".to_owned(),
                )
            })?;
        warnings.push(element_diagnostic(
            &elements[inversion],
            format!("inversion {value} does not specify an explicit bass tone; ignored"),
        ));
        next_child += 1;
    }
    let bass = if children
        .get(next_child)
        .is_some_and(|child| elements[*child].name == "bass")
    {
        let bass = parse_harmony_pitch(elements, children[next_child], "bass-step", "bass-alter")?;
        next_child += 1;
        (bass != root).then_some(bass)
    } else {
        None
    };
    let mut seen_degrees = Vec::new();
    while children
        .get(next_child)
        .is_some_and(|child| elements[*child].name == "degree")
    {
        let degree_element = children[next_child];
        let change = parse_chord_degree(elements, degree_element)?;
        if seen_degrees.contains(&change.number) {
            warnings.push(element_diagnostic(
                &elements[degree_element],
                format!(
                    "conflicting operations for chord degree {}; Chord Symbol omitted",
                    change.number
                ),
            ));
            return Ok(None);
        }
        if let Some(position) = degrees
            .iter()
            .position(|degree| degree.number == change.number)
        {
            match change.operation {
                DegreeOperation::Add => {
                    warnings.push(element_diagnostic(
                        &elements[degree_element],
                        format!(
                            "added chord degree {} already exists; Chord Symbol omitted",
                            change.number
                        ),
                    ));
                    return Ok(None);
                }
                DegreeOperation::Alter => {
                    let Some(altered_interval) =
                        degrees[position].interval.checked_add(change.alteration)
                    else {
                        return Err(element_diagnostic(
                            &elements[degree_element],
                            "altered degree is outside the supported numeric range".to_owned(),
                        ));
                    };
                    degrees[position].interval = altered_interval;
                }
                DegreeOperation::Subtract => {
                    degrees.remove(position);
                }
            }
        } else if !matches!(change.operation, DegreeOperation::Add) {
            warnings.push(element_diagnostic(
                &elements[degree_element],
                format!(
                    "{} targets chord degree {}, absent from kind {kind:?}; Chord Symbol omitted",
                    change.operation.name(),
                    change.number
                ),
            ));
            return Ok(None);
        } else {
            let Some(base_interval) = added_degree_interval(change.number) else {
                warnings.push(element_diagnostic(
                    &elements[degree_element],
                    format!(
                        "unsupported added chord degree {}; Chord Symbol omitted",
                        change.number
                    ),
                ));
                return Ok(None);
            };
            let Some(added_interval) = base_interval.checked_add(change.alteration) else {
                return Err(element_diagnostic(
                    &elements[degree_element],
                    "added degree is outside the supported numeric range".to_owned(),
                ));
            };
            degrees.push(ChordDegree::new(change.number, added_interval));
        }
        seen_degrees.push(change.number);
        next_child += 1;
    }
    if children
        .get(next_child)
        .is_some_and(|child| elements[*child].name == "frame")
    {
        next_child += 1;
        warnings.push(element_diagnostic(
            &elements[children[next_child - 1]],
            "harmony frame diagrams are not supported; omitted".to_owned(),
        ));
    }
    let offset_x = if children
        .get(next_child)
        .is_some_and(|child| elements[*child].name == "offset")
    {
        let offset = children[next_child];
        require_no_attributes(&elements[offset])?;
        let ticks = leaf_text(&elements[offset])?.parse::<i32>().map_err(|_| {
            element_diagnostic(&elements[offset], "offset must be an integer".to_owned())
        })?;
        let divisions = divisions.ok_or_else(|| {
            element_diagnostic(
                &elements[offset],
                "a positive <divisions> value is required for harmony offsets".to_owned(),
            )
        })?;
        next_child += 1;
        f64::from(ticks) * 40.0 / f64::from(divisions)
    } else {
        0.0
    };
    while children
        .get(next_child)
        .is_some_and(|child| matches!(elements[*child].name.as_str(), "footnote" | "level"))
    {
        let unsupported = children[next_child];
        warnings.push(element_diagnostic(
            &elements[unsupported],
            format!(
                "harmony <{}> content is not supported; ignored",
                elements[unsupported].name
            ),
        ));
        next_child += 1;
    }
    let mut input_staff = None;
    if children
        .get(next_child)
        .is_some_and(|child| elements[*child].name == "staff")
    {
        let staff = children[next_child];
        let value = leaf_text(&elements[staff])?;
        let staff_index = parse_input_staff_index(&elements[staff], value, input_staff_count)?;
        input_staff = Some(staff_index as u32 + 1);
        next_child += 1;
    }
    if next_child != children.len() {
        return Err(element_diagnostic(
            &elements[children[next_child]],
            "unsupported harmony content".to_owned(),
        ));
    }
    let mut tones = degrees
        .iter()
        .map(|degree| ChordTone {
            interval: degree.interval,
            tone: (i64::from(root) + i64::from(degree.interval)).rem_euclid(12) as u8,
        })
        .filter(|tone| tone.tone != root && Some(tone.tone) != bass)
        .collect::<Vec<_>>();
    tones.sort_by_key(|tone| tone.interval);
    let mut seen_tones = Vec::new();
    tones.retain(|tone| {
        if seen_tones.contains(&tone.tone) {
            false
        } else {
            seen_tones.push(tone.tone);
            true
        }
    });
    Ok(Some((
        ChordSymbol { root, bass, tones },
        offset_x,
        input_staff,
    )))
}

fn parse_chord_degree(elements: &[Element], degree: usize) -> Result<DegreeChange, String> {
    let element = &elements[degree];
    require_no_attributes(element)?;
    require_container_text(element)?;
    let [value, alteration, degree_type] = exact_children(
        elements,
        degree,
        &["degree-value", "degree-alter", "degree-type"],
    )?;
    let value = leaf_text(&elements[value])?
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            element_diagnostic(
                &elements[value],
                "degree value must be a positive integer".to_owned(),
            )
        })?;
    let alteration = parse_integral_semitones(
        elements,
        alteration,
        "degree alteration must be an integer number of semitones",
    )?;
    let operation = match leaf_text(&elements[degree_type])? {
        "add" => DegreeOperation::Add,
        "alter" => DegreeOperation::Alter,
        "subtract" => DegreeOperation::Subtract,
        value => {
            return Err(element_diagnostic(
                &elements[degree_type],
                format!("invalid chord degree type {value:?}"),
            ));
        }
    };
    Ok(DegreeChange {
        number: value,
        alteration,
        operation,
    })
}

fn added_degree_interval(degree: u32) -> Option<i32> {
    let degree_offset = degree.checked_sub(1)?;
    let octave = i32::try_from(degree_offset / 7).ok()?.checked_mul(12)?;
    let simple = (degree_offset % 7) as usize;
    Some(octave + [0, 2, 4, 5, 7, 9, 10][simple])
}

fn parse_integral_semitones(
    elements: &[Element],
    element: usize,
    message: &str,
) -> Result<i32, String> {
    let value = leaf_text(&elements[element])?;
    let (integer, fraction) = value.split_once('.').unwrap_or((value, ""));
    if fraction.chars().any(|digit| digit != '0') {
        return Err(element_diagnostic(&elements[element], message.to_owned()));
    }
    integer
        .parse::<i32>()
        .map_err(|_| element_diagnostic(&elements[element], message.to_owned()))
}

fn parse_harmony_pitch(
    elements: &[Element],
    pitch: usize,
    step_name: &str,
    alter_name: &str,
) -> Result<u8, String> {
    let element = &elements[pitch];
    require_no_attributes(element)?;
    require_container_text(element)?;
    let children = &element.children;
    let step = children
        .first()
        .copied()
        .ok_or_else(|| element_diagnostic(element, format!("<{step_name}> is required")))?;
    if elements[step].name != step_name || children.len() > 2 {
        return Err(element_diagnostic(
            element,
            format!("unsupported shape: expected <{step_name}> and optional <{alter_name}>"),
        ));
    }
    let natural = match leaf_text(&elements[step])? {
        "C" => 0i32,
        "D" => 2,
        "E" => 4,
        "F" => 5,
        "G" => 7,
        "A" => 9,
        "B" => 11,
        value => {
            return Err(element_diagnostic(
                &elements[step],
                format!("unsupported pitch step {value:?}"),
            ));
        }
    };
    let alteration = if let Some(alter) = children.get(1).copied() {
        if elements[alter].name != alter_name {
            return Err(element_diagnostic(
                &elements[alter],
                format!("expected <{alter_name}> after <{step_name}>"),
            ));
        }
        parse_integral_semitones(
            elements,
            alter,
            "pitch alteration must be an integer number of semitones",
        )?
    } else {
        0
    };
    Ok((i64::from(natural) + i64::from(alteration)).rem_euclid(12) as u8)
}

fn parse_time(elements: &[Element], time: usize) -> Result<Meter, String> {
    let element = &elements[time];
    validate_attributes(element, &["number"])?;
    require_container_text(element)?;
    let [beats, beat_type] = exact_children(elements, time, &["beats", "beat-type"])?;
    let parse_digit = |id| -> Result<u8, String> {
        let text = leaf_text(&elements[id])?;
        text.parse::<u8>()
            .ok()
            .filter(|value| (1..=9).contains(value))
            .ok_or_else(|| {
                element_diagnostic(
                    &elements[id],
                    "unsupported time signature; beats and beat-type must each be one digit from 1 through 9"
                        .to_owned(),
                )
            })
    };
    Ok(Meter {
        beats: parse_digit(beats)?,
        beat_type: parse_digit(beat_type)?,
    })
}

fn parse_input_staff_index(
    element: &Element,
    number: &str,
    input_staff_count: u32,
) -> Result<usize, String> {
    let staff_number = number
        .parse::<u32>()
        .ok()
        .filter(|staff| *staff > 0 && *staff <= input_staff_count)
        .ok_or_else(|| {
            element_diagnostic(
                element,
                format!(
                    "Input Staff number must be between 1 and {input_staff_count}, not {number:?}"
                ),
            )
        })?;
    Ok(staff_number as usize - 1)
}

fn parse_key_signature(elements: &[Element], key: usize) -> Result<KeySignature, String> {
    let element = &elements[key];
    validate_attributes(element, &["number"])?;
    require_container_text(element)?;
    if element.children.is_empty() {
        return Err(element_diagnostic(
            element,
            "key must contain a supported key signature".to_owned(),
        ));
    }
    let mut previous_order = 0;
    let mut cancel = None;
    let mut fifths = None;
    let mut mode = None;
    for child in &element.children {
        let child = &elements[*child];
        let order = match child.name.as_str() {
            "cancel" => 1,
            "fifths" => 2,
            "mode" => 3,
            _ => {
                return Err(element_diagnostic(
                    child,
                    "unsupported key element".to_owned(),
                ));
            }
        };
        if order <= previous_order {
            return Err(element_diagnostic(
                child,
                "key elements are duplicated or out of order".to_owned(),
            ));
        }
        previous_order = order;
        let value = leaf_text(child)?;
        match child.name.as_str() {
            "cancel" if value.parse::<u8>().ok().is_some_and(|value| value <= 7) => {
                cancel = value.parse::<u8>().ok();
            }
            "fifths"
                if value
                    .parse::<i8>()
                    .ok()
                    .is_some_and(|value| (-7..=7).contains(&value)) =>
            {
                fifths = value.parse::<i8>().ok();
            }
            "mode"
                if matches!(
                    value,
                    "major"
                        | "minor"
                        | "dorian"
                        | "phrygian"
                        | "lydian"
                        | "mixolydian"
                        | "aeolian"
                        | "ionian"
                        | "locrian"
                        | "none"
                ) =>
            {
                mode = Some(value.to_owned())
            }
            "cancel" | "fifths" | "mode" => {
                return Err(element_diagnostic(
                    child,
                    format!("unsupported key value {value:?}"),
                ));
            }
            _ => unreachable!(),
        }
    }
    let fifths =
        fifths.ok_or_else(|| element_diagnostic(element, "key requires <fifths>".to_owned()))?;
    Ok(KeySignature {
        fifths,
        cancel,
        mode,
    })
}

fn parse_input_clef(elements: &[Element], clef: usize) -> Result<InputClef, String> {
    let element = &elements[clef];
    validate_attributes(element, &["number"])?;
    require_container_text(element)?;
    let mut sign = None;
    let mut line = None;
    let mut octave_change = 0;
    let mut previous_order = 0;
    for child in &element.children {
        let child = &elements[*child];
        let value = leaf_text(child)?;
        let order = match child.name.as_str() {
            "sign" => 1,
            "line" => 2,
            "clef-octave-change" => 3,
            _ => {
                return Err(element_diagnostic(
                    child,
                    format!("unsupported clef element <{}>", child.name),
                ));
            }
        };
        if order <= previous_order {
            return Err(element_diagnostic(
                child,
                "clef elements are duplicated or out of order".to_owned(),
            ));
        }
        previous_order = order;
        match child.name.as_str() {
            "sign" if matches!(value, "G" | "F" | "C" | "percussion" | "tab" | "none") => {
                sign = Some(value.to_owned());
            }
            "line"
                if value
                    .parse::<u8>()
                    .ok()
                    .is_some_and(|line| (1..=5).contains(&line)) =>
            {
                line = value.parse::<u8>().ok();
            }
            "clef-octave-change" if value.parse::<i8>().is_ok() => {
                octave_change = value.parse::<i8>().unwrap();
            }
            "sign" | "line" | "clef-octave-change" => {
                return Err(element_diagnostic(
                    child,
                    format!("unsupported clef value {value:?}"),
                ));
            }
            _ => {
                return Err(element_diagnostic(
                    child,
                    format!("unsupported clef element <{}>", child.name),
                ));
            }
        }
    }
    Ok(InputClef {
        sign: sign.ok_or_else(|| element_diagnostic(element, "clef requires <sign>".to_owned()))?,
        line,
        octave_change,
    })
}

fn voice_state_index<T: Default>(states: &mut VoiceState<T>, voice_id: &VoiceId) -> usize {
    if let Some(index) = states
        .iter()
        .position(|(existing_voice_id, _)| existing_voice_id == voice_id)
    {
        return index;
    }
    states.push((voice_id.clone(), T::default()));
    states.len() - 1
}

fn validate_beam_group(
    note: &Element,
    event: &Event,
    active_beams: &mut [bool; 5],
) -> Result<(), String> {
    let mut current_levels = [false; 5];
    for beam in &event.beams {
        current_levels[usize::from(beam.level)] = true;
    }
    for level in 1..=4 {
        if active_beams[level] && !current_levels[level] {
            return Err(element_diagnostic(
                note,
                format!("beam level {level} group is interrupted"),
            ));
        }
    }
    for beam in &event.beams {
        let index = usize::from(beam.level);
        match beam.state {
            BeamState::Begin if active_beams[index] => {
                return Err(element_diagnostic(
                    note,
                    format!(
                        "beam level {} begins before the prior group ends",
                        beam.level
                    ),
                ));
            }
            BeamState::Begin => active_beams[index] = true,
            BeamState::Continue | BeamState::End if !active_beams[index] => {
                return Err(element_diagnostic(
                    note,
                    format!("beam level {} continues without a begin marker", beam.level),
                ));
            }
            BeamState::Continue => {}
            BeamState::End => active_beams[index] = false,
            BeamState::ForwardHook | BeamState::BackwardHook if active_beams[index] => {
                return Err(element_diagnostic(
                    note,
                    format!(
                        "beam hook at level {} interrupts an open beam group",
                        beam.level
                    ),
                ));
            }
            BeamState::ForwardHook | BeamState::BackwardHook => {}
        }
    }
    Ok(())
}

fn parse_cursor_movement(
    elements: &[Element],
    movement: usize,
    divisions: Option<u32>,
    input_staff_count: u32,
) -> Result<f64, String> {
    let element = &elements[movement];
    let divisions = divisions.ok_or_else(|| {
        element_diagnostic(
            element,
            "a positive <divisions> value is required before cursor movement".to_owned(),
        )
    })?;
    require_no_attributes(element)?;
    require_container_text(element)?;
    let duration = element.children.first().copied().ok_or_else(|| {
        element_diagnostic(element, "cursor movement requires <duration>".to_owned())
    })?;
    if elements[duration].name != "duration" {
        return Err(element_diagnostic(
            element,
            "cursor movement must begin with <duration>".to_owned(),
        ));
    }
    require_no_attributes(&elements[duration])?;
    let ticks = leaf_text(&elements[duration])?
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            element_diagnostic(
                &elements[duration],
                "cursor movement duration must be a positive integer".to_owned(),
            )
        })?;
    let mut saw_voice = false;
    let mut saw_staff = false;
    for child in element.children.iter().skip(1) {
        let child_element = &elements[*child];
        match child_element.name.as_str() {
            "voice" if element.name == "forward" && !saw_voice && !saw_staff => {
                require_no_attributes(child_element)?;
                if leaf_text(child_element)?.is_empty() {
                    return Err(element_diagnostic(
                        child_element,
                        "<voice> identifier must not be empty".to_owned(),
                    ));
                }
                saw_voice = true;
            }
            "staff" if element.name == "forward" && !saw_staff => {
                require_no_attributes(child_element)?;
                let number = leaf_text(child_element)?;
                parse_input_staff_index(child_element, number, input_staff_count)?;
                saw_staff = true;
            }
            _ => {
                return Err(element_diagnostic(
                    child_element,
                    "unsupported cursor movement content".to_owned(),
                ));
            }
        }
    }
    Ok(f64::from(ticks) * 16.0 / f64::from(divisions))
}

fn duration_dot_factor(dot_count: usize) -> (u32, u32) {
    match dot_count {
        0 => (1, 1),
        1 => (3, 2),
        2 => (7, 4),
        _ => unreachable!("dot count is validated before duration conversion"),
    }
}

fn duration_matches_type(
    duration_ticks: u32,
    divisions: u32,
    base_units: u32,
    dot_count: usize,
    ratio: Option<&TupletRatio>,
) -> bool {
    let (dot_numerator, dot_denominator) = duration_dot_factor(dot_count);
    let actual = ratio.map_or(1, |ratio| ratio.actual);
    let normal = ratio.map_or(1, |ratio| ratio.normal);
    u128::from(duration_ticks) * 16 * u128::from(actual) * u128::from(dot_denominator)
        == u128::from(divisions)
            * u128::from(base_units)
            * u128::from(dot_numerator)
            * u128::from(normal)
}

fn parse_time_modification(
    elements: &[Element],
    time_modification: usize,
) -> Result<TupletRatio, String> {
    let element = &elements[time_modification];
    require_no_attributes(element)?;
    require_container_text(element)?;
    if element.children.len() < 2
        || elements[element.children[0]].name != "actual-notes"
        || elements[element.children[1]].name != "normal-notes"
    {
        return Err(element_diagnostic(
            element,
            "<time-modification> requires <actual-notes> followed by <normal-notes>".to_owned(),
        ));
    }
    let parse_count = |index: usize, name: &str| -> Result<u32, String> {
        let child = &elements[element.children[index]];
        require_no_attributes(child)?;
        leaf_text(child)?
            .parse::<u32>()
            .ok()
            .filter(|count| *count > 0)
            .ok_or_else(|| {
                element_diagnostic(child, format!("<{name}> must be a positive integer"))
            })
    };
    let actual = parse_count(0, "actual-notes")?;
    let normal = parse_count(1, "normal-notes")?;
    let mut saw_normal_type = false;
    for child in element.children.iter().skip(2) {
        let child_element = &elements[*child];
        match child_element.name.as_str() {
            "normal-type" if !saw_normal_type => {
                require_no_attributes(child_element)?;
                let value = leaf_text(child_element)?;
                if !matches!(
                    value,
                    "whole" | "half" | "quarter" | "eighth" | "16th" | "32nd" | "64th"
                ) {
                    return Err(element_diagnostic(
                        child_element,
                        format!("unsupported tuplet normal type {value:?}"),
                    ));
                }
                saw_normal_type = true;
            }
            "normal-dot" => {
                require_no_attributes(child_element)?;
                require_container_text(child_element)?;
                if !child_element.children.is_empty() {
                    return Err(element_diagnostic(
                        child_element,
                        "<normal-dot> must be empty".to_owned(),
                    ));
                }
            }
            _ => {
                return Err(element_diagnostic(
                    child_element,
                    "unsupported <time-modification> content".to_owned(),
                ));
            }
        }
    }
    Ok(TupletRatio { actual, normal })
}

fn parse_tuplet_notations(
    elements: &[Element],
    notations: usize,
) -> Result<Option<TupletBoundary>, String> {
    let element = &elements[notations];
    require_no_attributes(element)?;
    require_container_text(element)?;
    if element.children.is_empty() {
        return Err(element_diagnostic(
            element,
            "<notations> must contain a supported <tuplet> boundary".to_owned(),
        ));
    }
    if element.children.len() != 1 || elements[element.children[0]].name != "tuplet" {
        return Err(element_diagnostic(
            element,
            "only one <tuplet> boundary per note is supported".to_owned(),
        ));
    }
    let tuplet = element.children[0];
    let tuplet_element = &elements[tuplet];
    validate_attributes(
        tuplet_element,
        &[
            "type",
            "number",
            "bracket",
            "show-number",
            "show-type",
            "line-shape",
            "placement",
        ],
    )?;
    require_container_text(tuplet_element)?;
    let number = optional_attribute(tuplet_element, "number").unwrap_or("1");
    if number
        .parse::<u32>()
        .ok()
        .filter(|number| *number > 0)
        .is_none()
    {
        return Err(element_diagnostic(
            tuplet_element,
            "tuplet number must be a positive integer".to_owned(),
        ));
    }
    let mut saw_actual = false;
    let mut saw_normal = false;
    for display_group in &tuplet_element.children {
        let group = &elements[*display_group];
        match group.name.as_str() {
            "tuplet-actual" if !saw_actual => saw_actual = true,
            "tuplet-normal" if !saw_normal => saw_normal = true,
            _ => {
                return Err(element_diagnostic(
                    group,
                    "unsupported or duplicate tuplet display group".to_owned(),
                ));
            }
        }
        require_no_attributes(group)?;
        require_container_text(group)?;
        let mut saw_number = false;
        let mut saw_type = false;
        for display_value in &group.children {
            let value = &elements[*display_value];
            match value.name.as_str() {
                "tuplet-number" if !saw_number && !saw_type => {
                    require_no_attributes(value)?;
                    leaf_text(value)?;
                    saw_number = true;
                }
                "tuplet-type" if !saw_type => {
                    require_no_attributes(value)?;
                    let note_type = leaf_text(value)?;
                    if !matches!(
                        note_type,
                        "whole" | "half" | "quarter" | "eighth" | "16th" | "32nd" | "64th"
                    ) {
                        return Err(element_diagnostic(
                            value,
                            format!("unsupported tuplet display type {note_type:?}"),
                        ));
                    }
                    saw_type = true;
                }
                "tuplet-dot" if saw_type => {
                    require_no_attributes(value)?;
                    require_container_text(value)?;
                    if !value.children.is_empty() {
                        return Err(element_diagnostic(
                            value,
                            "<tuplet-dot> must be empty".to_owned(),
                        ));
                    }
                }
                _ => {
                    return Err(element_diagnostic(
                        value,
                        "unsupported tuplet display value".to_owned(),
                    ));
                }
            }
        }
    }
    let tuplet_type = optional_attribute(tuplet_element, "type").ok_or_else(|| {
        element_diagnostic(
            tuplet_element,
            "<tuplet> requires a type attribute".to_owned(),
        )
    })?;
    match tuplet_type {
        "start" => Ok(Some(TupletBoundary::Start(number.to_owned()))),
        "stop" => Ok(Some(TupletBoundary::Stop(number.to_owned()))),
        value => Err(element_diagnostic(
            tuplet_element,
            format!("tuplet type must be 'start' or 'stop', not {value:?}"),
        )),
    }
}

fn parse_note(
    elements: &[Element],
    note: usize,
    divisions: u32,
    input_staff_states: &[InputStaffState],
    input_staff_count: u32,
    active_tuplets: &VoiceState<Option<String>>,
    warnings: &mut Vec<String>,
) -> Result<ParsedNote, String> {
    let note_element = &elements[note];
    require_no_attributes(note_element)?;
    require_container_text(note_element)?;
    let children = &note_element.children;
    let chord_marker = children
        .first()
        .filter(|child| elements[**child].name == "chord")
        .copied();
    if let Some(chord) = chord_marker {
        require_no_attributes(&elements[chord])?;
        require_container_text(&elements[chord])?;
        if !elements[chord].children.is_empty() {
            return Err(element_diagnostic(
                &elements[chord],
                "<chord/> must be empty".to_owned(),
            ));
        }
    } else if let Some(chord) = children
        .iter()
        .find(|child| elements[**child].name == "chord")
    {
        return Err(element_diagnostic(
            &elements[*chord],
            "<chord/> must precede <pitch>".to_owned(),
        ));
    }
    let pitch_position = usize::from(chord_marker.is_some());
    let event_element = children.get(pitch_position).copied().ok_or_else(|| {
        element_diagnostic(note_element, "unsupported empty note event".to_owned())
    })?;
    let is_rest = elements[event_element].name == "rest";
    if elements[event_element].name != "pitch" && !is_rest {
        return Err(element_diagnostic(
            note_element,
            "unsupported note shape; expected <pitch> or <rest> first".to_owned(),
        ));
    }
    if is_rest {
        if let Some(chord) = chord_marker {
            return Err(element_diagnostic(
                &elements[chord],
                "rest chords are unsupported".to_owned(),
            ));
        }
    }
    let mut position = pitch_position + 1;
    let duration = *children.get(position).ok_or_else(|| {
        element_diagnostic(note_element, "note event requires <duration>".to_owned())
    })?;
    if elements[duration].name != "duration" {
        return Err(element_diagnostic(
            &elements[duration],
            "expected <duration> after <pitch> or <rest>".to_owned(),
        ));
    }
    position += 1;
    let mut tie_start = false;
    let mut tie_stop = false;
    while children
        .get(position)
        .is_some_and(|child| elements[*child].name == "tie")
    {
        let tie = children[position];
        let tie_type = required_attribute(&elements[tie], "type")?;
        require_container_text(&elements[tie])?;
        if !elements[tie].children.is_empty() {
            return Err(element_diagnostic(
                &elements[tie],
                "<tie> must be empty".to_owned(),
            ));
        }
        match tie_type {
            "start" if !tie_start => tie_start = true,
            "stop" if !tie_stop => tie_stop = true,
            _ => {
                return Err(element_diagnostic(
                    &elements[tie],
                    "tie type must be one unique 'start' and/or 'stop'".to_owned(),
                ));
            }
        }
        position += 1;
    }
    let voice_id = VoiceId(
        children
            .get(position)
            .filter(|child| elements[**child].name == "voice")
            .map(|voice| {
                position += 1;
                leaf_text(&elements[*voice]).map(|value| value.to_owned())
            })
            .transpose()?,
    );
    let note_type = *children
        .get(position)
        .ok_or_else(|| element_diagnostic(note_element, "note event requires <type>".to_owned()))?;
    position += 1;
    let mut dot_count = 0;
    while children
        .get(position)
        .is_some_and(|child| elements[*child].name == "dot")
    {
        let dot = children[position];
        if !elements[dot].attributes.is_empty() || !elements[dot].children.is_empty() {
            return Err(element_diagnostic(
                &elements[dot],
                "<dot> must be empty and have no attributes".to_owned(),
            ));
        }
        position += 1;
        dot_count += 1;
    }
    let accidental = children
        .get(position)
        .filter(|child| elements[**child].name == "accidental")
        .copied();
    if accidental.is_some() {
        position += 1;
    }
    let time_modification = children
        .get(position)
        .filter(|child| elements[**child].name == "time-modification")
        .copied()
        .map(|time_modification| {
            position += 1;
            parse_time_modification(elements, time_modification)
        })
        .transpose()?;
    let stem = children
        .get(position)
        .filter(|child| elements[**child].name == "stem")
        .copied();
    if stem.is_some() {
        position += 1;
    }
    let notehead = children
        .get(position)
        .filter(|child| elements[**child].name == "notehead")
        .copied();
    if notehead.is_some() {
        position += 1;
    }
    let input_staff_element = children
        .get(position)
        .filter(|child| elements[**child].name == "staff")
        .copied();
    let input_staff_index = if let Some(staff) = input_staff_element {
        position += 1;
        require_no_attributes(&elements[staff])?;
        let value = leaf_text(&elements[staff])?;
        parse_input_staff_index(&elements[staff], value, input_staff_count)?
    } else {
        0
    };
    let input_staff = input_staff_index as u32 + 1;
    let transposition = input_staff_states[input_staff_index].transposition;
    let mut beams = Vec::new();
    while children
        .get(position)
        .is_some_and(|child| elements[*child].name == "beam")
    {
        let beam = children[position];
        let level = match elements[beam].attributes.as_slice() {
            [] => 1,
            [(name, value)] if name == "number" => value
                .parse::<u8>()
                .ok()
                .filter(|level| (1..=4).contains(level))
                .ok_or_else(|| {
                    element_diagnostic(
                        &elements[beam],
                        "beam number must be 1 through 4".to_owned(),
                    )
                })?,
            _ => {
                return Err(element_diagnostic(
                    &elements[beam],
                    "<beam> supports only the optional number attribute".to_owned(),
                ));
            }
        };
        if !elements[beam].children.is_empty() {
            return Err(element_diagnostic(
                &elements[beam],
                "<beam> must be text only".to_owned(),
            ));
        }
        let state = match elements[beam].text.trim() {
            "begin" => BeamState::Begin,
            "continue" => BeamState::Continue,
            "end" => BeamState::End,
            "forward hook" => BeamState::ForwardHook,
            "backward hook" => BeamState::BackwardHook,
            value => {
                return Err(element_diagnostic(
                    &elements[beam],
                    format!("unsupported beam value {value:?}"),
                ));
            }
        };
        if beams
            .iter()
            .any(|existing: &BeamMark| existing.level == level)
        {
            return Err(element_diagnostic(
                &elements[beam],
                format!("duplicate beam level {level}"),
            ));
        }
        beams.push(BeamMark { level, state });
        position += 1;
    }
    let tuplet_boundary = children
        .get(position)
        .filter(|child| elements[**child].name == "notations")
        .copied()
        .map(|notations| {
            position += 1;
            parse_tuplet_notations(elements, notations)
        })
        .transpose()?
        .flatten();
    if position != children.len() {
        return Err(element_diagnostic(
            note_element,
            "unsupported note content after beam declarations".to_owned(),
        ));
    }
    if is_rest {
        require_no_attributes(&elements[event_element])?;
        require_container_text(&elements[event_element])?;
        if !elements[event_element].children.is_empty() {
            return Err(element_diagnostic(
                &elements[event_element],
                "unsupported rest shape".to_owned(),
            ));
        }
    }
    let (duration_name, base_units, mark_count) = match leaf_text(&elements[note_type])? {
        "whole" => ("whole", 64, 0),
        "half" => ("half", 32, 2),
        "quarter" => ("quarter", 16, 0),
        "eighth" => ("eighth", 8, 0),
        "16th" => ("16th", 4, 0),
        "32nd" => ("32nd", 2, 0),
        "64th" => ("64th", 1, 0),
        value => {
            return Err(element_diagnostic(
                &elements[note_type],
                format!("unsupported note type {value:?}"),
            ));
        }
    };
    if let Some(accidental) = accidental {
        if is_rest {
            return Err(element_diagnostic(
                &elements[accidental],
                "accidental display marks require a pitched event".to_owned(),
            ));
        }
        let pitch_alteration = elements[event_element]
            .children
            .iter()
            .find(|child| elements[**child].name == "alter")
            .map(|alter| elements[*alter].text.trim().parse::<i32>().unwrap_or(0))
            .unwrap_or(0);
        validate_accidental_display(elements, accidental, pitch_alteration, warnings)?;
    }
    if let Some(stem) = stem {
        validate_explicit_stem(elements, stem, warnings)?;
    }
    if let Some(notehead) = notehead {
        if is_rest {
            return Err(element_diagnostic(
                &elements[notehead],
                "pitched notehead styles are not supported on rests".to_owned(),
            ));
        }
        validate_pitched_notehead(elements, notehead, duration_name, warnings)?;
    }
    if dot_count > 2 {
        return Err(element_diagnostic(
            note_element,
            "at most two augmentation dots are supported".to_owned(),
        ));
    }
    if dot_count > 0 && !matches!(duration_name, "quarter" | "half") {
        return Err(element_diagnostic(
            &elements[note_type],
            "dots are supported only on quarter and half notes and Rests".to_owned(),
        ));
    }
    if is_rest && !beams.is_empty() {
        return Err(element_diagnostic(
            note_element,
            "beams on rests are unsupported".to_owned(),
        ));
    }
    if is_rest && (tie_start || tie_stop) {
        return Err(element_diagnostic(
            note_element,
            "ties may connect only pitched events".to_owned(),
        ));
    }
    let maximum_beam_level = match duration_name {
        "eighth" => 1,
        "16th" => 2,
        "32nd" => 3,
        "64th" => 4,
        _ => 0,
    };
    if let Some(beam) = beams.iter().find(|beam| beam.level > maximum_beam_level) {
        return Err(element_diagnostic(
            &elements[note_type],
            format!(
                "beam level {} is not supported by {} notes",
                beam.level, duration_name
            ),
        ));
    }
    let duration_ticks = leaf_text(&elements[duration])?
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            element_diagnostic(
                &elements[duration],
                "<duration> must be a positive integer".to_owned(),
            )
        })?;
    let display_only_tuplet = time_modification.is_none()
        && (active_tuplets
            .iter()
            .any(|(active_voice, number)| active_voice == &voice_id && number.is_some())
            || matches!(tuplet_boundary, Some(TupletBoundary::Start(_))));
    if !display_only_tuplet
        && !duration_matches_type(
            duration_ticks,
            divisions,
            base_units,
            dot_count,
            time_modification.as_ref(),
        )
    {
        return Err(element_diagnostic(
            &elements[duration],
            format!("duration {duration_ticks} does not match <type> at divisions {divisions}"),
        ));
    }
    let (dot_numerator, dot_denominator) = duration_dot_factor(dot_count);
    let duration_units = if display_only_tuplet {
        f64::from(duration_ticks) * 16.0 / f64::from(divisions)
    } else {
        f64::from(base_units) * f64::from(dot_numerator) / f64::from(dot_denominator)
            * time_modification.as_ref().map_or(1.0, |ratio| {
                f64::from(ratio.normal) / f64::from(ratio.actual)
            })
    };
    let duration_marks = if is_rest {
        dot_count as u8
    } else {
        match duration_name {
            "quarter" => dot_count as u8,
            "half" => mark_count + dot_count as u8,
            _ => mark_count,
        }
    };
    let tones = if is_rest {
        Vec::new()
    } else {
        vec![ToneEvent {
            tone: parse_pitch(elements, event_element, transposition)?,
            tie_start,
            tie_stop,
            input_staff,
        }]
    };
    Ok(ParsedNote {
        event: Event {
            tones,
            duration_units,
            onset_units: 0.0,
            duration_name,
            dots: duration_marks,
            beams,
            voice_id,
            input_staff,
            is_spacer: false,
        },
        chord_marker,
        tuplet_boundary,
    })
}

fn validate_explicit_stem(
    elements: &[Element],
    stem: usize,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let element = &elements[stem];
    require_no_attributes(element)?;
    let value = leaf_text(element)?;
    match value {
        "up" | "down" => warnings.push(element_diagnostic(
            element,
            "explicit stem direction is not supported; inferred direction is used".to_owned(),
        )),
        "double" | "none" => {
            return Err(element_diagnostic(
                element,
                format!("stem direction {value:?} is not supported"),
            ));
        }
        _ => {
            return Err(element_diagnostic(
                element,
                format!("invalid stem direction {value:?}"),
            ));
        }
    }
    Ok(())
}

fn validate_pitched_notehead(
    elements: &[Element],
    notehead: usize,
    duration_name: &str,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    const NOTEHEAD_VALUES: &[&str] = &[
        "slash",
        "triangle",
        "diamond",
        "square",
        "cross",
        "x",
        "circle-x",
        "inverted triangle",
        "arrow down",
        "arrow up",
        "circled",
        "slashed",
        "back slashed",
        "normal",
        "cluster",
        "circle dot",
        "left triangle",
        "rectangle",
        "none",
        "do",
        "re",
        "mi",
        "fa",
        "fa up",
        "so",
        "la",
        "ti",
        "other",
    ];
    let element = &elements[notehead];
    validate_attributes(
        element,
        &[
            "filled",
            "parentheses",
            "font-family",
            "font-style",
            "font-size",
            "font-weight",
            "color",
            "smufl",
        ],
    )?;
    if !element.children.is_empty() {
        return Err(element_diagnostic(
            element,
            "notehead must contain plain text".to_owned(),
        ));
    }
    let value = element.text.trim();
    if value.is_empty() || !NOTEHEAD_VALUES.contains(&value) {
        return Err(element_diagnostic(
            element,
            format!("invalid notehead value {value:?}"),
        ));
    }

    let mut has_unsupported_presentation = value != "normal";
    let default_filled = !matches!(duration_name, "whole" | "half");
    for (attribute, default) in [
        ("filled", if default_filled { "yes" } else { "no" }),
        ("parentheses", "no"),
    ] {
        if let Some(value) = optional_attribute(element, attribute) {
            if !matches!(value, "yes" | "no") {
                return Err(element_diagnostic(
                    element,
                    format!("{attribute} must be 'yes' or 'no', not {value:?}"),
                ));
            }
            has_unsupported_presentation |= value != default;
        }
    }
    for (attribute, allowed, default) in [
        ("font-style", &["normal", "italic"][..], "normal"),
        ("font-weight", &["normal", "bold"][..], "normal"),
    ] {
        if let Some(value) = optional_attribute(element, attribute) {
            if !allowed.contains(&value) {
                return Err(element_diagnostic(
                    element,
                    format!("invalid notehead {attribute} value {value:?}"),
                ));
            }
            has_unsupported_presentation |= value != default;
        }
    }
    if let Some(font_size) = optional_attribute(element, "font-size") {
        if !valid_musicxml_font_size(font_size) {
            return Err(element_diagnostic(
                element,
                format!("invalid notehead font-size {font_size:?}"),
            ));
        }
        has_unsupported_presentation |= font_size != "medium";
    }
    if optional_attribute(element, "font-family").is_some_and(|family| !family.is_empty()) {
        has_unsupported_presentation = true;
    }
    if let Some(color) = optional_attribute(element, "color") {
        if !valid_musicxml_color(color) {
            return Err(element_diagnostic(
                element,
                "notehead color must be a six-digit hexadecimal color".to_owned(),
            ));
        }
        has_unsupported_presentation |= !color.eq_ignore_ascii_case("#000000");
    }
    if let Some(smufl) = optional_attribute(element, "smufl") {
        if !valid_smufl_glyph_name(smufl) {
            return Err(element_diagnostic(
                element,
                format!("invalid notehead SMuFL glyph name {smufl:?}"),
            ));
        }
        has_unsupported_presentation = true;
    }
    if has_unsupported_presentation {
        warnings.push(element_diagnostic(
            element,
            "notehead presentation is not supported; numeric Tonehead is retained".to_owned(),
        ));
    }
    Ok(())
}

fn validate_accidental_display(
    elements: &[Element],
    accidental: usize,
    pitch_alteration: i32,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    const ACCIDENTAL_VALUES: &[&str] = &[
        "sharp",
        "natural",
        "flat",
        "double-sharp",
        "sharp-sharp",
        "flat-flat",
        "natural-sharp",
        "natural-flat",
        "quarter-flat",
        "quarter-sharp",
        "three-quarters-flat",
        "three-quarters-sharp",
        "sharp-down",
        "sharp-up",
        "natural-down",
        "natural-up",
        "flat-down",
        "flat-up",
        "double-sharp-down",
        "double-sharp-up",
        "flat-flat-down",
        "flat-flat-up",
        "arrow-down",
        "arrow-up",
        "triple-sharp",
        "triple-flat",
        "slash-quarter-sharp",
        "slash-sharp",
        "slash-flat",
        "double-slash-flat",
        "sharp-1",
        "sharp-2",
        "sharp-3",
        "sharp-5",
        "flat-1",
        "flat-2",
        "flat-3",
        "flat-4",
        "sori",
        "koron",
        "other",
    ];
    let element = &elements[accidental];
    validate_attributes(
        element,
        &[
            "cautionary",
            "editorial",
            "parentheses",
            "bracket",
            "size",
            "default-x",
            "default-y",
            "relative-x",
            "relative-y",
            "font-family",
            "font-style",
            "font-size",
            "font-weight",
            "color",
            "smufl",
        ],
    )?;
    if !element.children.is_empty() {
        return Err(element_diagnostic(
            element,
            "accidental must contain plain text".to_owned(),
        ));
    }
    let value = element.text.trim();
    if !ACCIDENTAL_VALUES.contains(&value) {
        return Err(element_diagnostic(
            element,
            format!("invalid accidental value {value:?}"),
        ));
    }
    for attribute in ["cautionary", "editorial", "parentheses", "bracket"] {
        if let Some(value) = optional_attribute(element, attribute) {
            if !matches!(value, "yes" | "no") {
                return Err(element_diagnostic(
                    element,
                    format!("{attribute} must be 'yes' or 'no', not {value:?}"),
                ));
            }
        }
    }
    if let Some(size) = optional_attribute(element, "size") {
        if !matches!(size, "full" | "cue" | "grace-cue" | "large") {
            return Err(element_diagnostic(
                element,
                format!("invalid accidental size {size:?}"),
            ));
        }
    }
    for attribute in ["default-x", "default-y", "relative-x", "relative-y"] {
        if let Some(value) = optional_attribute(element, attribute) {
            if !value.parse::<f64>().is_ok_and(f64::is_finite) {
                return Err(element_diagnostic(
                    element,
                    format!("accidental {attribute} must be a finite number"),
                ));
            }
        }
    }
    for (attribute, allowed) in [
        ("font-style", &["normal", "italic"][..]),
        ("font-weight", &["normal", "bold"][..]),
    ] {
        if let Some(value) = optional_attribute(element, attribute) {
            if !allowed.contains(&value) {
                return Err(element_diagnostic(
                    element,
                    format!("invalid accidental {attribute} value {value:?}"),
                ));
            }
        }
    }
    if optional_attribute(element, "font-size").is_some_and(|size| !valid_musicxml_font_size(size))
    {
        return Err(element_diagnostic(
            element,
            "invalid accidental font-size".to_owned(),
        ));
    }
    if optional_attribute(element, "color").is_some_and(|color| !valid_musicxml_color(color)) {
        return Err(element_diagnostic(
            element,
            "accidental color must be a six-digit hexadecimal color".to_owned(),
        ));
    }
    if optional_attribute(element, "smufl")
        .is_some_and(|glyph_name| !valid_smufl_accidental_glyph_name(glyph_name))
    {
        return Err(element_diagnostic(
            element,
            "invalid accidental SMuFL glyph name".to_owned(),
        ));
    }
    let disagreement =
        accidental_semitones(value).is_some_and(|alteration| alteration != pitch_alteration);
    let warning = if disagreement {
        "accidental display mark disagrees with the encoded pitch; pitch alteration remains authoritative"
    } else {
        "accidental display mark is not supported; omitted"
    };
    warnings.push(element_diagnostic(element, warning.to_owned()));
    Ok(())
}

fn accidental_semitones(value: &str) -> Option<i32> {
    match value {
        "sharp" | "natural-sharp" => Some(1),
        "natural" => Some(0),
        "flat" | "natural-flat" => Some(-1),
        "double-sharp" | "sharp-sharp" => Some(2),
        "flat-flat" => Some(-2),
        "triple-sharp" => Some(3),
        "triple-flat" => Some(-3),
        _ => None,
    }
}

fn valid_musicxml_font_size(value: &str) -> bool {
    matches!(
        value,
        "xx-small" | "x-small" | "small" | "medium" | "large" | "x-large" | "xx-large"
    ) || (!value.contains(['e', 'E']) && value.parse::<f64>().is_ok_and(f64::is_finite))
}

fn valid_smufl_glyph_name(value: &str) -> bool {
    !value.is_empty() && value.chars().all(is_smufl_name_character)
}

fn valid_smufl_accidental_glyph_name(value: &str) -> bool {
    [
        "acc",
        "medRenFla",
        "medRenNatura",
        "medRenShar",
        "kievanAccidental",
    ]
    .iter()
    .any(|prefix| {
        value
            .strip_prefix(prefix)
            .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(is_smufl_name_character))
    })
}

fn is_smufl_name_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | ':')
}

fn same_beams(left: &[BeamMark], right: &[BeamMark]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .all(|beam| right.iter().any(|candidate| candidate == beam))
}

fn validate_event_ties(
    note: &Element,
    event: &Event,
    pending_ties: &mut VoiceState<Vec<Tone>>,
) -> Result<(), String> {
    let voice_index = voice_state_index(pending_ties, &event.voice_id);
    let mut unmatched = std::mem::take(&mut pending_ties[voice_index].1);
    for tone in event.tones.iter().filter(|tone| tone.tie_stop) {
        let Some(position) = unmatched.iter().position(|expected| expected == &tone.tone) else {
            let message = if unmatched.is_empty() {
                "tie stop has no preceding tie start"
            } else {
                "tie stop must match a preceding concert pitch"
            };
            return Err(element_diagnostic(note, message.to_owned()));
        };
        unmatched.remove(position);
    }
    if !unmatched.is_empty() {
        return Err(element_diagnostic(
            note,
            "tie start must connect to the next equal concert pitch".to_owned(),
        ));
    }
    pending_ties[voice_index].1.extend(
        event
            .tones
            .iter()
            .filter(|tone| tone.tie_start)
            .map(|tone| tone.tone),
    );
    Ok(())
}

fn parse_pitch(elements: &[Element], pitch: usize, transposition: i32) -> Result<Tone, String> {
    require_no_attributes(&elements[pitch])?;
    require_container_text(&elements[pitch])?;
    let pitch_children = &elements[pitch].children;
    let (step_id, alter_id, octave_id) = match pitch_children.as_slice() {
        [step_id, octave_id]
            if elements[*step_id].name == "step" && elements[*octave_id].name == "octave" =>
        {
            (*step_id, None, *octave_id)
        }
        [step_id, alter_id, octave_id]
            if elements[*step_id].name == "step"
                && elements[*alter_id].name == "alter"
                && elements[*octave_id].name == "octave" =>
        {
            (*step_id, Some(*alter_id), *octave_id)
        }
        _ => {
            return Err(element_diagnostic(
                &elements[pitch],
                "unsupported shape: <pitch> must contain <step>, optional natural <alter>, and <octave>"
                    .to_owned(),
            ))
        }
    };
    let step = leaf_text(&elements[step_id])?;
    let pitch_class = match step {
        "C" => 0,
        "D" => 2,
        "E" => 4,
        "F" => 5,
        "G" => 7,
        "A" => 9,
        "B" => 11,
        _ => {
            return Err(element_diagnostic(
                &elements[step_id],
                format!("unsupported pitch step {step:?}"),
            ));
        }
    };
    let alter = alter_id
        .map(|alter| {
            let value = leaf_text(&elements[alter])?.parse::<i32>().map_err(|_| {
                element_diagnostic(
                    &elements[alter],
                    "unsupported pitch alteration; expected an integer from -2 through 2"
                        .to_owned(),
                )
            })?;
            if !(-2..=2).contains(&value) {
                return Err(element_diagnostic(
                    &elements[alter],
                    "unsupported pitch alteration; expected an integer from -2 through 2"
                        .to_owned(),
                ));
            }
            Ok(value)
        })
        .transpose()?
        .unwrap_or(0);
    let written_octave = leaf_text(&elements[octave_id])?
        .parse::<i32>()
        .map_err(|_| {
            element_diagnostic(
                &elements[octave_id],
                "unsupported pitch octave; expected an integer".to_owned(),
            )
        })?;
    let absolute_pitch = written_octave
        .checked_mul(12)
        .and_then(|value| value.checked_add(pitch_class))
        .and_then(|value| value.checked_add(alter))
        .and_then(|value| value.checked_add(transposition))
        .ok_or_else(|| {
            element_diagnostic(
                &elements[pitch],
                "pitch is outside the supported numeric range".to_owned(),
            )
        })?;
    let octave = absolute_pitch.div_euclid(12);
    if !(0..=9).contains(&octave) {
        return Err(element_diagnostic(
            &elements[pitch],
            format!("unsupported concert pitch octave {octave}; expected 0 through 9"),
        ));
    }
    Ok(Tone {
        octave: octave as u8,
        value: absolute_pitch.rem_euclid(12) as u8,
    })
}

fn transpose_semitones(elements: &[Element], transpose: usize) -> Result<i32, String> {
    validate_attributes(&elements[transpose], &["number"])?;
    require_container_text(&elements[transpose])?;
    let children = &elements[transpose].children;
    let mut previous_order = 0;
    let mut chromatic = None;
    let mut octave_change = 0;
    let mut seen = Vec::new();
    for child in children {
        let element = &elements[*child];
        let order = match element.name.as_str() {
            "diatonic" => 1,
            "chromatic" => 2,
            "octave-change" => 3,
            "double" => 4,
            _ => {
                return Err(element_diagnostic(
                    element,
                    "unsupported transpose element".to_owned(),
                ))
            }
        };
        if order <= previous_order {
            return Err(element_diagnostic(
                element,
                "transpose elements are duplicated or out of order".to_owned(),
            ));
        }
        previous_order = order;
        seen.push(element.name.as_str());
        match element.name.as_str() {
            "diatonic" => {
                leaf_text(element)?.parse::<i32>().map_err(|_| {
                    element_diagnostic(
                        element,
                        "diatonic transposition must be an integer".to_owned(),
                    )
                })?;
            }
            "chromatic" => {
                chromatic = Some(leaf_text(element)?.parse::<i32>().map_err(|_| {
                    element_diagnostic(
                        element,
                        "chromatic transposition must be an integer number of semitones".to_owned(),
                    )
                })?);
            }
            "octave-change" => {
                octave_change = leaf_text(element)?.parse::<i32>().map_err(|_| {
                    element_diagnostic(element, "octave-change must be an integer".to_owned())
                })?;
            }
            "double" => {
                if !element.children.is_empty() || !element.text.trim().is_empty() {
                    return Err(element_diagnostic(
                        element,
                        "double transposition must be empty".to_owned(),
                    ));
                }
                let above = match element.attributes.as_slice() {
                    [] => false,
                    [(name, value)]
                        if name == "above" && matches!(value.as_str(), "yes" | "no") =>
                    {
                        value == "yes"
                    }
                    _ => {
                        return Err(element_diagnostic(
                            element,
                            "double transposition has an invalid above attribute".to_owned(),
                        ))
                    }
                };
                octave_change = octave_change
                    .checked_add(if above { 1 } else { -1 })
                    .ok_or_else(|| {
                        element_diagnostic(
                            element,
                            "octave transposition is out of range".to_owned(),
                        )
                    })?;
            }
            _ => unreachable!(),
        }
    }
    if chromatic.is_none() {
        return Err(element_diagnostic(
            &elements[transpose],
            "transpose requires a chromatic value".to_owned(),
        ));
    }
    let _ = seen;
    chromatic
        .unwrap()
        .checked_add(octave_change.checked_mul(12).ok_or_else(|| {
            element_diagnostic(
                &elements[transpose],
                "octave transposition is out of range".to_owned(),
            )
        })?)
        .ok_or_else(|| {
            element_diagnostic(
                &elements[transpose],
                "transposition is out of range".to_owned(),
            )
        })
}

fn element_diagnostic(element: &Element, message: String) -> String {
    format!(
        "{message} (byte {}, line {}, column {}, XML path {})",
        element.byte_offset, element.line, element.column, element.path
    )
}

fn collect_voice_errors(elements: &[Element]) -> Vec<String> {
    let mut errors = Vec::new();
    for element in elements.iter().filter(|element| element.name == "voice") {
        if !element.children.is_empty() || !element.attributes.is_empty() {
            continue;
        }
        let value = element.text.trim();
        if value.is_empty() {
            errors.push(element_diagnostic(
                element,
                "unsupported score: <voice> identifier must not be empty".to_owned(),
            ));
        }
    }
    for part in elements.iter().filter(|element| element.name == "part") {
        let mut voice_ids = Vec::new();
        let mut notes_without_voice = Vec::new();
        for measure in &part.children {
            if elements[*measure].name != "measure" {
                continue;
            }
            for note in &elements[*measure].children {
                if elements[*note].name != "note" {
                    continue;
                }
                let voice = elements[*note]
                    .children
                    .iter()
                    .find(|child| elements[**child].name == "voice");
                if let Some(voice) = voice {
                    let value = elements[*voice].text.trim();
                    if !value.is_empty() && !voice_ids.contains(&value) {
                        voice_ids.push(value);
                    }
                } else {
                    notes_without_voice.push(*note);
                }
            }
        }
        if voice_ids.len() > 1 {
            for note in notes_without_voice {
                errors.push(element_diagnostic(
                    &elements[note],
                    "multiple Voices require an explicit <voice> identifier on every note"
                        .to_owned(),
                ));
            }
        }
    }
    errors
}

fn collect_value_errors(elements: &[Element]) -> Vec<String> {
    let mut errors = Vec::new();
    for element in elements {
        if !element.children.is_empty() || !element.attributes.is_empty() {
            continue;
        }
        let value = element.text.trim();
        let message = match element.name.as_str() {
            "alter"
                if value
                    .parse::<i32>()
                    .map_or(true, |alter| !(-2..=2).contains(&alter)) =>
            {
                Some("unsupported pitch alteration; expected an integer from -2 through 2")
            }
            "step" if !matches!(value, "A" | "B" | "C" | "D" | "E" | "F" | "G") => {
                errors.push(element_diagnostic(
                    element,
                    format!("unsupported pitch step {value:?}"),
                ));
                None
            }
            "octave" if value.parse::<i32>().is_err() => Some("pitch octave must be an integer"),
            _ => None,
        };
        if let Some(message) = message {
            errors.push(element_diagnostic(element, message.to_owned()));
        }
    }
    errors
}

fn collect_duration_errors(elements: &[Element]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut active_divisions = None;
    for element in elements {
        if element.name == "divisions" && element.children.is_empty() {
            active_divisions = element
                .text
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|value| *value > 0);
        }
        if element.name != "note" {
            continue;
        }
        let Some(divisions) = active_divisions else {
            continue;
        };
        let duration = element
            .children
            .iter()
            .find(|child| elements[**child].name == "duration")
            .copied();
        let note_type = element
            .children
            .iter()
            .find(|child| elements[**child].name == "type")
            .copied();
        let (Some(duration), Some(note_type)) = (duration, note_type) else {
            continue;
        };
        let base_units = match elements[note_type].text.trim() {
            "whole" => 64u64,
            "half" => 32,
            "quarter" => 16,
            "eighth" => 8,
            "16th" => 4,
            "32nd" => 2,
            "64th" => 1,
            _ => continue,
        };
        let dots = element
            .children
            .iter()
            .filter(|child| elements[**child].name == "dot")
            .count();
        if dots > 0
            || element.children.iter().any(|child| {
                matches!(
                    elements[*child].name.as_str(),
                    "time-modification" | "notations"
                )
            })
        {
            continue;
        }
        let duration_ticks = elements[duration].text.trim().parse::<u32>().ok();
        let Some(duration_ticks) = duration_ticks.filter(|value| *value > 0) else {
            continue;
        };
        if u64::from(duration_ticks) * 16 != u64::from(divisions) * base_units {
            errors.push(element_diagnostic(
                &elements[duration],
                format!("duration {duration_ticks} does not match <type> at divisions {divisions}"),
            ));
        }
    }
    errors
}

fn root_element(elements: &[Element]) -> Result<usize, String> {
    let mut is_child = vec![false; elements.len()];
    for element in elements {
        for child in &element.children {
            is_child[*child] = true;
        }
    }
    let roots = is_child
        .iter()
        .enumerate()
        .filter_map(|(index, child)| (!child).then_some(index))
        .collect::<Vec<_>>();
    if roots.len() != 1 {
        return Err("malformed XML: expected one document element".to_owned());
    }
    Ok(roots[0])
}

fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a str, String> {
    if element.attributes.len() != 1 || element.attributes[0].0 != name {
        return Err(element_diagnostic(
            element,
            format!(
                "unsupported shape: <{}> requires only the {name:?} attribute",
                element.name
            ),
        ));
    }
    Ok(&element.attributes[0].1)
}

fn require_attribute(element: &Element, name: &str, expected: &str) -> Result<(), String> {
    if required_attribute(element, name)? != expected {
        return Err(element_diagnostic(
            element,
            format!(
                "unsupported score: <{}> {name} must be {expected:?}",
                element.name
            ),
        ));
    }
    Ok(())
}

fn exact_children<const N: usize>(
    elements: &[Element],
    parent: usize,
    expected: &[&str],
) -> Result<[usize; N], String> {
    let children = &elements[parent].children;
    if children.len() != N
        || children
            .iter()
            .zip(expected)
            .any(|(child, name)| elements[*child].name != *name)
    {
        return Err(element_diagnostic(
            &elements[parent],
            format!(
                "unsupported shape: <{}> must contain {}",
                elements[parent].name,
                expected.join(", ")
            ),
        ));
    }
    Ok(std::array::from_fn(|index| children[index]))
}

fn require_container_text(element: &Element) -> Result<(), String> {
    if element.text.trim().is_empty() {
        Ok(())
    } else {
        Err(element_diagnostic(
            element,
            format!(
                "unsupported shape: unexpected text inside <{}>",
                element.name
            ),
        ))
    }
}

fn require_no_attributes(element: &Element) -> Result<(), String> {
    if element.attributes.is_empty() {
        Ok(())
    } else {
        Err(element_diagnostic(
            element,
            format!(
                "unsupported shape: attributes on <{}> are not supported",
                element.name
            ),
        ))
    }
}

fn validate_attributes(element: &Element, allowed: &[&str]) -> Result<(), String> {
    for (index, (name, _)) in element.attributes.iter().enumerate() {
        if !allowed.contains(&name.as_str()) {
            return Err(element_diagnostic(
                element,
                format!("unsupported attribute {name:?} on <{}>", element.name),
            ));
        }
        if element.attributes[..index]
            .iter()
            .any(|(previous, _)| previous == name)
        {
            return Err(element_diagnostic(
                element,
                format!("duplicate {name:?} attribute on <{}>", element.name),
            ));
        }
    }
    Ok(())
}

fn optional_attribute<'a>(element: &'a Element, name: &str) -> Option<&'a str> {
    element
        .attributes
        .iter()
        .find_map(|(attribute, value)| (attribute == name).then_some(value.as_str()))
}

fn valid_musicxml_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn leaf_text(element: &Element) -> Result<&str, String> {
    if !element.children.is_empty() || !element.attributes.is_empty() {
        return Err(element_diagnostic(
            element,
            format!(
                "unsupported shape: <{}> must be a plain text element",
                element.name
            ),
        ));
    }
    let text = element.text.trim();
    if text.is_empty() {
        return Err(element_diagnostic(
            element,
            format!("unsupported score: <{}> may not be empty", element.name),
        ));
    }
    Ok(text)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DrawLayer {
    Staff,
    Barline,
    Beam,
    Event,
    Text,
    Tie,
}

#[derive(Clone, Copy)]
enum PathClass {
    Rest,
    Stem,
    Flag,
    Beam,
    BeamHook,
    DurationMark,
    ChordBracket,
    Tonehead,
    ChordSymbolRoot,
    ChordSymbolBass,
    ChordSymbolTone,
    TextGlyph,
    DirectionSymbol,
    SegnoMark,
    CodaMark,
    TextEnclosure,
    Tie,
    RepeatBarline,
    RepeatDot,
    EndingBracket,
    EndingLabel,
}

#[derive(Clone)]
enum PathMetadata {
    Event(usize),
    ToneIndex(usize),
    Tone(u8),
    Octave(u8),
    Duration(&'static str),
    ChordRole(&'static str),
    BeamLevel(u8),
    EndingLabel(String),
}

#[derive(Clone, Copy)]
enum PathStyle {
    StaffLine,
    MediumStroke,
    HeavyStroke,
    DottedStroke,
    DashedStroke,
    MediumOutline,
    FilledGlyph,
    BravuraGlyph,
    TextGlyph,
    TextEnclosure,
    Beam,
    Solid,
}

struct DrawCommand {
    layer: DrawLayer,
    id: String,
    class: Option<PathClass>,
    metadata: Vec<PathMetadata>,
    geometry: PathGeometry,
}

struct PathGeometry {
    transform: Option<(i32, i32)>,
    x_offset: f64,
    y_offset: f64,
    scale: f64,
    rotation: Option<(i32, i32, i32)>,
    path_data: String,
    style: PathStyle,
    color: Option<String>,
}

impl PathGeometry {
    fn new(transform: Option<(i32, i32)>, path_data: impl Into<String>, style: PathStyle) -> Self {
        Self {
            transform,
            x_offset: 0.0,
            y_offset: 0.0,
            scale: 1.0,
            rotation: None,
            path_data: path_data.into(),
            style,
            color: None,
        }
    }

    fn scaled(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }

    fn offset_x(mut self, offset: f64) -> Self {
        self.x_offset = offset;
        self
    }

    fn offset_y(mut self, offset: f64) -> Self {
        self.y_offset = offset;
        self
    }

    fn colored(mut self, color: &str) -> Self {
        self.color = Some(color.to_owned());
        self
    }

    fn rotated_180_if(self, condition: bool) -> Self {
        self.rotated_180_about_if(condition, 6, 9)
    }

    fn rotated_180_about_if(mut self, condition: bool, x: i32, y: i32) -> Self {
        if condition {
            self.rotation = Some((180, x, y));
        }
        self
    }
}

#[derive(Clone, Copy)]
struct ComponentBounds {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl ComponentBounds {
    fn stroked(left: f64, top: f64, right: f64, bottom: f64, stroke_width: f64) -> Self {
        let stroke_radius = stroke_width / 2.0;
        Self {
            left: left - stroke_radius,
            top: top - stroke_radius,
            right: right + stroke_radius,
            bottom: bottom + stroke_radius,
        }
    }

    fn filled(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    fn overlaps_vertically(self, other: Self) -> bool {
        self.top <= other.bottom && other.top <= self.bottom
    }
}

#[derive(Clone)]
struct EventGeometry {
    primary: Vec<ComponentBounds>,
    incoming: Vec<ComponentBounds>,
}

impl EventGeometry {
    fn all(&self) -> impl Iterator<Item = &ComponentBounds> {
        self.primary.iter().chain(&self.incoming)
    }
}

fn duration_mark_position(
    duration_name: &str,
    mark_count: u8,
    mark_index: u8,
    x: i32,
    lane_y: i32,
) -> (i32, i32) {
    match duration_name {
        "quarter" => (x + 14 + i32::from(mark_index) * 7, lane_y + 12),
        "half" if mark_index < 2 => (x + 14, lane_y + 5 + i32::from(mark_index) * 7),
        "half" => (x + 21 + i32::from(mark_index - 2) * 7, lane_y + 12),
        _ if mark_count > 0 => (x + 14, lane_y + 9),
        _ => unreachable!("event has no duration marks"),
    }
}

fn rest_duration_mark_position(mark_index: u8, x: i32, rest_y: i32) -> (i32, i32) {
    (x + 14 + i32::from(mark_index) * 7, rest_y + 9)
}

fn event_geometry(
    event: &Event,
    x: i32,
    active_beams: &[Option<(i32, i32)>; 5],
    pending_ties: &[(Tone, i32, i32)],
    stems_up: bool,
) -> EventGeometry {
    let mut primary = Vec::new();
    let mut incoming = Vec::new();
    if event.is_spacer {
        return EventGeometry { primary, incoming };
    }
    if event.tones.is_empty() {
        let glyph = bravura_glyphs::rest(event.duration_name);
        let rest_y = rest_origin_y(event.duration_name, glyph.bounds);
        primary.push(ComponentBounds::filled(
            f64::from(x) + glyph.bounds.0,
            rest_y + glyph.bounds.1,
            f64::from(x) + glyph.bounds.2,
            rest_y + glyph.bounds.3,
        ));
        for mark in 0..event.dots {
            let (mark_x, mark_y) = rest_duration_mark_position(mark, x, rest_y.floor() as i32);
            primary.push(ComponentBounds::filled(
                f64::from(mark_x),
                f64::from(mark_y) - 1.2,
                f64::from(mark_x) + 2.4,
                f64::from(mark_y) + 1.2,
            ));
        }
        return EventGeometry { primary, incoming };
    }

    let mut lane_offsets = [0i32; 10];
    let tone_xs = event
        .tones
        .iter()
        .map(|tone| {
            let octave = usize::from(tone.tone.octave);
            let tone_x = x + lane_offsets[octave];
            lane_offsets[octave] += 12;
            tone_x
        })
        .collect::<Vec<_>>();
    let leftmost_tone_x = *tone_xs.iter().min().expect("event has tones");
    let rightmost_tone_x = *tone_xs.iter().max().expect("event has tones");
    for (tone_index, tone_event) in event.tones.iter().enumerate() {
        let (left, right) = (1.0, 11.0);
        let tone_y = lane_y(tone_event.tone.octave);
        primary.push(ComponentBounds::stroked(
            f64::from(tone_xs[tone_index]) + left,
            f64::from(tone_y) + 2.0,
            f64::from(tone_xs[tone_index]) + right,
            f64::from(tone_y) + 16.0,
            1.8,
        ));
    }

    let stem_tone_index = stem_tone_index(event, stems_up);
    let stem_octave_y = lane_y(event.tones[stem_tone_index].tone.octave);
    let (stem_start, stem_end) = stem_endpoints(stem_octave_y, stems_up);
    let stem_x = tone_xs[stem_tone_index] + 6;
    if event.duration_name != "whole" {
        primary.push(ComponentBounds::stroked(
            f64::from(stem_x),
            f64::from(stem_end),
            f64::from(stem_x),
            f64::from(stem_start),
            1.5,
        ));
        let flag_levels = unbeamed_flag_levels(event);
        if !flag_levels.is_empty() {
            let glyph = bravura_glyphs::flag(flag_level_count(event.duration_name), stems_up);
            primary.push(ComponentBounds::filled(
                f64::from(stem_x) + glyph.bounds.0 - 0.75,
                f64::from(stem_end) + glyph.bounds.1,
                f64::from(stem_x) + glyph.bounds.2 - 0.75,
                f64::from(stem_end) + glyph.bounds.3,
            ));
        }
    }

    let beam_direction = if stems_up { 1 } else { -1 };
    let primary_beam_slope = active_beams[1]
        .map(|(start_x, start_y)| f64::from(stem_end - start_y) / f64::from(stem_x - start_x))
        .unwrap_or(0.0);
    for beam in &event.beams {
        let level = usize::from(beam.level);
        let offset = beam_level_offset(beam.level, primary_beam_slope, beam_direction);
        let beam_y = stem_end + offset;
        match beam.state {
            BeamState::Begin => {}
            BeamState::Continue | BeamState::End => {
                if let Some((start_x, start_y)) = active_beams[level] {
                    incoming.push(beam_segment_bounds(
                        start_x,
                        f64::from(start_y + offset),
                        stem_x,
                        f64::from(beam_y),
                        beam_direction,
                    ));
                }
            }
            BeamState::ForwardHook | BeamState::BackwardHook => {
                let direction = if beam.state == BeamState::ForwardHook {
                    1
                } else {
                    -1
                };
                let (start_x, start_y, end_x, end_y) = if direction > 0 {
                    (
                        stem_x,
                        f64::from(beam_y),
                        stem_x + 9,
                        f64::from(beam_y) + primary_beam_slope * 9.0,
                    )
                } else {
                    (
                        stem_x - 9,
                        f64::from(beam_y) - primary_beam_slope * 9.0,
                        stem_x,
                        f64::from(beam_y),
                    )
                };
                primary.push(beam_segment_bounds(
                    start_x,
                    start_y,
                    end_x,
                    end_y,
                    beam_direction,
                ));
            }
        }
    }

    let first_tone = &event.tones[0].tone;
    for mark in 0..event.dots {
        let (mark_x, mark_y) = duration_mark_position(
            event.duration_name,
            event.dots,
            mark,
            x,
            lane_y(first_tone.octave),
        );
        primary.push(ComponentBounds::filled(
            f64::from(mark_x),
            f64::from(mark_y) - 1.2,
            f64::from(mark_x) + 2.4,
            f64::from(mark_y) + 1.2,
        ));
    }

    if event.tones.iter().any(|tone| tone.tone.octave <= 1) {
        primary.push(ComponentBounds::stroked(
            f64::from(leftmost_tone_x - 8),
            148.0,
            f64::from(rightmost_tone_x + 20),
            148.0,
            1.0,
        ));
    }
    if event.tones.iter().any(|tone| tone.tone.octave >= 8) {
        primary.push(ComponentBounds::stroked(
            f64::from(leftmost_tone_x - 8),
            -12.0,
            f64::from(rightmost_tone_x + 20),
            -12.0,
            1.0,
        ));
    }
    if event.tones.len() > 1 {
        let top = event
            .tones
            .iter()
            .map(|tone| lane_y(tone.tone.octave))
            .min()
            .expect("chord has a tone");
        let bottom = event
            .tones
            .iter()
            .map(|tone| lane_y(tone.tone.octave) + 18)
            .max()
            .expect("chord has a tone");
        primary.push(ComponentBounds::stroked(
            f64::from(leftmost_tone_x - 9),
            f64::from(top),
            f64::from(leftmost_tone_x - 3),
            f64::from(bottom),
            1.5,
        ));
        primary.push(ComponentBounds::stroked(
            f64::from(rightmost_tone_x + 13),
            f64::from(top),
            f64::from(rightmost_tone_x + 19),
            f64::from(bottom),
            1.5,
        ));
    }

    for (tone_index, tone_event) in event.tones.iter().enumerate() {
        if !tone_event.tie_stop {
            continue;
        }
        let Some((_, start_x, start_y)) = pending_ties
            .iter()
            .find(|(pending_tone, _, _)| *pending_tone == tone_event.tone)
        else {
            continue;
        };
        let end_x = tone_xs[tone_index] + 5;
        incoming.push(ComponentBounds::stroked(
            f64::from((*start_x).min(*start_x + 10).min(end_x - 10).min(end_x)),
            f64::from(*start_y),
            f64::from((*start_x).max(*start_x + 10).max(end_x - 10).max(end_x)),
            f64::from(*start_y + 9),
            1.5,
        ));
    }

    EventGeometry { primary, incoming }
}

fn next_event_x(preferred_x: i32, previous: &EventGeometry, current: &EventGeometry) -> i32 {
    let mut required_x = preferred_x;
    for previous_component in previous.all() {
        for current_component in &current.primary {
            if previous_component.overlaps_vertically(*current_component) {
                required_x = required_x.max(
                    (f64::from(preferred_x) + previous_component.right + 2.0
                        - current_component.left)
                        .ceil() as i32,
                );
            }
        }
    }
    required_x
}

struct Scene {
    view_left: i32,
    view_top: i32,
    view_width: i32,
    view_height: i32,
    commands: Vec<DrawCommand>,
}

fn escape_svg_attribute(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        escaped.push_str(match character {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '"' => "&quot;",
            '\'' => "&apos;",
            _ => {
                escaped.push(character);
                continue;
            }
        });
    }
    escaped
}

impl Scene {
    fn new(view_top: i32, view_width: i32, view_height: i32) -> Self {
        Self {
            view_left: 0,
            view_top,
            view_width,
            view_height,
            commands: Vec::new(),
        }
    }

    fn add_path(
        &mut self,
        layer: DrawLayer,
        id: impl Into<String>,
        class: Option<PathClass>,
        metadata: Vec<PathMetadata>,
        geometry: PathGeometry,
    ) {
        self.commands.push(DrawCommand {
            layer,
            id: id.into(),
            class,
            metadata,
            geometry,
        });
    }

    fn to_svg(&self) -> String {
        let mut svg = String::new();
        writeln!(
            svg,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\" role=\"img\">",
            self.view_left, self.view_top, self.view_width, self.view_height
        )
        .expect("writing to a String cannot fail");
        writeln!(
            svg,
            "<rect id=\"background\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#fff\"/>",
            self.view_left, self.view_top, self.view_width, self.view_height
        )
        .expect("writing to a String cannot fail");
        for layer in [
            DrawLayer::Staff,
            DrawLayer::Barline,
            DrawLayer::Beam,
            DrawLayer::Event,
            DrawLayer::Text,
            DrawLayer::Tie,
        ] {
            for command in self
                .commands
                .iter()
                .filter(|command| command.layer == layer)
            {
                self.write_path(&mut svg, command);
            }
        }
        svg.push_str("</svg>\n");
        svg
    }

    fn write_path(&self, svg: &mut String, command: &DrawCommand) {
        write!(svg, "<path id=\"{}\"", command.id).expect("writing to a String cannot fail");
        if let Some(class) = command.class {
            let class = match class {
                PathClass::Rest => "rest",
                PathClass::Stem => "stem",
                PathClass::Flag => "flag",
                PathClass::Beam => "beam",
                PathClass::BeamHook => "beam-hook",
                PathClass::DurationMark => "duration-mark",
                PathClass::ChordBracket => "chord-bracket",
                PathClass::Tonehead => "tonehead",
                PathClass::ChordSymbolRoot => "chord-symbol-root",
                PathClass::ChordSymbolBass => "chord-symbol-bass",
                PathClass::ChordSymbolTone => "chord-symbol-tone",
                PathClass::TextGlyph => "text-glyph",
                PathClass::DirectionSymbol => "direction-symbol",
                PathClass::SegnoMark => "segno-mark",
                PathClass::CodaMark => "coda-mark",
                PathClass::TextEnclosure => "text-enclosure",
                PathClass::Tie => "tie",
                PathClass::RepeatBarline => "repeat-barline",
                PathClass::RepeatDot => "repeat-dot",
                PathClass::EndingBracket => "ending-bracket",
                PathClass::EndingLabel => "ending-label",
            };
            write!(svg, " class=\"{class}\"").expect("writing to a String cannot fail");
        }
        for metadata in &command.metadata {
            match metadata {
                PathMetadata::Event(value) => write!(svg, " data-event=\"{value}\""),
                PathMetadata::ToneIndex(value) => write!(svg, " data-tone-index=\"{value}\""),
                PathMetadata::Tone(value) => write!(svg, " data-tone=\"{value}\""),
                PathMetadata::Octave(value) => write!(svg, " data-octave=\"{value}\""),
                PathMetadata::Duration(value) => write!(svg, " data-duration=\"{value}\""),
                PathMetadata::ChordRole(value) => write!(svg, " data-chord-role=\"{value}\""),
                PathMetadata::BeamLevel(value) => write!(svg, " data-level=\"{value}\""),
                PathMetadata::EndingLabel(value) => write!(
                    svg,
                    " data-ending-label=\"{}\"",
                    escape_svg_attribute(value)
                ),
            }
            .expect("writing to a String cannot fail");
        }
        let mut transforms = String::new();
        if let Some((x, y)) = command.geometry.transform {
            if command.geometry.x_offset == 0.0 && command.geometry.y_offset == 0.0 {
                write!(transforms, "translate({x} {y})")
            } else if command.geometry.y_offset == 0.0 {
                write!(
                    transforms,
                    "translate({:.2} {y})",
                    f64::from(x) + command.geometry.x_offset
                )
            } else if command.geometry.x_offset == 0.0 {
                write!(
                    transforms,
                    "translate({x} {:.2})",
                    f64::from(y) + command.geometry.y_offset
                )
            } else {
                write!(
                    transforms,
                    "translate({:.2} {:.2})",
                    f64::from(x) + command.geometry.x_offset,
                    f64::from(y) + command.geometry.y_offset
                )
            }
            .expect("writing to a String cannot fail");
        }
        if command.geometry.scale != 1.0 {
            if !transforms.is_empty() {
                transforms.push(' ');
            }
            write!(transforms, "scale({})", command.geometry.scale)
                .expect("writing to a String cannot fail");
        }
        if let Some((degrees, x, y)) = command.geometry.rotation {
            if !transforms.is_empty() {
                transforms.push(' ');
            }
            write!(transforms, "rotate({degrees} {x} {y})")
                .expect("writing to a String cannot fail");
        }
        if !transforms.is_empty() {
            write!(svg, " transform=\"{transforms}\"")
        } else {
            Ok(())
        }
        .expect("writing to a String cannot fail");
        write!(svg, " d=\"{}\"", command.geometry.path_data)
            .expect("writing to a String cannot fail");
        match command.geometry.style {
            PathStyle::StaffLine => svg.push_str(" stroke=\"#171717\""),
            PathStyle::MediumStroke => svg.push_str(" stroke=\"#171717\" stroke-width=\"1.5\""),
            PathStyle::HeavyStroke => svg.push_str(" stroke=\"#171717\" stroke-width=\"3\""),
            PathStyle::DottedStroke => {
                svg.push_str(" stroke=\"#171717\" stroke-width=\"1.5\" stroke-dasharray=\"1 3\"")
            }
            PathStyle::DashedStroke => {
                svg.push_str(" stroke=\"#171717\" stroke-width=\"1.5\" stroke-dasharray=\"5 3\"")
            }
            PathStyle::MediumOutline => {
                svg.push_str(" fill=\"none\" stroke=\"#171717\" stroke-width=\"1.5\"")
            }
            PathStyle::FilledGlyph => svg.push_str(" fill=\"#171717\" fill-rule=\"evenodd\""),
            PathStyle::BravuraGlyph => write!(
                svg,
                " fill=\"{}\" fill-rule=\"nonzero\"",
                command.geometry.color.as_deref().unwrap_or("#171717")
            )
            .expect("writing to a String cannot fail"),
            PathStyle::TextGlyph => write!(
                svg,
                " fill=\"{}\" fill-rule=\"nonzero\"",
                command.geometry.color.as_deref().unwrap_or("#171717")
            )
            .expect("writing to a String cannot fail"),
            PathStyle::TextEnclosure => write!(
                svg,
                " fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\"",
                command.geometry.color.as_deref().unwrap_or("#171717")
            )
            .expect("writing to a String cannot fail"),
            PathStyle::Beam => svg.push_str(" fill=\"#171717\""),
            PathStyle::Solid => svg.push_str(" fill=\"#171717\""),
        }
        svg.push_str("/>\n");
    }
}

fn rest_origin_y(duration_name: &str, bounds: (f64, f64, f64, f64)) -> f64 {
    let (_, top, _, bottom) = bounds;
    let center_line_y = 68.0;
    match duration_name {
        "whole" => center_line_y - top,
        "half" => center_line_y - bottom,
        "quarter" | "eighth" | "16th" | "32nd" | "64th" => center_line_y - (top + bottom) / 2.0,
        _ => unreachable!("rest duration is validated before rendering"),
    }
}

fn add_chord_symbol(
    scene: &mut Scene,
    symbol: &ChordSymbol,
    x: i32,
    offset_x: f64,
    index: usize,
    right: &mut i32,
    view_top: &mut i32,
) -> i32 {
    const ROOT_TOP: i32 = -20;
    const SMALL_DIGIT_X: i32 = 22;
    const TONE_SPACING: i32 = 13;
    let root_id = format!("chord-symbol-{index}-root");
    scene.add_path(
        DrawLayer::Event,
        root_id,
        Some(PathClass::ChordSymbolRoot),
        vec![
            PathMetadata::ChordRole("root"),
            PathMetadata::Tone(symbol.root),
        ],
        PathGeometry::new(
            Some((x, ROOT_TOP)),
            glyph_path(symbol.root),
            PathStyle::FilledGlyph,
        )
        .offset_x(offset_x)
        .scaled(2.0)
        .rotated_180_if(symbol.root == 10),
    );
    if let Some(bass) = symbol.bass {
        scene.add_path(
            DrawLayer::Event,
            format!("chord-symbol-{index}-bass"),
            Some(PathClass::ChordSymbolBass),
            vec![PathMetadata::ChordRole("bass"), PathMetadata::Tone(bass)],
            PathGeometry::new(
                Some((x + SMALL_DIGIT_X, ROOT_TOP + 17)),
                glyph_path(bass),
                PathStyle::FilledGlyph,
            )
            .offset_x(offset_x)
            .rotated_180_if(bass == 10),
        );
    }
    for (tone_index, tone) in symbol.tones.iter().enumerate() {
        scene.add_path(
            DrawLayer::Event,
            format!("chord-symbol-{index}-tone-{tone_index}"),
            Some(PathClass::ChordSymbolTone),
            vec![
                PathMetadata::ChordRole("tone"),
                PathMetadata::Tone(tone.tone),
            ],
            PathGeometry::new(
                Some((
                    x + SMALL_DIGIT_X + tone_index as i32 * TONE_SPACING,
                    ROOT_TOP,
                )),
                glyph_path(tone.tone),
                PathStyle::FilledGlyph,
            )
            .offset_x(offset_x)
            .rotated_180_if(tone.tone == 10),
        );
    }
    *view_top = (*view_top).min(ROOT_TOP);
    let last_tone_x =
        symbol.tones.len().saturating_sub(1).min(i32::MAX as usize) as i32 * TONE_SPACING;
    let symbol_width = SMALL_DIGIT_X + last_tone_x + 24;
    *right = (*right).max((f64::from(x + symbol_width) + offset_x).ceil() as i32);
    symbol_width
}

fn chord_symbol_bounds(x: i32, width: i32, offset_x: f64) -> ComponentBounds {
    ComponentBounds::filled(
        f64::from(x) + offset_x,
        -20.0,
        f64::from(x + width) + offset_x,
        14.0,
    )
}

struct TextBounds {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

struct TextLayoutContext<'a> {
    occupied: &'a mut Vec<TextBounds>,
    notation_bounds: &'a [ComponentBounds],
    warnings: &'a mut Vec<String>,
    left: &'a mut i32,
    right: &'a mut i32,
    view_top: &'a mut i32,
    view_bottom: &'a mut i32,
}

fn add_text_item(
    scene: &mut Scene,
    item: &PositionedText,
    index: usize,
    anchor_x: f64,
    measure_start_x: f64,
    layout: &mut TextLayoutContext<'_>,
) {
    if let Some(glyph) = item.path_glyph {
        add_direction_glyph(scene, item, glyph, index, anchor_x, measure_start_x, layout);
        return;
    }
    let scale = item.font_size / noto_sans_glyphs::UNITS_PER_EM;
    let mut glyphs = Vec::new();
    let mut line_index = 0usize;
    let mut line_x = 0.0;
    let mut max_line_width: f64 = 0.0;
    let line_height = item.font_size * 1.2;
    let mut ink_bounds: Option<(f64, f64, f64, f64)> = None;
    let space_advance = noto_sans_glyphs::glyph(' ')
        .expect("supported text repertoire includes a space")
        .advance
        * scale;
    for (glyph_index, character) in item.text.chars().enumerate() {
        if character == '\n' {
            max_line_width = max_line_width.max(line_x);
            line_index += 1;
            line_x = 0.0;
            continue;
        }
        if character == '\t' {
            line_x += space_advance * 4.0;
            continue;
        }
        let glyph = noto_sans_glyphs::glyph(character)
            .expect("text was checked against the bundled glyph repertoire");
        if !glyph.path.is_empty() {
            let y_offset = line_index as f64 * line_height;
            let left = line_x + glyph.bounds[0] * scale;
            let top = y_offset + glyph.bounds[1] * scale;
            let glyph_right = line_x + glyph.bounds[2] * scale;
            let bottom = y_offset + glyph.bounds[3] * scale;
            ink_bounds = Some(match ink_bounds {
                Some((min_left, min_top, max_right, max_bottom)) => (
                    min_left.min(left),
                    min_top.min(top),
                    max_right.max(glyph_right),
                    max_bottom.max(bottom),
                ),
                None => (left, top, glyph_right, bottom),
            });
            glyphs.push((glyph_index, glyph, line_x, y_offset));
        }
        line_x += glyph.advance * scale;
    }
    max_line_width = max_line_width.max(line_x);
    let Some((ink_left, ink_top, ink_right, ink_bottom)) = ink_bounds else {
        return;
    };

    let text_x = item
        .default_x
        .map(|value| measure_start_x + value * 2.0)
        .unwrap_or(anchor_x)
        + item.relative_x.unwrap_or(0.0) * 2.0;
    let base_y = item
        .default_y
        .map(|value| 68.0 - value * 2.0)
        .unwrap_or(match item.placement {
            TextPlacement::Above => 16.0,
            TextPlacement::Below => 140.0,
        })
        - item.relative_y.unwrap_or(0.0) * 2.0;
    let left = text_x + ink_left.min(0.0);
    let text_right = text_x + ink_right.max(max_line_width);
    let padding = if item.enclosure.is_some() { 3.0 } else { 0.0 };
    let step = (ink_bottom - ink_top + padding * 2.0 + 4.0).max(item.font_size + 4.0);
    let mut shift = 0.0;
    let (box_left, box_top, box_right, box_bottom) = loop {
        let candidate = enclosure_bounds(
            item.enclosure.as_deref(),
            left - padding,
            base_y + ink_top + shift - padding,
            text_right + padding,
            base_y + ink_bottom + shift + padding,
        );
        let overlaps_text = layout.occupied.iter().any(|previous| {
            candidate.0 < previous.right + 2.0
                && candidate.2 + 2.0 > previous.left
                && candidate.1 < previous.bottom + 2.0
                && candidate.3 + 2.0 > previous.top
        });
        let overlaps_notation = layout.notation_bounds.iter().any(|previous| {
            candidate.0 < previous.right + 2.0
                && candidate.2 + 2.0 > previous.left
                && candidate.1 < previous.bottom + 2.0
                && candidate.3 + 2.0 > previous.top
        });
        if !overlaps_text && !overlaps_notation {
            break candidate;
        }
        if shift.abs() > 2000.0 {
            layout.warnings.push(format!(
                "text item {index} could not be separated from nearby text or notation"
            ));
            break candidate;
        }
        let collision_side = if base_y < 68.0 {
            TextPlacement::Above
        } else if base_y > 68.0 {
            TextPlacement::Below
        } else {
            item.placement
        };
        shift += match collision_side {
            TextPlacement::Above => -step,
            TextPlacement::Below => step,
        };
    };
    layout.occupied.push(TextBounds {
        left: box_left,
        top: box_top,
        right: box_right,
        bottom: box_bottom,
    });

    if let Some(enclosure) = &item.enclosure {
        let path = text_enclosure_path(enclosure, box_left, box_top, box_right, box_bottom);
        scene.add_path(
            DrawLayer::Text,
            format!("text-{index}-enclosure"),
            Some(PathClass::TextEnclosure),
            Vec::new(),
            PathGeometry::new(None, path, PathStyle::TextEnclosure).colored(&item.color),
        );
    }
    for (glyph_index, glyph, glyph_x, glyph_y) in glyphs {
        let x = text_x + glyph_x;
        let y = base_y + shift + glyph_y;
        let x_integer = x.floor() as i32;
        let y_integer = y.floor() as i32;
        scene.add_path(
            DrawLayer::Text,
            format!("text-{index}-glyph-{glyph_index}"),
            Some(PathClass::TextGlyph),
            Vec::new(),
            PathGeometry::new(
                Some((x_integer, y_integer)),
                glyph.path,
                PathStyle::TextGlyph,
            )
            .scaled(scale)
            .offset_x(x - f64::from(x_integer))
            .offset_y(y - f64::from(y_integer))
            .colored(&item.color),
        );
    }
    *layout.right = (*layout.right).max(box_right.ceil() as i32 + 8);
    *layout.left = (*layout.left).min(box_left.floor() as i32 - 8);
    let enclosure_stroke_padding = if item.enclosure.is_some() { 0.75 } else { 0.0 };
    *layout.view_top = (*layout.view_top).min((box_top - enclosure_stroke_padding).floor() as i32);
    *layout.view_bottom =
        (*layout.view_bottom).max((box_bottom + enclosure_stroke_padding).ceil() as i32);
}

fn add_direction_glyph(
    scene: &mut Scene,
    item: &PositionedText,
    glyph: bravura_glyphs::Glyph,
    index: usize,
    anchor_x: f64,
    measure_start_x: f64,
    layout: &mut TextLayoutContext<'_>,
) {
    let scale = item.font_size / 16.0 * item.path_scale;
    let left = item
        .default_x
        .map(|value| measure_start_x + value * 2.0)
        .unwrap_or(anchor_x)
        + item.relative_x.unwrap_or(0.0) * 2.0
        + glyph.bounds.0 * scale;
    let text_y = item
        .default_y
        .map(|value| 68.0 - value * 2.0)
        .unwrap_or(match item.placement {
            TextPlacement::Above => 16.0,
            TextPlacement::Below => 140.0,
        })
        - item.relative_y.unwrap_or(0.0) * 2.0
        + item.path_y_offset * scale;
    let top = text_y + glyph.bounds.1 * scale;
    let right = left + (glyph.bounds.2 - glyph.bounds.0) * scale;
    let bottom = top + (glyph.bounds.3 - glyph.bounds.1) * scale;
    let collision_height = if matches!(item.path_id_prefix, "segno" | "coda") {
        let segno_bounds = bravura_glyphs::segno().bounds;
        (segno_bounds.3 - segno_bounds.1) * scale
    } else {
        bottom - top
    };
    let step = (collision_height + 4.0).max(item.font_size + 4.0);
    let mut shift = 0.0;
    let (box_left, box_top, box_right, box_bottom) = loop {
        let candidate = (left, top + shift, right, bottom + shift);
        let overlaps_text = layout.occupied.iter().any(|previous| {
            candidate.0 < previous.right + 2.0
                && candidate.2 + 2.0 > previous.left
                && candidate.1 < previous.bottom + 2.0
                && candidate.3 + 2.0 > previous.top
        });
        let overlaps_notation = layout.notation_bounds.iter().any(|previous| {
            candidate.0 < previous.right + 2.0
                && candidate.2 + 2.0 > previous.left
                && candidate.1 < previous.bottom + 2.0
                && candidate.3 + 2.0 > previous.top
        });
        if !overlaps_text && !overlaps_notation {
            break candidate;
        }
        if shift.abs() > 2000.0 {
            layout.warnings.push(format!(
                "symbol item {index} could not be separated from nearby text or notation"
            ));
            break candidate;
        }
        let collision_side = if text_y < 68.0 {
            TextPlacement::Above
        } else if text_y > 68.0 {
            TextPlacement::Below
        } else {
            item.placement
        };
        shift += match collision_side {
            TextPlacement::Above => -step,
            TextPlacement::Below => step,
        };
    };
    layout.occupied.push(TextBounds {
        left: box_left,
        top: box_top,
        right: box_right,
        bottom: box_bottom,
    });
    let x = box_left - glyph.bounds.0 * scale;
    let y = box_top - glyph.bounds.1 * scale;
    let x_integer = x.floor() as i32;
    let y_integer = y.floor() as i32;
    scene.add_path(
        DrawLayer::Text,
        format!("direction-{}-{index}", item.path_id_prefix),
        Some(match item.path_id_prefix {
            "segno" => PathClass::SegnoMark,
            "coda" => PathClass::CodaMark,
            _ => PathClass::DirectionSymbol,
        }),
        Vec::new(),
        PathGeometry::new(
            Some((x_integer, y_integer)),
            glyph.path,
            PathStyle::BravuraGlyph,
        )
        .scaled(scale)
        .offset_x(x - f64::from(x_integer))
        .offset_y(y - f64::from(y_integer))
        .colored(&item.color),
    );
    *layout.right = (*layout.right).max(box_right.ceil() as i32 + 8);
    *layout.left = (*layout.left).min(box_left.floor() as i32 - 8);
    *layout.view_top = (*layout.view_top).min(box_top.floor() as i32);
    *layout.view_bottom = (*layout.view_bottom).max(box_bottom.ceil() as i32);
}

fn enclosure_bounds(
    shape: Option<&str>,
    mut left: f64,
    mut top: f64,
    mut right: f64,
    mut bottom: f64,
) -> (f64, f64, f64, f64) {
    if matches!(shape, Some("square" | "circle")) {
        let side = (right - left).max(bottom - top);
        let center_x = (left + right) / 2.0;
        let center_y = (top + bottom) / 2.0;
        left = center_x - side / 2.0;
        right = center_x + side / 2.0;
        top = center_y - side / 2.0;
        bottom = center_y + side / 2.0;
    }
    (left, top, right, bottom)
}

fn text_enclosure_path(shape: &str, left: f64, top: f64, right: f64, bottom: f64) -> String {
    let (left, top, right, bottom) = enclosure_bounds(Some(shape), left, top, right, bottom);
    if shape == "bracket" {
        let arm = ((bottom - top) / 3.0).min(3.0);
        return format!(
            "M{:.2} {top:.2}H{left:.2}V{bottom:.2}H{:.2}M{:.2} {top:.2}H{right:.2}V{bottom:.2}H{:.2}",
            left + arm,
            left + arm,
            right - arm,
            right - arm
        );
    }
    if matches!(shape, "oval" | "circle") {
        let center_x = (left + right) / 2.0;
        let center_y = (top + bottom) / 2.0;
        let radius_x = (right - left) / 2.0;
        let radius_y = (bottom - top) / 2.0;
        let kappa = 0.552_284_749_8;
        return format!(
            "M{center_x:.2} {top:.2}C{:.2} {top:.2} {right:.2} {:.2} {right:.2} {center_y:.2}C{right:.2} {:.2} {:.2} {bottom:.2} {center_x:.2} {bottom:.2}C{:.2} {bottom:.2} {left:.2} {:.2} {left:.2} {center_y:.2}C{left:.2} {:.2} {:.2} {top:.2} {center_x:.2} {top:.2}Z",
            center_x + radius_x * kappa,
            center_y - radius_y * kappa,
            center_y + radius_y * kappa,
            center_x + radius_x * kappa,
            center_x - radius_x * kappa,
            center_y + radius_y * kappa,
            center_y - radius_y * kappa,
            center_x - radius_x * kappa
        );
    }
    if matches!(
        shape,
        "triangle"
            | "diamond"
            | "pentagon"
            | "hexagon"
            | "heptagon"
            | "octagon"
            | "nonagon"
            | "decagon"
    ) {
        let sides = match shape {
            "triangle" => 3,
            "diamond" => 4,
            "pentagon" => 5,
            "hexagon" => 6,
            "heptagon" => 7,
            "octagon" => 8,
            "nonagon" => 9,
            "decagon" => 10,
            _ => unreachable!(),
        };
        let center_x = (left + right) / 2.0;
        let center_y = (top + bottom) / 2.0;
        let radius_x = (right - left) / 2.0;
        let radius_y = (bottom - top) / 2.0;
        let points = (0..sides)
            .map(|index| {
                let angle = std::f64::consts::TAU * index as f64 / f64::from(sides)
                    - std::f64::consts::FRAC_PI_2;
                format!(
                    "{:.2} {:.2}",
                    center_x + radius_x * angle.cos(),
                    center_y + radius_y * angle.sin()
                )
            })
            .collect::<Vec<_>>();
        return format!("M{}Z", points.join("L"));
    }
    format!("M{left:.2} {top:.2}H{right:.2}V{bottom:.2}H{left:.2}Z")
}

fn render_svg(score: &Score, warnings: &mut Vec<String>) -> String {
    if score.parts.len() == 1 {
        return render_part_svg(&score.parts[0], warnings, None).svg;
    }
    let provisional = score
        .parts
        .iter()
        .map(|part| {
            let mut provisional_warnings = Vec::new();
            render_part_svg(part, &mut provisional_warnings, None)
        })
        .collect::<Vec<_>>();
    let provisional_barline_positions = provisional
        .iter()
        .map(|part| &part.barline_positions)
        .collect::<Vec<_>>();
    let mut measure_barline_positions = vec![0; score.parts[0].measures.len()];
    for measure_index in 0..measure_barline_positions.len() {
        measure_barline_positions[measure_index] = provisional_barline_positions
            .iter()
            .map(|positions| {
                let prior_alignment_offset = if measure_index == 0 {
                    0
                } else {
                    measure_barline_positions[measure_index - 1] - positions[measure_index - 1]
                };
                positions[measure_index] + prior_alignment_offset
            })
            .max()
            .expect("a score contains at least one Part");
    }
    let rendered = score
        .parts
        .iter()
        .map(|part| render_part_svg(part, warnings, Some(&measure_barline_positions)))
        .collect::<Vec<_>>();
    let left = rendered
        .iter()
        .map(|part| part.view_box.left)
        .min()
        .unwrap_or(0);
    let notation_right = rendered
        .iter()
        .map(|part| part.view_box.left + part.view_box.width)
        .max()
        .unwrap_or(128);
    let part_name_right = score
        .parts
        .iter()
        .map(|part| {
            12.0 + part
                .name
                .chars()
                .map(|character| {
                    noto_sans_glyphs::glyph(character)
                        .expect("Part name was checked against the bundled glyph repertoire")
                        .advance
                        * 12.0
                        / noto_sans_glyphs::UNITS_PER_EM
                })
                .sum::<f64>()
        })
        .fold(0.0_f64, f64::max)
        .ceil() as i32;
    let right = notation_right.max(part_name_right);
    let width = right - left;
    let row_heights = rendered
        .iter()
        .map(|part| part.view_box.height + 40)
        .collect::<Vec<_>>();
    let height = row_heights.iter().sum::<i32>();
    let mut svg = String::new();
    writeln!(svg, "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{left} 0 {width} {height}\" role=\"img\">")
        .expect("writing to a String cannot fail");
    writeln!(svg, "<rect id=\"background\" x=\"{left}\" y=\"0\" width=\"{width}\" height=\"{height}\" fill=\"#fff\"/>")
        .expect("writing to a String cannot fail");
    let mut row_top = 0;
    for (part_index, (part, rendered_part)) in score.parts.iter().zip(rendered).enumerate() {
        let offset_y = row_top + 28 - rendered_part.view_box.top;
        let scale = 12.0 / noto_sans_glyphs::UNITS_PER_EM;
        let mut name_x = 12.0;
        for (glyph_index, character) in part.name.chars().enumerate() {
            let glyph = noto_sans_glyphs::glyph(character)
                .expect("Part name was checked against the bundled glyph repertoire");
            if !glyph.path.is_empty() {
                writeln!(svg, "<path id=\"part-name-{part_index}-glyph-{glyph_index}\" transform=\"translate({name_x:.2} {}) scale({scale:.4})\" d=\"{}\" fill=\"#171717\" fill-rule=\"evenodd\"/>", row_top + 18, glyph.path)
                    .expect("writing to a String cannot fail");
            }
            name_x += glyph.advance * scale;
        }
        writeln!(svg, "<g transform=\"translate(0 {offset_y})\">")
            .expect("writing to a String cannot fail");
        for line in rendered_part
            .svg
            .lines()
            .skip(2)
            .take_while(|line| *line != "</svg>")
        {
            svg.push_str(&line.replace("id=\"", &format!("id=\"part-{part_index}-")));
            svg.push('\n');
        }
        svg.push_str("</g>\n");
        row_top += row_heights[part_index];
    }
    svg.push_str("</svg>\n");
    svg
}

fn measure_barline_position(
    barline: &MeasureBarline,
    measure_start_x: i32,
    measure_end_x: i32,
    event_positions: &[i32],
) -> i32 {
    match barline.location.as_str() {
        "left" => measure_start_x,
        "middle" => event_positions
            .get(barline.event_index)
            .copied()
            .unwrap_or(measure_end_x),
        "right" => measure_end_x,
        _ => unreachable!("barline location is validated during parsing"),
    }
}

fn repeat_barline_strokes(
    style: Option<&str>,
    repeat_start: bool,
    x: i32,
) -> Vec<(i32, PathStyle, &'static str)> {
    match style.unwrap_or(if repeat_start {
        "heavy-light"
    } else {
        "light-heavy"
    }) {
        "regular" => vec![(x, PathStyle::StaffLine, "M0 27.5V108.5")],
        "dotted" => vec![(x, PathStyle::DottedStroke, "M0 27.5V108.5")],
        "dashed" => vec![(x, PathStyle::DashedStroke, "M0 27.5V108.5")],
        "heavy" => vec![(x, PathStyle::HeavyStroke, "M0 27.5V108.5")],
        "light-light" => vec![
            (x - 4, PathStyle::MediumStroke, "M0 27.5V108.5"),
            (x, PathStyle::MediumStroke, "M0 27.5V108.5"),
        ],
        "light-heavy" => vec![
            (x - 4, PathStyle::MediumStroke, "M0 27.5V108.5"),
            (x, PathStyle::HeavyStroke, "M0 27.5V108.5"),
        ],
        "heavy-light" => vec![
            (x - 4, PathStyle::HeavyStroke, "M0 27.5V108.5"),
            (x, PathStyle::MediumStroke, "M0 27.5V108.5"),
        ],
        "heavy-heavy" => vec![
            (x - 4, PathStyle::HeavyStroke, "M0 27.5V108.5"),
            (x, PathStyle::HeavyStroke, "M0 27.5V108.5"),
        ],
        "tick" => vec![(x, PathStyle::MediumStroke, "M0 27.5V45.5")],
        "short" => vec![(x, PathStyle::MediumStroke, "M0 40V96")],
        "none" => Vec::new(),
        _ => unreachable!("bar style is validated during parsing"),
    }
}

fn add_ending_label(scene: &mut Scene, label: &str, x: i32, y: i32, measure_index: usize) {
    let scale = 0.025;
    let mut glyph_x = x as f64;
    for (glyph_index, character) in label.chars().enumerate() {
        if let Some(glyph) = noto_sans_glyphs::glyph(character) {
            if !glyph.path.is_empty() {
                scene.add_path(
                    DrawLayer::Barline,
                    format!("ending-label-{measure_index}-{glyph_index}"),
                    Some(PathClass::EndingLabel),
                    vec![PathMetadata::EndingLabel(label.to_owned())],
                    PathGeometry::new(
                        Some((glyph_x.round() as i32, y)),
                        glyph.path,
                        PathStyle::FilledGlyph,
                    )
                    .scaled(scale),
                );
            }
            glyph_x += glyph.advance * scale;
        }
    }
}

fn render_part_svg(
    part: &Part,
    warnings: &mut Vec<String>,
    measure_barline_positions: Option<&[i32]>,
) -> RenderedPart {
    let mut scene = Scene::new(0, 0, 0);
    let group_stem_directions = beamed_group_stem_directions(part);
    let mut x = 92;
    let mut left = 0;
    let mut right = 128;
    let mut view_top = 0;
    let mut view_bottom = 128;
    let mut index = 0;
    let mut active_meter = None;
    let mut meter_index = 0;
    let mut pending_tie_paths: VoiceTiePaths = Vec::new();
    let mut active_beams: VoiceBeamStates = Vec::new();
    let mut pending_beamed_stems: VoiceBeamedStems = Vec::new();
    let mut final_barline_x = 0;
    let mut barline_positions = Vec::new();
    let mut chord_symbol_index = 0;
    let mut text_item_index = 0;
    let mut occupied_text = Vec::new();
    let mut pending_texts = Vec::new();
    let mut notation_bounds = Vec::new();
    let mut active_ending: Option<(i32, String)> = None;
    for (measure_index, measure) in part.measures.iter().enumerate() {
        let measure_start_x = f64::from(x);
        let mut event_positions = Vec::new();
        let mut previous_event_geometry = None;
        let mut harmony_index = 0;
        let mut text_index = 0;
        let mut measure_harmony_right = None;
        if let Some(meter) = measure.meter {
            if active_meter != Some(meter) {
                let numerator_id = if meter_index == 0 {
                    "meter-numerator".to_owned()
                } else {
                    format!("meter-numerator-{meter_index}")
                };
                let denominator_id = if meter_index == 0 {
                    "meter-denominator".to_owned()
                } else {
                    format!("meter-denominator-{meter_index}")
                };
                let meter_x = if measure_index == 0 { x - 34 } else { x };
                scene.add_path(
                    DrawLayer::Event,
                    numerator_id,
                    None,
                    Vec::new(),
                    PathGeometry::new(
                        Some((meter_x, 30)),
                        time_signature_glyph_path(meter.beats),
                        PathStyle::FilledGlyph,
                    )
                    .scaled(2.0)
                    .rotated_180_if(meter.beats == 10),
                );
                notation_bounds.push(ComponentBounds::filled(
                    f64::from(meter_x),
                    30.0,
                    f64::from(meter_x + 24),
                    66.0,
                ));
                scene.add_path(
                    DrawLayer::Event,
                    denominator_id,
                    None,
                    Vec::new(),
                    PathGeometry::new(
                        Some((meter_x, 70)),
                        time_signature_glyph_path(meter.beat_type),
                        PathStyle::FilledGlyph,
                    )
                    .scaled(2.0)
                    .rotated_180_if(meter.beat_type == 10),
                );
                notation_bounds.push(ComponentBounds::filled(
                    f64::from(meter_x),
                    70.0,
                    f64::from(meter_x + 24),
                    106.0,
                ));
                x += 28;
                meter_index += 1;
            }
            active_meter = Some(meter);
        }
        let event_origin_x = x;
        let mut measure_end_x = x;
        let mut placed_event_geometries: Vec<(f64, EventGeometry)> = Vec::new();
        for (measure_event_index, event) in measure.events.iter().enumerate() {
            let preferred_stems_up = group_stem_directions[measure_index][measure_event_index]
                .unwrap_or_else(|| stem_direction_is_up(event.tones.iter()));
            let stems_up = simultaneous_voice_stem_direction(
                measure_event_index,
                &measure.events,
                preferred_stems_up,
            );
            let beam_state_index = voice_state_index(&mut active_beams, &event.voice_id);
            let tie_state_index = voice_state_index(&mut pending_tie_paths, &event.voice_id);
            let beamed_stem_index = voice_state_index(&mut pending_beamed_stems, &event.voice_id);
            let active_beams_for_event = active_beams[beam_state_index].1;
            let pending_ties_for_event = pending_tie_paths[tie_state_index].1.clone();
            let preferred_x = event_origin_x + (event.onset_units * 2.5).ceil() as i32;
            x = preferred_x;
            let mut geometry = event_geometry(
                event,
                x,
                &active_beams_for_event,
                &pending_ties_for_event,
                stems_up,
            );
            for (previous_onset, previous_geometry) in &placed_event_geometries {
                if *previous_onset < event.onset_units {
                    let adjusted_x = next_event_x(x, previous_geometry, &geometry);
                    if adjusted_x != x {
                        x = adjusted_x;
                        geometry = event_geometry(
                            event,
                            x,
                            &active_beams_for_event,
                            &pending_ties_for_event,
                            stems_up,
                        );
                    }
                }
            }
            event_positions.push(x);
            placed_event_geometries.push((event.onset_units, geometry.clone()));
            measure_end_x = measure_end_x.max(x + (event.duration_units * 2.5).ceil() as i32);
            notation_bounds.extend(geometry.all().copied());
            while measure
                .texts
                .get(text_index)
                .is_some_and(|text| text.event_index <= measure_event_index)
            {
                pending_texts.push((
                    &measure.texts[text_index],
                    text_item_index,
                    f64::from(x) + measure.texts[text_index].offset_x,
                    measure_start_x,
                ));
                text_index += 1;
                text_item_index += 1;
            }
            let mut harmony_advance = 0;
            while measure
                .harmonies
                .get(harmony_index)
                .is_some_and(|harmony| harmony.event_index == measure_event_index)
            {
                let symbol_x = x + harmony_advance;
                let symbol_width = add_chord_symbol(
                    &mut scene,
                    &measure.harmonies[harmony_index].symbol,
                    symbol_x,
                    measure.harmonies[harmony_index].offset_x,
                    chord_symbol_index,
                    &mut right,
                    &mut view_top,
                );
                notation_bounds.push(chord_symbol_bounds(
                    symbol_x,
                    symbol_width,
                    measure.harmonies[harmony_index].offset_x,
                ));
                harmony_advance += symbol_width;
                measure_harmony_right = Some(
                    measure_harmony_right.unwrap_or(i32::MIN).max(
                        (f64::from(symbol_x + harmony_advance)
                            + measure.harmonies[harmony_index].offset_x)
                            .ceil() as i32,
                    ),
                );
                harmony_index += 1;
                chord_symbol_index += 1;
            }
            if event.is_spacer {
                index += 1;
                continue;
            }
            for component in geometry.all() {
                right = right.max(component.right.ceil() as i32);
            }
            if event.tones.is_empty() {
                let glyph = bravura_glyphs::rest(event.duration_name);
                let rest_y = rest_origin_y(event.duration_name, glyph.bounds);
                let rest_y_base = rest_y.floor() as i32;
                scene.add_path(
                    DrawLayer::Event,
                    format!("rest-{index}"),
                    Some(PathClass::Rest),
                    vec![PathMetadata::Duration(event.duration_name)],
                    PathGeometry::new(Some((x, rest_y_base)), glyph.path, PathStyle::BravuraGlyph)
                        .offset_y(rest_y - f64::from(rest_y_base)),
                );
                for mark in 0..event.dots {
                    let (mark_x, mark_y) = rest_duration_mark_position(mark, x, rest_y_base);
                    scene.add_path(
                        DrawLayer::Event,
                        format!("duration-mark-{index}-{mark}"),
                        Some(PathClass::DurationMark),
                        Vec::new(),
                        PathGeometry::new(
                            None,
                            format!("M{mark_x} {mark_y}a1.2 1.2 0 1 0 2.4 0a1.2 1.2 0 1 0 -2.4 0"),
                            PathStyle::Solid,
                        ),
                    );
                }
                right = right.max(x + 24);
                previous_event_geometry = Some(geometry);
                index += 1;
                continue;
            }
            let first_tone = &event.tones[0].tone;
            let mut lane_offsets = [0i32; 10];
            let tone_xs = event
                .tones
                .iter()
                .map(|tone| {
                    let octave = usize::from(tone.tone.octave);
                    let tone_x = x + lane_offsets[octave];
                    lane_offsets[octave] += 12;
                    tone_x
                })
                .collect::<Vec<_>>();
            let leftmost_tone_x = *tone_xs.iter().min().expect("event has tones");
            let rightmost_tone_x = *tone_xs.iter().max().expect("event has tones");
            let stem_tone_index = stem_tone_index(event, stems_up);
            let stem_octave_y = lane_y(event.tones[stem_tone_index].tone.octave);
            let (stem_start, stem_end) = stem_endpoints(stem_octave_y, stems_up);
            let beam_direction = if stems_up { 1 } else { -1 };
            let stem_id = if index == 0 {
                "stem".to_owned()
            } else {
                format!("stem-{index}")
            };
            let pending_stem_id = stem_id.clone();
            let stem_x = tone_xs[stem_tone_index] + 6;
            if event.duration_name != "whole" {
                view_top = view_top.min(stem_start.min(stem_end));
                view_bottom = view_bottom.max(stem_start.max(stem_end));
                scene.add_path(
                    DrawLayer::Event,
                    stem_id,
                    Some(PathClass::Stem),
                    Vec::new(),
                    PathGeometry::new(
                        None,
                        format!("M{stem_x} {stem_start}V{stem_end}"),
                        PathStyle::MediumStroke,
                    ),
                );
                let flag_levels = unbeamed_flag_levels(event);
                if !flag_levels.is_empty() {
                    let flag_count = flag_level_count(event.duration_name);
                    let flag_path =
                        bravura_glyphs::flag_contours(flag_count, stems_up, &flag_levels);
                    scene.add_path(
                        DrawLayer::Event,
                        format!("flag-{index}"),
                        Some(PathClass::Flag),
                        Vec::new(),
                        PathGeometry::new(
                            Some((stem_x, stem_end)),
                            flag_path,
                            PathStyle::BravuraGlyph,
                        )
                        .offset_x(-0.75),
                    );
                }
            }
            let primary_beam_slope = active_beams[beam_state_index].1[1]
                .map(|(start_x, start_y)| {
                    f64::from(stem_end - start_y) / f64::from(stem_x - start_x)
                })
                .unwrap_or(0.0);
            for beam in &event.beams {
                let level = usize::from(beam.level);
                let offset = beam_level_offset(beam.level, primary_beam_slope, beam_direction);
                let beam_y = stem_end + offset;
                match beam.state {
                    BeamState::Begin => {
                        active_beams[beam_state_index].1[level] = Some((stem_x, stem_end));
                        if level == 1 {
                            pending_beamed_stems[beamed_stem_index].1.clear();
                        }
                    }
                    BeamState::Continue => {
                        if level == 1 {
                            pending_beamed_stems[beamed_stem_index].1.push((
                                pending_stem_id.clone(),
                                stem_x,
                                stem_start,
                            ));
                        }
                    }
                    BeamState::End => {
                        let (start_x, start_y) = active_beams[beam_state_index].1[level]
                            .expect("beam group is validated before rendering");
                        scene.add_path(
                            DrawLayer::Beam,
                            format!("beam-{index}-{level}"),
                            Some(PathClass::Beam),
                            vec![PathMetadata::BeamLevel(beam.level)],
                            PathGeometry::new(
                                None,
                                beam_segment_path(
                                    start_x,
                                    f64::from(start_y + offset),
                                    stem_x,
                                    f64::from(beam_y),
                                    beam_direction,
                                ),
                                PathStyle::Beam,
                            ),
                        );
                        if level == 1 {
                            for (stem_id, intermediate_x, intermediate_start_y) in
                                pending_beamed_stems[beamed_stem_index].1.drain(..)
                            {
                                let [beam_start, beam_end, ..] = beam_segment_points(
                                    start_x,
                                    f64::from(start_y),
                                    stem_x,
                                    f64::from(stem_end),
                                    beam_direction,
                                );
                                let beam_slope =
                                    (beam_end.1 - beam_start.1) / (beam_end.0 - beam_start.0);
                                let beam_y = beam_start.1
                                    + beam_slope * (f64::from(intermediate_x) - beam_start.0);
                                if let Some(stem) = scene
                                    .commands
                                    .iter_mut()
                                    .find(|command| command.id == stem_id)
                                {
                                    stem.geometry.path_data = format!(
                                        "M{intermediate_x} {intermediate_start_y}V{beam_y:.2}"
                                    );
                                }
                            }
                        }
                        active_beams[beam_state_index].1[level] = None;
                    }
                    BeamState::ForwardHook | BeamState::BackwardHook => {
                        let direction = if beam.state == BeamState::ForwardHook {
                            1
                        } else {
                            -1
                        };
                        let slope = primary_beam_slope;
                        let (start_x, start_y, end_x, end_y) = if direction > 0 {
                            (
                                stem_x,
                                f64::from(beam_y),
                                stem_x + 9,
                                f64::from(beam_y) + slope * 9.0,
                            )
                        } else {
                            (
                                stem_x - 9,
                                f64::from(beam_y) - slope * 9.0,
                                stem_x,
                                f64::from(beam_y),
                            )
                        };
                        scene.add_path(
                            DrawLayer::Beam,
                            format!("beam-hook-{index}-{level}"),
                            Some(PathClass::BeamHook),
                            vec![PathMetadata::BeamLevel(beam.level)],
                            PathGeometry::new(
                                None,
                                beam_segment_path(start_x, start_y, end_x, end_y, beam_direction),
                                PathStyle::Beam,
                            ),
                        );
                    }
                }
            }
            for mark in 0..event.dots {
                let (mark_x, mark_y) = duration_mark_position(
                    event.duration_name,
                    event.dots,
                    mark,
                    x,
                    lane_y(first_tone.octave),
                );
                scene.add_path(
                    DrawLayer::Event,
                    format!("duration-mark-{index}-{mark}"),
                    Some(PathClass::DurationMark),
                    Vec::new(),
                    PathGeometry::new(
                        None,
                        format!("M{mark_x} {mark_y}a1.2 1.2 0 1 0 2.4 0a1.2 1.2 0 1 0 -2.4 0"),
                        PathStyle::Solid,
                    ),
                );
            }
            if event.tones.iter().any(|tone| tone.tone.octave <= 1) {
                scene.add_path(
                    DrawLayer::Event,
                    format!("ledger-line-below-event-{index}"),
                    None,
                    Vec::new(),
                    PathGeometry::new(
                        None,
                        format!("M{} 148H{}", leftmost_tone_x - 8, rightmost_tone_x + 20),
                        PathStyle::StaffLine,
                    ),
                );
            }
            if event.tones.iter().any(|tone| tone.tone.octave >= 8) {
                scene.add_path(
                    DrawLayer::Event,
                    format!("ledger-line-above-event-{index}"),
                    None,
                    Vec::new(),
                    PathGeometry::new(
                        None,
                        format!("M{} -12H{}", leftmost_tone_x - 8, rightmost_tone_x + 20),
                        PathStyle::StaffLine,
                    ),
                );
            }
            if event.tones.len() > 1 {
                let top = event
                    .tones
                    .iter()
                    .map(|tone| lane_y(tone.tone.octave))
                    .min()
                    .expect("chord has a tone");
                let bottom = event
                    .tones
                    .iter()
                    .map(|tone| lane_y(tone.tone.octave) + 18)
                    .max()
                    .expect("chord has a tone");
                scene.add_path(
                    DrawLayer::Event,
                    format!("chord-bracket-left-{index}"),
                    Some(PathClass::ChordBracket),
                    vec![PathMetadata::Event(index)],
                    PathGeometry::new(
                        None,
                        format!(
                            "M{} {top}C{} {} {} {} {} {bottom}",
                            leftmost_tone_x - 3,
                            leftmost_tone_x - 9,
                            top + 3,
                            leftmost_tone_x - 9,
                            bottom - 3,
                            leftmost_tone_x - 3
                        ),
                        PathStyle::MediumOutline,
                    ),
                );
                scene.add_path(
                    DrawLayer::Event,
                    format!("chord-bracket-right-{index}"),
                    Some(PathClass::ChordBracket),
                    vec![PathMetadata::Event(index)],
                    PathGeometry::new(
                        None,
                        format!(
                            "M{} {top}C{} {} {} {} {} {bottom}",
                            rightmost_tone_x + 13,
                            rightmost_tone_x + 19,
                            top + 3,
                            rightmost_tone_x + 19,
                            bottom - 3,
                            rightmost_tone_x + 13
                        ),
                        PathStyle::MediumOutline,
                    ),
                );
                right = right.max(rightmost_tone_x + 22);
            }
            for (tone_index, tone_event) in event.tones.iter().enumerate() {
                let tone = tone_event.tone;
                let octave_y = lane_y(tone.octave);
                let tone_x = tone_xs[tone_index];
                let tonehead_id = if tone_index == 0 && index == 0 {
                    "tonehead".to_owned()
                } else if tone_index == 0 {
                    format!("tonehead-{index}")
                } else {
                    format!("tonehead-{index}-{tone_index}")
                };
                scene.add_path(
                    DrawLayer::Event,
                    tonehead_id,
                    Some(PathClass::Tonehead),
                    vec![
                        PathMetadata::Event(index),
                        PathMetadata::ToneIndex(tone_index),
                        PathMetadata::Tone(tone.value),
                        PathMetadata::Octave(tone.octave),
                        PathMetadata::Duration(event.duration_name),
                    ],
                    PathGeometry::new(
                        Some((tone_x, octave_y)),
                        glyph_path(tone.value),
                        PathStyle::FilledGlyph,
                    )
                    .rotated_180_if(tone.value == 10),
                );
                let tie_y = octave_y + 18;
                if tone_event.tie_stop {
                    if let Some(position) = pending_tie_paths[tie_state_index]
                        .1
                        .iter()
                        .position(|(pending_tone, _, _)| *pending_tone == tone)
                    {
                        let (_, start_x, start_y) =
                            pending_tie_paths[tie_state_index].1.remove(position);
                        let end_x = tone_x + 5;
                        scene.add_path(
                            DrawLayer::Tie,
                            format!("tie-{index}-{tone_index}"),
                            Some(PathClass::Tie),
                            Vec::new(),
                            PathGeometry::new(
                                None,
                                format!(
                                    "M{start_x} {start_y}C{} {} {} {} {end_x} {start_y}",
                                    start_x + 10,
                                    start_y + 9,
                                    end_x - 10,
                                    start_y + 9
                                ),
                                PathStyle::MediumOutline,
                            ),
                        );
                        view_bottom = view_bottom.max(start_y + 10);
                    }
                }
                if tone_event.tie_start {
                    pending_tie_paths[tie_state_index]
                        .1
                        .push((tone, tone_x + 8, tie_y));
                }
                view_top = view_top.min(octave_y - 20);
                view_bottom = view_bottom.max(octave_y + 37);
            }
            right = right.max(x + 28);
            previous_event_geometry = Some(geometry);
            index += 1;
        }
        x = measure_end_x;
        while let Some(text) = measure.texts.get(text_index) {
            pending_texts.push((
                text,
                text_item_index,
                f64::from(x) + text.offset_x,
                measure_start_x,
            ));
            text_index += 1;
            text_item_index += 1;
        }
        let mut harmony_advance = 0;
        while let Some(harmony) = measure.harmonies.get(harmony_index) {
            let symbol_x = x + harmony_advance;
            let symbol_width = add_chord_symbol(
                &mut scene,
                &harmony.symbol,
                symbol_x,
                harmony.offset_x,
                chord_symbol_index,
                &mut right,
                &mut view_top,
            );
            notation_bounds.push(chord_symbol_bounds(
                symbol_x,
                symbol_width,
                harmony.offset_x,
            ));
            harmony_advance += symbol_width;
            measure_harmony_right = Some(
                measure_harmony_right
                    .unwrap_or(i32::MIN)
                    .max((f64::from(symbol_x + harmony_advance) + harmony.offset_x).ceil() as i32),
            );
            harmony_index += 1;
            chord_symbol_index += 1;
        }
        let mut barline_x = x;
        if let Some(harmony_right) = measure_harmony_right {
            barline_x = barline_x.max(harmony_right + 5);
        }
        if let Some(geometry) = &previous_event_geometry {
            let barline_span = ComponentBounds::filled(0.0, 27.5, 0.0, 108.5);
            for component in geometry.all() {
                if component.overlaps_vertically(barline_span) {
                    barline_x = barline_x.max((component.right + 5.0).ceil() as i32);
                }
            }
        }
        if let Some(shared_position) =
            measure_barline_positions.and_then(|positions| positions.get(measure_index))
        {
            barline_x = barline_x.max(*shared_position);
        }
        notation_bounds.push(ComponentBounds::stroked(
            f64::from(barline_x),
            27.5,
            f64::from(barline_x),
            108.5,
            1.0,
        ));
        scene.add_path(
            DrawLayer::Barline,
            format!("barline-{}", measure_index + 1),
            None,
            Vec::new(),
            PathGeometry::new(
                None,
                format!("M{barline_x} 27.5V108.5"),
                PathStyle::StaffLine,
            ),
        );
        for repeat in measure
            .barlines
            .iter()
            .filter(|barline| barline.repeat_start || barline.repeat_end)
        {
            let repeat_x = if repeat.repeat_start && repeat.location == "left" {
                event_positions
                    .first()
                    .copied()
                    .unwrap_or(measure_start_x as i32)
                    - 16
            } else {
                measure_barline_position(
                    repeat,
                    measure_start_x as i32,
                    barline_x,
                    &event_positions,
                )
            };
            let repeat_start = repeat.repeat_start;
            for (styled_line_x, style, path) in
                repeat_barline_strokes(repeat.style.as_deref(), repeat_start, repeat_x)
            {
                scene.add_path(
                    DrawLayer::Barline,
                    format!("repeat-barline-{measure_index}-{styled_line_x}"),
                    Some(PathClass::RepeatBarline),
                    Vec::new(),
                    PathGeometry::new(Some((styled_line_x, 0)), path, style),
                );
            }
            for (dot_index, dot_y) in [62, 74].into_iter().enumerate() {
                let dot_x = repeat_x + if repeat_start { 4 } else { -12 };
                scene.add_path(
                    DrawLayer::Barline,
                    format!("repeat-dot-{measure_index}-{dot_index}"),
                    Some(PathClass::RepeatDot),
                    Vec::new(),
                    PathGeometry::new(
                        Some((dot_x, dot_y)),
                        "M0 0C0 -2 4 -2 4 0S0 2 0 0Z",
                        PathStyle::Solid,
                    ),
                );
            }
        }
        for barline in &measure.barlines {
            if let Some(label) = &barline.ending_start {
                let start_x = measure_barline_position(
                    barline,
                    measure_start_x as i32,
                    barline_x,
                    &event_positions,
                );
                view_top = view_top.min(-6);
                active_ending = Some((start_x, label.label.clone()));
                add_ending_label(&mut scene, &label.label, start_x + 2, 19, measure_index);
            }
            if let Some(ending_end) = &barline.ending_end {
                let (start_x, _) = active_ending
                    .take()
                    .expect("ending spans were validated during parsing");
                let end_x = measure_barline_position(
                    barline,
                    measure_start_x as i32,
                    barline_x,
                    &event_positions,
                );
                scene.add_path(
                    DrawLayer::Barline,
                    format!("ending-bracket-{measure_index}"),
                    Some(PathClass::EndingBracket),
                    Vec::new(),
                    PathGeometry::new(
                        Some((start_x, -4)),
                        if ending_end.discontinue {
                            format!("M0 27V0H{}", end_x - start_x)
                        } else {
                            format!("M0 27V0H{}V27", end_x - start_x)
                        },
                        PathStyle::MediumOutline,
                    ),
                );
            }
        }
        final_barline_x = barline_x;
        barline_positions.push(barline_x);
        right = right.max(barline_x + 12);
        x = barline_x + 4;
    }
    for staff_y in [28.0, 68.0, 108.0] {
        notation_bounds.push(ComponentBounds::stroked(
            12.0,
            staff_y,
            f64::from(final_barline_x),
            staff_y,
            1.0,
        ));
    }
    notation_bounds.push(ComponentBounds::filled(26.0, 49.0, 38.0, 67.0));
    notation_bounds.push(ComponentBounds::filled(26.0, 69.0, 38.0, 87.0));
    for (text, text_item_index, anchor_x, measure_start_x) in pending_texts {
        add_text_item(
            &mut scene,
            text,
            text_item_index,
            anchor_x,
            measure_start_x,
            &mut TextLayoutContext {
                occupied: &mut occupied_text,
                notation_bounds: &notation_bounds,
                warnings,
                left: &mut left,
                right: &mut right,
                view_top: &mut view_top,
                view_bottom: &mut view_bottom,
            },
        );
    }
    let view_width = right + 12 - left;
    scene.view_left = left;
    scene.view_top = view_top;
    scene.view_width = view_width;
    scene.view_height = view_bottom - view_top;
    let view_box = SvgViewBox {
        left,
        top: view_top,
        width: view_width,
        height: view_bottom - view_top,
    };
    for (id, path_data) in [
        ("staff-line-1", format!("M12 28H{final_barline_x}")),
        ("staff-line-2", format!("M12 68H{final_barline_x}")),
        ("staff-line-3", format!("M12 108H{final_barline_x}")),
    ] {
        scene.add_path(
            DrawLayer::Staff,
            id,
            None,
            Vec::new(),
            PathGeometry::new(None, path_data, PathStyle::StaffLine),
        );
    }
    scene.add_path(
        DrawLayer::Staff,
        "octave-5",
        None,
        Vec::new(),
        PathGeometry::new(Some((26, 49)), glyph_path(5), PathStyle::FilledGlyph),
    );
    scene.add_path(
        DrawLayer::Staff,
        "octave-4",
        None,
        Vec::new(),
        PathGeometry::new(Some((26, 69)), glyph_path(4), PathStyle::FilledGlyph),
    );
    RenderedPart {
        svg: scene.to_svg(),
        view_box,
        barline_positions,
    }
}

fn unbeamed_flag_levels(event: &Event) -> Vec<u8> {
    let flag_count = flag_level_count(event.duration_name);
    (1..=flag_count)
        .filter(|level| !event.beams.iter().any(|beam| beam.level == *level))
        .collect()
}

fn flag_level_count(duration_name: &str) -> u8 {
    match duration_name {
        "eighth" => 1,
        "16th" => 2,
        "32nd" => 3,
        "64th" => 4,
        _ => 0,
    }
}

fn lane_y(octave: u8) -> i32 {
    match octave {
        0..=4 => 69 + (4 - i32::from(octave)) * 20,
        _ => 49 - (i32::from(octave) - 5) * 20,
    }
}

fn stem_direction_is_up<'a>(tones: impl Iterator<Item = &'a ToneEvent>) -> bool {
    let (top, bottom) = tones
        .map(|tone| lane_y(tone.tone.octave) + 9)
        .fold((i32::MAX, i32::MIN), |(top, bottom), tone_y| {
            (top.min(tone_y), bottom.max(tone_y))
        });
    top + bottom >= 2 * 68
}

fn beamed_group_stem_directions(part: &Part) -> Vec<Vec<Option<bool>>> {
    let mut directions = part
        .measures
        .iter()
        .map(|measure| vec![None; measure.events.len()])
        .collect::<Vec<_>>();
    let mut group_events: VoiceBeamEvents = Vec::new();

    for (measure_index, measure) in part.measures.iter().enumerate() {
        for (event_index, event) in measure.events.iter().enumerate() {
            let Some(primary_beam) = event.beams.iter().find(|beam| beam.level == 1) else {
                continue;
            };
            let position = (measure_index, event_index);
            let group_index = voice_state_index(&mut group_events, &event.voice_id);
            match primary_beam.state {
                BeamState::Begin => {
                    group_events[group_index].1.clear();
                    group_events[group_index].1.push(position);
                }
                BeamState::Continue => {
                    if !group_events[group_index].1.is_empty() {
                        group_events[group_index].1.push(position);
                    }
                }
                BeamState::End => {
                    if !group_events[group_index].1.is_empty() {
                        group_events[group_index].1.push(position);
                        let stems_up =
                            stem_direction_is_up(group_events[group_index].1.iter().flat_map(
                                |(measure_index, event_index)| {
                                    part.measures[*measure_index].events[*event_index]
                                        .tones
                                        .iter()
                                },
                            ));
                        for (measure_index, event_index) in group_events[group_index].1.drain(..) {
                            directions[measure_index][event_index] = Some(stems_up);
                        }
                    }
                }
                BeamState::ForwardHook | BeamState::BackwardHook => {}
            }
        }
    }

    directions
}

fn stem_tone_index(event: &Event, stems_up: bool) -> usize {
    let tones = event.tones.iter().enumerate();
    if stems_up {
        tones
            .min_by_key(|(_, tone)| lane_y(tone.tone.octave))
            .map(|(tone_index, _)| tone_index)
    } else {
        tones
            .max_by_key(|(_, tone)| lane_y(tone.tone.octave))
            .map(|(tone_index, _)| tone_index)
    }
    .expect("pitched event has a tone")
}

fn simultaneous_voice_stem_direction(
    event_index: usize,
    events: &[Event],
    preferred_stems_up: bool,
) -> bool {
    let event = &events[event_index];
    if event.tones.is_empty() || event.duration_name == "whole" || !event.beams.is_empty() {
        return preferred_stems_up;
    }
    let preferred_collisions =
        simultaneous_voice_stem_collisions(event, events, preferred_stems_up);
    if preferred_collisions == 0 {
        return preferred_stems_up;
    }
    let alternate_collisions =
        simultaneous_voice_stem_collisions(event, events, !preferred_stems_up);
    if alternate_collisions < preferred_collisions {
        !preferred_stems_up
    } else {
        preferred_stems_up
    }
}

fn simultaneous_voice_stem_collisions(event: &Event, events: &[Event], stems_up: bool) -> usize {
    let stem_tone_index = stem_tone_index(event, stems_up);
    let stem_tone = &event.tones[stem_tone_index].tone;
    let mut event_lane_offsets = [0i32; 10];
    let mut stem_tone_x = 0;
    for (tone_index, tone) in event.tones.iter().enumerate() {
        let octave = usize::from(tone.tone.octave);
        if tone_index == stem_tone_index {
            stem_tone_x = event_lane_offsets[octave];
        }
        event_lane_offsets[octave] += 12;
    }
    let stem_x = stem_tone_x + 6;
    let (stem_start, stem_end) = stem_endpoints(lane_y(stem_tone.octave), stems_up);
    let stem_top = stem_start.min(stem_end);
    let stem_bottom = stem_start.max(stem_end);
    events
        .iter()
        .filter(|peer| {
            peer.voice_id != event.voice_id
                && peer.onset_units == event.onset_units
                && !peer.tones.is_empty()
        })
        .flat_map(|peer| {
            let mut lane_offsets = [0i32; 10];
            peer.tones.iter().filter_map(move |tone| {
                let octave = usize::from(tone.tone.octave);
                let tone_x = lane_offsets[octave];
                lane_offsets[octave] += 12;
                let tone_y = lane_y(tone.tone.octave);
                let head_top = tone_y + 2;
                let head_bottom = tone_y + 16;
                let vertical_overlap = stem_top <= head_bottom && head_top <= stem_bottom;
                (vertical_overlap && (tone_x + 1..=tone_x + 11).contains(&stem_x)).then_some(())
            })
        })
        .count()
}

fn stem_endpoints(tonehead_y: i32, stems_up: bool) -> (i32, i32) {
    if stems_up {
        (tonehead_y - 1, tonehead_y - 30)
    } else {
        (tonehead_y + 19, tonehead_y + 48)
    }
}

fn beam_segment_points(
    start_x: i32,
    start_y: f64,
    end_x: i32,
    end_y: f64,
    beam_direction: i32,
) -> [(f64, f64); 4] {
    let center_dx = f64::from(end_x - start_x);
    let slope = (end_y - start_y) / center_dx;
    let (left_offset, right_offset) = if slope == 0.0 {
        (0.75, -0.75)
    } else {
        let beam_rises = slope < 0.0;
        let stems_up = beam_direction > 0;
        let edge_offset = if stems_up != beam_rises { 0.75 } else { -0.75 };
        (edge_offset, edge_offset)
    };
    let left_x = f64::from(start_x) + left_offset;
    let right_x = f64::from(end_x) + right_offset;
    let left_y = start_y;
    let right_y = end_y;
    let vertical_thickness = 3.0 * slope.hypot(1.0);
    let inner_offset = f64::from(beam_direction) * vertical_thickness;
    [
        (left_x, left_y),
        (right_x, right_y),
        (right_x, right_y + inner_offset),
        (left_x, left_y + inner_offset),
    ]
}

fn beam_level_offset(level: u8, slope: f64, beam_direction: i32) -> i32 {
    let per_level_spacing = (4.0 * slope.hypot(1.0)).round() as i32;
    i32::from(level - 1) * per_level_spacing * beam_direction
}

fn beam_segment_bounds(
    start_x: i32,
    start_y: f64,
    end_x: i32,
    end_y: f64,
    beam_direction: i32,
) -> ComponentBounds {
    let points = beam_segment_points(start_x, start_y, end_x, end_y, beam_direction);
    let left = points
        .iter()
        .map(|point| point.0)
        .fold(f64::INFINITY, f64::min);
    let top = points
        .iter()
        .map(|point| point.1)
        .fold(f64::INFINITY, f64::min);
    let right = points
        .iter()
        .map(|point| point.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = points
        .iter()
        .map(|point| point.1)
        .fold(f64::NEG_INFINITY, f64::max);
    ComponentBounds::filled(left, top, right, bottom)
}

fn beam_segment_path(
    start_x: i32,
    start_y: f64,
    end_x: i32,
    end_y: f64,
    beam_direction: i32,
) -> String {
    let [(left_x, left_y), (right_x, right_y), (inner_right_x, inner_right_y), (inner_left_x, inner_left_y)] =
        beam_segment_points(start_x, start_y, end_x, end_y, beam_direction);
    format!(
        "M{left_x:.2} {left_y:.2}L{right_x:.2} {right_y:.2}L{inner_right_x:.2} {inner_right_y:.2}L{inner_left_x:.2} {inner_left_y:.2}Z"
    )
}

fn glyph_path(value: u8) -> &'static str {
    match value {
        0 => "M6 1C2.8 1 1 3.5 1 9S2.8 17 6 17 11 14.5 11 9 9.2 1 6 1ZM6 3.5C7.7 3.5 8.4 5.2 8.4 9S7.7 14.5 6 14.5 3.6 12.8 3.6 9 4.3 3.5 6 3.5Z",
        1 => "M2 5.1L6.1 1.5H8.8V16.5H5.9V5.1L3.3 7.2Z",
        2 => "M2 4.7C2 2.6 3.8 1 6.2 1S10.5 2.4 10.5 4.8C10.5 7.2 9.2 8.8 7.2 10.7L4.7 14.2H10.8V16.5H1.2V14.6L5.5 9.7C7.2 7.8 8 6.5 8 4.9 8 3.8 7.3 3.3 6.2 3.3S4.4 4 4.4 5.2V6H2Z",
        3 => "M2 3.1C3 1.7 4.3 1 6.3 1 9 1 10.6 2.4 10.6 4.7 10.6 6.3 9.8 7.4 8.4 8 10 8.5 11 9.8 11 11.8 11 14.8 9 17 6 17 4 17 2.6 16.1 1.4 14.5L3.2 13C3.9 14 4.8 14.5 6 14.5 7.6 14.5 8.4 13.6 8.4 11.8 8.4 10.1 7.6 9.3 5.7 9.3H4.5V7H5.8C7.4 7 8.1 6.3 8.1 4.9 8.1 3.7 7.5 3.3 6.2 3.3 5.2 3.3 4.4 3.7 3.8 4.6Z",
        4 => "M7.2 1H9.8V10.4H11V12.8H9.8V16.5H7.1V12.8H1V10.4ZM7.1 5.2L3.6 10.4H7.1Z",
        5 => "M2.1 1.5H10.7V3.9H4.4V6.9C5.1 6.4 6 6.1 7 6.1 9.6 6.1 11 8 11 11.2 11 14.8 9.2 17 6.2 17 4.2 17 2.8 16.2 1.6 14.7L3.4 13.1C4.1 14 5 14.5 6.2 14.5 7.7 14.5 8.4 13.4 8.4 11.3 8.4 9.3 7.8 8.4 6.5 8.4 5.6 8.4 4.9 8.8 4.4 9.5H2.1Z",
        6 => "M8.9 1.3L10.3 3.2C8.4 4.4 6.7 5.8 5.4 7.5 6 7.1 6.8 6.9 7.5 6.9 9.8 6.9 11 8.7 11 11.5 11 14.9 9.1 17 6.1 17 3 17 1 14.9 1 11.5 1 7.2 4.2 3.4 8.9 1.3ZM6.1 9.2C4.5 9.2 3.6 10.1 3.6 11.9 3.6 13.7 4.4 14.6 6.1 14.6 7.7 14.6 8.4 13.7 8.4 11.9 8.4 10.1 7.7 9.2 6.1 9.2Z",
        7 => "M1.5 1.5H11V3.6L5.6 16.5H2.7L8 3.9H1.5Z",
        8 => "M6 1C9 1 10.7 2.4 10.7 4.8 10.7 6.3 9.9 7.4 8.6 8 10.2 8.6 11 9.8 11 11.8 11 15.1 9.2 17 6 17 2.8 17 1 15.1 1 11.8 1 9.8 1.8 8.6 3.4 8 2.1 7.4 1.3 6.3 1.3 4.8 1.3 2.4 3 1 6 1ZM6 3.3C4.7 3.3 4 3.9 4 5.1 4 6.3 4.7 7 6 7S8 6.3 8 5.1C8 3.9 7.3 3.3 6 3.3ZM6 9.3C4.4 9.3 3.6 10.1 3.6 11.9 3.6 13.7 4.4 14.6 6 14.6S8.4 13.7 8.4 11.9C8.4 10.1 7.6 9.3 6 9.3Z",
        9 => "M3.1 16.7L1.7 14.8C3.6 13.6 5.3 12.2 6.6 10.5 6 10.9 5.2 11.1 4.5 11.1 2.2 11.1 1 9.3 1 6.5 1 3.1 2.9 1 5.9 1 9 1 11 3.1 11 6.5 11 10.8 7.8 14.6 3.1 16.7ZM5.9 8.8C7.5 8.8 8.4 7.9 8.4 6.1 8.4 4.3 7.6 3.4 5.9 3.4 4.3 3.4 3.6 4.3 3.6 6.1 3.6 7.9 4.3 8.8 5.9 8.8Z",
        10 => glyph_path(2),
        11 => "M10 3.1C9 1.7 7.7 1 5.7 1 3 1 1.4 2.4 1.4 4.7 1.4 6.3 2.2 7.4 3.6 8 2 8.5 1 9.8 1 11.8 1 14.8 3 17 6 17 8 17 9.4 16.1 10.6 14.5L8.8 13C8.1 14 7.2 14.5 6 14.5 4.4 14.5 3.6 13.6 3.6 11.8 3.6 10.1 4.4 9.3 6.3 9.3H7.5V7H6.2C4.6 7 3.9 6.3 3.9 4.9 3.9 3.7 4.5 3.3 5.8 3.3 6.8 3.3 7.6 3.7 8.2 4.6Z",
        _ => unreachable!("tone values are reduced to one dozenal digit"),
    }
}

fn time_signature_glyph_path(value: u8) -> &'static str {
    match value {
        0 => "M6 1C2.7 1 1 3.5 1 9S2.7 17 6 17 11 14.5 11 9 9.3 1 6 1ZM6 3.8C7.4 3.8 8.1 5.4 8.1 9S7.4 14.2 6 14.2 3.9 12.6 3.9 9 4.6 3.8 6 3.8Z",
        1 => "M2 4L6 1H9V14.5H11V17H1V14.5H4V5.1L2 6.4Z",
        2 => "M1.2 4.8C1.2 2.4 3.1 1 6.1 1 9.2 1 10.8 2.4 10.8 4.8 10.8 7.3 9.3 9.1 7.2 11.1L4.4 14.3H11V17H1V14.8L5.2 10C7.1 7.9 8 6.5 8 4.9 8 3.7 7.3 3.3 6.1 3.3 4.8 3.3 4.1 3.9 4.1 5.2V6H1.2Z",
        3 => "M1.5 3.3C2.6 1.7 4.1 1 6.3 1 9.1 1 10.7 2.4 10.7 4.7 10.7 6.3 9.9 7.4 8.4 8 10.1 8.6 11 9.9 11 11.9 11 15.1 9.2 17 6.1 17 4 17 2.5 16.2 1.2 14.5L3.3 12.9C4 13.9 4.9 14.4 6.1 14.4 7.7 14.4 8.4 13.6 8.4 11.9 8.4 10.2 7.7 9.3 5.8 9.3H4.6V7H5.8C7.4 7 8.1 6.3 8.1 4.9 8.1 3.8 7.5 3.4 6.3 3.4 5.2 3.4 4.5 3.8 3.8 4.8Z",
        4 => "M7.3 1H10V10.2H11.5V12.8H10V17H7.2V12.8H0.8V10.3ZM7.2 5.2L3.7 10.2H7.2Z",
        5 => "M1.4 1.5H10.8V4.1H4.2V7C5 6.4 5.9 6.1 7 6.1 9.6 6.1 11 8 11 11.4 11 15 9.2 17 6.1 17 4 17 2.5 16.2 1.2 14.6L3.3 12.9C4 13.9 4.9 14.4 6.1 14.4 7.6 14.4 8.4 13.4 8.4 11.4 8.4 9.4 7.8 8.5 6.5 8.5 5.6 8.5 4.8 8.9 4.3 9.7H1.4Z",
        6 => "M8.9 1.2L10.8 3.2C8.5 4.8 6.6 6.5 5.2 8.1 5.8 7.6 6.7 7.3 7.6 7.3 9.8 7.3 11 9.1 11 11.8 11 15.1 9.2 17 6.1 17 2.9 17 1 15 1 11.6 1 7.1 4.1 3.3 8.9 1.2ZM6.1 9.5C4.5 9.5 3.7 10.4 3.7 12 3.7 13.7 4.5 14.5 6.1 14.5 7.6 14.5 8.4 13.7 8.4 12 8.4 10.4 7.6 9.5 6.1 9.5Z",
        7 => "M0.8 1.5H11V4L5.4 17H2.3L7.9 4.1H0.8Z",
        8 => "M6 1C9.1 1 10.8 2.4 10.8 4.8 10.8 6.3 10 7.4 8.6 8 10.2 8.6 11 9.9 11 11.9 11 15.1 9.2 17 6 17S1 15.1 1 11.9C1 9.9 1.8 8.6 3.4 8 2 7.4 1.2 6.3 1.2 4.8 1.2 2.4 2.9 1 6 1ZM6 3.4C4.7 3.4 4 4 4 5.2S4.7 7 6 7 8 6.4 8 5.2 7.3 3.4 6 3.4ZM6 9.4C4.4 9.4 3.6 10.2 3.6 12S4.4 14.5 6 14.5 8.4 13.7 8.4 12 7.6 9.4 6 9.4Z",
        9 => "M3.1 16.8L1.2 14.8C3.5 13.2 5.4 11.5 6.8 9.9 6.2 10.4 5.3 10.7 4.4 10.7 2.2 10.7 1 8.9 1 6.2 1 3 2.9 1 6 1 9.2 1 11 3.1 11 6.5 11 10.9 7.9 14.7 3.1 16.8ZM6 8.3C7.6 8.3 8.4 7.4 8.4 5.9 8.4 4.3 7.6 3.5 6 3.5 4.4 3.5 3.6 4.3 3.6 5.9 3.6 7.4 4.4 8.3 6 8.3Z",
        10 => glyph_path(10),
        11 => glyph_path(11),
        _ => unreachable!("time signatures are validated before rendering"),
    }
}

#[cfg(test)]
mod tests {
    use super::{glyph_path, parse_score, render_svg, Meter};

    #[test]
    fn tonehead_one_has_no_projecting_foot() {
        assert_eq!(glyph_path(1), "M2 5.1L6.1 1.5H8.8V16.5H5.9V5.1L3.3 7.2Z");
    }

    #[test]
    fn dek_uses_the_two_outline_rotated_180_degrees() {
        let source = include_str!("../tests/fixtures/minimal.musicxml").replace(
            "<step>C</step><octave>4</octave>",
            "<step>A</step><alter>1</alter><octave>4</octave>",
        );
        let score = parse_score(source.as_bytes()).expect("dek fixture should parse");
        let mut warnings = score.warnings.clone();
        let svg = render_svg(&score, &mut warnings);
        let dek = svg
            .lines()
            .find(|line| line.contains("data-tone=\"10\""))
            .expect("rendered dek tonehead");

        assert!(dek.contains(&format!("d=\"{}\"", glyph_path(2))), "{dek}");
        assert!(dek.contains("rotate(180 6 9)"), "{dek}");
    }

    #[test]
    fn mixed_duration_beams_render_a_partial_third_level_hook() {
        let score = parse_score(include_bytes!(
            "../tests/fixtures/no-transposition.musicxml"
        ))
        .expect("canonical beaming fixture should parse");
        let mut warnings = score.warnings.clone();
        let svg = render_svg(&score, &mut warnings);
        assert!(
            svg.lines().any(|line| {
                line.contains("id=\"beam-7-1\"")
                    && line.contains("d=\"M494.75 39.00L507.25 39.00L507.25 42.00L494.75 42.00Z\"")
            }),
            "primary mixed-duration beam: {svg}"
        );
        assert!(
            svg.lines().any(|line| {
                line.contains("id=\"beam-7-2\"")
                    && line.contains("d=\"M494.75 43.00L507.25 43.00L507.25 46.00L494.75 46.00Z\"")
            }),
            "secondary beam should keep a 1-unit edge gap: {svg}"
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
            svg.lines().any(|line| {
                line.contains("id=\"beam-hook-7-3\"")
                    && line.contains("d=\"M499.75 47.00L507.25 47.00L507.25 50.00L499.75 50.00Z\"")
            }),
            "partial hook should stay parallel with the same gap: {svg}"
        );
    }

    #[test]
    fn canonical_fixtures_have_complete_four_four_measures() {
        for (name, source) in [
            (
                "canonical",
                include_bytes!("../tests/fixtures/canonical.musicxml").as_slice(),
            ),
            (
                "no-transposition",
                include_bytes!("../tests/fixtures/no-transposition.musicxml").as_slice(),
            ),
        ] {
            let score = parse_score(source).expect("fixture should parse");
            assert_eq!(score.parts[0].measures.len(), 9, "{name} measure count");
            for (index, measure) in score.parts[0].measures.iter().enumerate() {
                assert!(
                    measure.meter
                        == Some(Meter {
                            beats: 4,
                            beat_type: 4
                        }),
                    "{name} measure {} meter",
                    index + 1
                );
                let duration_units = measure
                    .events
                    .iter()
                    .map(|event| event.duration_units)
                    .sum::<f64>();
                assert_eq!(
                    duration_units,
                    64.0,
                    "{name} measure {} duration",
                    index + 1
                );
            }
        }
    }
}
