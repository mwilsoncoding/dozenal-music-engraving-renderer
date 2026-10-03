use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process;

mod xml;
use xml::Element;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tone {
    octave: u8,
    value: u8,
}

struct Event {
    tones: Vec<ToneEvent>,
    duration_units: u32,
    duration_name: &'static str,
    dots: u8,
    beams: Vec<BeamMark>,
}

struct ToneEvent {
    tone: Tone,
    tie_start: bool,
    tie_stop: bool,
}

struct ParsedNote {
    event: Event,
    chord_marker: Option<usize>,
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

struct Measure {
    events: Vec<Event>,
    meter: Option<Meter>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Meter {
    beats: u8,
    beat_type: u8,
}

struct Score {
    measures: Vec<Measure>,
}

fn main() {
    if let Err(message) = run() {
        eprintln!("domunor: {message}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let (input_path, output_path) = parse_args()?;
    let source = xml::read_file(&input_path)
        .map_err(|error| format!("cannot read {}: {error}", input_path.display()))?;
    let score =
        parse_score(&source).map_err(|error| format!("{}: {error}", input_path.display()))?;
    let svg = render_svg(&score);

    if output_path
        .extension()
        .is_none_or(|extension| extension != "svg")
    {
        return Err("output path must have the .svg extension".to_owned());
    }

    fs::write(&output_path, svg)
        .map_err(|error| format!("cannot write {}: {error}", output_path.display()))
}

fn parse_args() -> Result<(PathBuf, PathBuf), String> {
    let mut args = env::args_os().skip(1);
    let mut input = None;
    let mut output = None;

    while let Some(argument) = args.next() {
        if argument == "--output" {
            if output.is_some() {
                return Err("--output may only be specified once".to_owned());
            }
            output = Some(PathBuf::from(
                args.next()
                    .ok_or_else(|| "--output requires a path".to_owned())?,
            ));
        } else if argument.to_string_lossy().starts_with('-') {
            return Err(format!("unsupported option {}", argument.to_string_lossy()));
        } else if input.replace(PathBuf::from(argument)).is_some() {
            return Err("provide exactly one input file".to_owned());
        }
    }

    let input = input.ok_or_else(|| "usage: domunor INPUT [--output FILE.svg]".to_owned())?;
    let output = output.unwrap_or_else(|| {
        let mut path = input.clone();
        path.set_extension("svg");
        path
    });
    Ok((input, output))
}

fn parse_score(source: &[u8]) -> Result<Score, String> {
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

    let [part_list, part] = exact_children(&elements, root, &["part-list", "part"])?;
    require_no_attributes(&elements[part_list])?;
    require_container_text(&elements[part_list])?;
    let [score_part] = exact_children(&elements, part_list, &["score-part"])?;
    let part_id = required_attribute(&elements[score_part], "id")?.to_owned();
    require_container_text(&elements[score_part])?;
    let [part_name] = exact_children(&elements, score_part, &["part-name"])?;
    leaf_text(&elements[part_name])?;

    if required_attribute(&elements[part], "id")? != part_id {
        return Err(element_diagnostic(
            &elements[part],
            "unsupported shape: part id does not match score-part id".to_owned(),
        ));
    }
    require_container_text(&elements[part])?;
    let mut divisions = None;
    let mut transposition = 0;
    let mut active_meter = None;
    let mut pending_ties = Vec::new();
    let mut active_beams = [false; 5];
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
        let mut event_sources = Vec::new();
        let mut chord_target = None;
        for child in &measure_element.children {
            let element = &elements[*child];
            match element.name.as_str() {
                "attributes" => {
                    require_no_attributes(element)?;
                    require_container_text(element)?;
                    let mut saw_divisions = false;
                    let mut saw_transpose = false;
                    let mut saw_time = false;
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
                            "transpose" if !saw_transpose => {
                                transposition = transpose_semitones(&elements, *attribute)?;
                                saw_transpose = true;
                            }
                            "time" if !saw_time => {
                                active_meter = Some(parse_time(&elements, *attribute)?);
                                saw_time = true;
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
                    let parsed = parse_note(&elements, *child, divisions, transposition)?;
                    if let Some(chord_marker) = parsed.chord_marker {
                        let Some(target) = chord_target else {
                            return Err(element_diagnostic(
                                &elements[chord_marker],
                                "<chord/> marker requires a preceding pitched event".to_owned(),
                            ));
                        };
                        let event: &mut Event = &mut events[target];
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
                        events.push(parsed.event);
                        event_sources.push(*child);
                        chord_target = events
                            .last()
                            .filter(|event| !event.tones.is_empty())
                            .map(|_| events.len() - 1);
                    }
                }
                _ => {
                    return Err(element_diagnostic(
                        element,
                        format!("unsupported measure element <{}>", element.name),
                    ));
                }
            }
        }
        if events.is_empty() {
            return Err(element_diagnostic(
                measure_element,
                "unsupported score: a measure must contain at least one event".to_owned(),
            ));
        }
        for (event, source) in events.iter().zip(event_sources) {
            validate_beam_group(&elements[source], event, &mut active_beams)?;
            validate_event_ties(&elements[source], event, &mut pending_ties)?;
        }
        measures.push(Measure {
            events,
            meter: active_meter,
        });
    }
    if measures.is_empty() {
        return Err(element_diagnostic(
            &elements[part],
            "unsupported score: <part> must contain at least one measure".to_owned(),
        ));
    }
    if !pending_ties.is_empty() {
        return Err(element_diagnostic(
            &elements[part],
            "tie start has no matching tie stop".to_owned(),
        ));
    }
    if active_beams.iter().skip(1).any(|active| *active) {
        return Err(element_diagnostic(
            &elements[part],
            "beam group is missing an end marker".to_owned(),
        ));
    }

    Ok(Score { measures })
}

fn parse_time(elements: &[Element], time: usize) -> Result<Meter, String> {
    let element = &elements[time];
    require_no_attributes(element)?;
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

fn parse_note(
    elements: &[Element],
    note: usize,
    divisions: u32,
    transposition: i32,
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
    let voice = *children.get(position).ok_or_else(|| {
        element_diagnostic(note_element, "note event requires <voice>".to_owned())
    })?;
    position += 1;
    let note_type = *children
        .get(position)
        .ok_or_else(|| element_diagnostic(note_element, "note event requires <type>".to_owned()))?;
    position += 1;
    let dot = if children
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
        Some(dot)
    } else {
        None
    };
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
    leaf_text(&elements[voice])?;
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
    let dots = usize::from(dot.is_some());
    if dots > 0 && !matches!(duration_name, "quarter" | "half") {
        return Err(element_diagnostic(
            &elements[note_type],
            "only dotted quarter and dotted half notes are supported".to_owned(),
        ));
    }
    if is_rest && dots != 0 {
        return Err(element_diagnostic(
            &elements[dot.expect("dot exists")],
            "dotted rests are not supported".to_owned(),
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
    let duration_units = if dots == 0 {
        base_units
    } else {
        base_units + base_units / 2
    };
    let duration_marks = if dots == 1 {
        if duration_name == "quarter" {
            1
        } else {
            3
        }
    } else {
        mark_count
    };
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
    if u64::from(duration_ticks) * 16 != u64::from(divisions) * u64::from(duration_units) {
        return Err(element_diagnostic(
            &elements[duration],
            format!("duration {duration_ticks} does not match <type> at divisions {divisions}"),
        ));
    }
    let tones = if is_rest {
        Vec::new()
    } else {
        vec![ToneEvent {
            tone: parse_pitch(elements, event_element, transposition)?,
            tie_start,
            tie_stop,
        }]
    };
    Ok(ParsedNote {
        event: Event {
            tones,
            duration_units,
            duration_name,
            dots: duration_marks,
            beams,
        },
        chord_marker,
    })
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
    pending_ties: &mut Vec<Tone>,
) -> Result<(), String> {
    let mut unmatched = std::mem::take(pending_ties);
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
    pending_ties.extend(
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
    require_no_attributes(&elements[transpose])?;
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
    let mut voice_id = None;
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
        } else if let Some(existing_id) = voice_id {
            if existing_id != value {
                errors.push(element_diagnostic(
                    element,
                    "unsupported score: multiple voices are not supported".to_owned(),
                ));
            }
        } else {
            voice_id = Some(value);
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
    for (index, element) in elements.iter().enumerate() {
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
        if dots > 1 || (dots == 1 && !matches!(elements[note_type].text.trim(), "quarter" | "half"))
        {
            continue;
        }
        let duration_ticks = elements[duration].text.trim().parse::<u32>().ok();
        let Some(duration_ticks) = duration_ticks.filter(|value| *value > 0) else {
            continue;
        };
        let duration_units = if dots == 1 {
            base_units + base_units / 2
        } else {
            base_units
        };
        if u64::from(duration_ticks) * 16 != u64::from(divisions) * duration_units {
            errors.push(element_diagnostic(
                &elements[duration],
                format!("duration {duration_ticks} does not match <type> at divisions {divisions}"),
            ));
        }
        let _ = index;
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
    Tie,
}

#[derive(Clone, Copy)]
enum PathMetadata {
    Event(usize),
    ToneIndex(usize),
    Tone(u8),
    Octave(u8),
    Duration(&'static str),
    BeamLevel(u8),
}

#[derive(Clone, Copy)]
enum PathStyle {
    StaffLine,
    MediumStroke,
    MediumOutline,
    FilledGlyph,
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
    scale: u8,
    rotation: Option<(i32, i32, i32)>,
    path_data: String,
    style: PathStyle,
}

impl PathGeometry {
    fn new(transform: Option<(i32, i32)>, path_data: impl Into<String>, style: PathStyle) -> Self {
        Self {
            transform,
            scale: 1,
            rotation: None,
            path_data: path_data.into(),
            style,
        }
    }

    fn scaled(mut self, scale: u8) -> Self {
        self.scale = scale;
        self
    }

    fn rotated_180_if(mut self, condition: bool) -> Self {
        if condition {
            self.rotation = Some((180, 6, 9));
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

struct EventGeometry {
    primary: Vec<ComponentBounds>,
    incoming: Vec<ComponentBounds>,
}

impl EventGeometry {
    fn all(&self) -> impl Iterator<Item = &ComponentBounds> {
        self.primary.iter().chain(&self.incoming)
    }
}

fn event_geometry(
    event: &Event,
    x: i32,
    active_beams: &[Option<(i32, i32)>; 5],
    pending_ties: &[(Tone, i32, i32)],
) -> EventGeometry {
    let mut primary = Vec::new();
    let mut incoming = Vec::new();
    if event.tones.is_empty() {
        let (left, top, right, bottom) = match event.duration_name {
            "whole" => (2.0, 5.0, 10.0, 8.0),
            "half" => (2.0, 8.0, 10.0, 11.0),
            "quarter" | "eighth" | "16th" | "32nd" => (3.0, 1.0, 12.0, 17.0),
            "64th" => (3.0, 1.0, 12.0, 18.0),
            _ => unreachable!("rest duration was validated while parsing"),
        };
        primary.push(ComponentBounds::filled(
            f64::from(x) + left,
            40.0 + top,
            f64::from(x) + right,
            40.0 + bottom,
        ));
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

    let stem_tone_index = event
        .tones
        .iter()
        .enumerate()
        .min_by_key(|(_, tone)| lane_y(tone.tone.octave))
        .map(|(tone_index, _)| tone_index)
        .expect("pitched event has a tone");
    let stem_octave_y = lane_y(event.tones[stem_tone_index].tone.octave);
    let stem_start = stem_octave_y - 1;
    let stem_end = stem_octave_y - 30;
    let stem_x = tone_xs[stem_tone_index] + 6;
    if event.duration_name != "whole" {
        primary.push(ComponentBounds::stroked(
            f64::from(stem_x),
            f64::from(stem_end),
            f64::from(stem_x),
            f64::from(stem_start),
            1.5,
        ));
        let flag_count = match event.duration_name {
            "eighth" => 1,
            "16th" => 2,
            "32nd" => 3,
            "64th" => 4,
            _ => 0,
        };
        for flag in 0..flag_count {
            let level = (flag + 1) as u8;
            if event.beams.iter().any(|beam| beam.level == level) {
                continue;
            }
            let flag_y = stem_end + flag * 3;
            primary.push(ComponentBounds::stroked(
                f64::from(stem_x),
                f64::from(flag_y),
                f64::from(stem_x + 12),
                f64::from(flag_y + 11),
                1.5,
            ));
        }
    }

    for beam in &event.beams {
        let level = usize::from(beam.level);
        let offset = i32::from(beam.level - 1) * 4;
        let beam_y = stem_end + offset;
        match beam.state {
            BeamState::Begin => {}
            BeamState::Continue | BeamState::End => {
                if let Some((start_x, start_y)) = active_beams[level] {
                    incoming.push(ComponentBounds::stroked(
                        f64::from(start_x.min(stem_x)),
                        f64::from(start_y.min(beam_y)),
                        f64::from(start_x.max(stem_x)),
                        f64::from(start_y.max(beam_y)),
                        3.0,
                    ));
                }
            }
            BeamState::ForwardHook | BeamState::BackwardHook => {
                let direction = if beam.state == BeamState::ForwardHook {
                    1
                } else {
                    -1
                };
                let end_x = stem_x + direction * 9;
                primary.push(ComponentBounds::stroked(
                    f64::from(stem_x.min(end_x)),
                    f64::from(beam_y),
                    f64::from(stem_x.max(end_x)),
                    f64::from(beam_y + 4),
                    3.0,
                ));
            }
        }
    }

    let first_tone = &event.tones[0].tone;
    for mark in 0..event.dots {
        let (mark_x, mark_y) = match (event.duration_name, event.dots, mark) {
            ("quarter", 1, _) => (x + 14, lane_y(first_tone.octave) + 12),
            ("half", 2, _) => (x + 14, lane_y(first_tone.octave) + 5 + i32::from(mark) * 7),
            ("half", 3, 0..=1) => (x + 14, lane_y(first_tone.octave) + 5 + i32::from(mark) * 7),
            ("half", 3, _) => (x + 19, lane_y(first_tone.octave) + 12),
            _ => (x + 14, lane_y(first_tone.octave) + 9),
        };
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
    view_top: i32,
    view_width: i32,
    view_height: i32,
    commands: Vec<DrawCommand>,
}

impl Scene {
    fn new(view_top: i32, view_width: i32, view_height: i32) -> Self {
        Self {
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
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 {} {} {}\" role=\"img\">",
            self.view_top, self.view_width, self.view_height
        )
        .expect("writing to a String cannot fail");
        writeln!(
            svg,
            "<rect id=\"background\" x=\"0\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#fff\"/>",
            self.view_top, self.view_width, self.view_height
        )
        .expect("writing to a String cannot fail");
        for layer in [
            DrawLayer::Staff,
            DrawLayer::Barline,
            DrawLayer::Beam,
            DrawLayer::Event,
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
                PathClass::Tie => "tie",
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
                PathMetadata::BeamLevel(value) => write!(svg, " data-level=\"{value}\""),
            }
            .expect("writing to a String cannot fail");
        }
        let mut transforms = String::new();
        if let Some((x, y)) = command.geometry.transform {
            write!(transforms, "translate({x} {y})").expect("writing to a String cannot fail");
        }
        if command.geometry.scale != 1 {
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
            PathStyle::MediumOutline => {
                svg.push_str(" fill=\"none\" stroke=\"#171717\" stroke-width=\"1.5\"")
            }
            PathStyle::FilledGlyph => svg.push_str(" fill=\"#171717\" fill-rule=\"evenodd\""),
            PathStyle::Beam => svg.push_str(
                " fill=\"none\" stroke=\"#171717\" stroke-width=\"3\" stroke-linecap=\"square\"",
            ),
            PathStyle::Solid => svg.push_str(" fill=\"#171717\""),
        }
        svg.push_str("/>\n");
    }
}

fn render_svg(score: &Score) -> String {
    let mut scene = Scene::new(0, 0, 0);
    let mut x = 92;
    let mut right = 128;
    let mut view_top = 0;
    let mut view_bottom = 128;
    let mut index = 0;
    let mut active_meter = None;
    let mut meter_index = 0;
    let mut pending_tie_paths: Vec<(Tone, i32, i32)> = Vec::new();
    let mut active_beams = [None; 5];
    let mut final_barline_x = 0;
    for (measure_index, measure) in score.measures.iter().enumerate() {
        let mut previous_event_geometry = None;
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
                let meter_x = if measure_index == 0 { x - 24 } else { x };
                scene.add_path(
                    DrawLayer::Event,
                    numerator_id,
                    None,
                    Vec::new(),
                    PathGeometry::new(
                        Some((meter_x, 31)),
                        time_signature_glyph_path(meter.beats),
                        PathStyle::FilledGlyph,
                    )
                    .scaled(2)
                    .rotated_180_if(meter.beats == 10),
                );
                scene.add_path(
                    DrawLayer::Event,
                    denominator_id,
                    None,
                    Vec::new(),
                    PathGeometry::new(
                        Some((meter_x, 71)),
                        time_signature_glyph_path(meter.beat_type),
                        PathStyle::FilledGlyph,
                    )
                    .scaled(2)
                    .rotated_180_if(meter.beat_type == 10),
                );
                x += 28;
                meter_index += 1;
            }
            active_meter = Some(meter);
        }
        for event in &measure.events {
            let preferred_x = x;
            let preferred_geometry =
                event_geometry(event, preferred_x, &active_beams, &pending_tie_paths);
            if let Some(previous) = &previous_event_geometry {
                x = next_event_x(preferred_x, previous, &preferred_geometry);
            }
            let geometry = if x == preferred_x {
                preferred_geometry
            } else {
                event_geometry(event, x, &active_beams, &pending_tie_paths)
            };
            for component in geometry.all() {
                right = right.max(component.right.ceil() as i32);
            }
            if event.tones.is_empty() {
                scene.add_path(
                    DrawLayer::Event,
                    format!("rest-{index}"),
                    Some(PathClass::Rest),
                    vec![PathMetadata::Duration(event.duration_name)],
                    PathGeometry::new(
                        Some((x, 40)),
                        rest_glyph_path(event.duration_name),
                        PathStyle::FilledGlyph,
                    ),
                );
                right = right.max(x + 24);
                previous_event_geometry = Some(geometry);
                x += (event.duration_units * 5).div_ceil(2) as i32;
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
            let stem_tone_index = event
                .tones
                .iter()
                .enumerate()
                .min_by_key(|(_, tone)| lane_y(tone.tone.octave))
                .map(|(tone_index, _)| tone_index)
                .expect("pitched event has a tone");
            let stem_octave_y = lane_y(event.tones[stem_tone_index].tone.octave);
            let stem_start = stem_octave_y - 1;
            let stem_end = stem_octave_y - 30;
            let stem_id = if index == 0 {
                "stem".to_owned()
            } else {
                format!("stem-{index}")
            };
            let stem_x = tone_xs[stem_tone_index] + 6;
            if event.duration_name != "whole" {
                view_top = view_top.min(stem_end);
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
                let flag_count = match event.duration_name {
                    "eighth" => 1,
                    "16th" => 2,
                    "32nd" => 3,
                    "64th" => 4,
                    _ => 0,
                };
                for flag in 0..flag_count {
                    let level = (flag + 1) as u8;
                    if event.beams.iter().any(|beam| beam.level == level) {
                        continue;
                    }
                    let flag_y = stem_end + flag * 3;
                    scene.add_path(
                        DrawLayer::Event,
                        format!("flag-{index}-{flag}"),
                        Some(PathClass::Flag),
                        Vec::new(),
                        PathGeometry::new(
                            Some((stem_x, flag_y)),
                            flag_glyph_path(),
                            PathStyle::FilledGlyph,
                        ),
                    );
                }
            }
            for beam in &event.beams {
                let level = usize::from(beam.level);
                let offset = i32::from(beam.level - 1) * 4;
                let beam_y = stem_end + offset;
                match beam.state {
                    BeamState::Begin => active_beams[level] = Some((stem_x, beam_y)),
                    BeamState::Continue | BeamState::End => {
                        let (start_x, start_y) =
                            active_beams[level].expect("beam group is validated before rendering");
                        scene.add_path(
                            DrawLayer::Beam,
                            format!("beam-{index}-{level}"),
                            Some(PathClass::Beam),
                            vec![PathMetadata::BeamLevel(beam.level)],
                            PathGeometry::new(
                                None,
                                format!("M{start_x} {start_y}L{stem_x} {beam_y}"),
                                PathStyle::Beam,
                            ),
                        );
                        if beam.state == BeamState::Continue {
                            active_beams[level] = Some((stem_x, beam_y));
                        } else {
                            active_beams[level] = None;
                        }
                    }
                    BeamState::ForwardHook | BeamState::BackwardHook => {
                        let direction = if beam.state == BeamState::ForwardHook {
                            1
                        } else {
                            -1
                        };
                        let end_x = stem_x + direction * 9;
                        let end_y = beam_y + 4;
                        scene.add_path(
                            DrawLayer::Beam,
                            format!("beam-hook-{index}-{level}"),
                            Some(PathClass::BeamHook),
                            vec![PathMetadata::BeamLevel(beam.level)],
                            PathGeometry::new(
                                None,
                                format!("M{stem_x} {beam_y}L{end_x} {end_y}"),
                                PathStyle::Beam,
                            ),
                        );
                    }
                }
            }
            for mark in 0..event.dots {
                let (mark_x, mark_y) = match (event.duration_name, event.dots, mark) {
                    ("quarter", 1, _) => (x + 14, lane_y(first_tone.octave) + 12),
                    ("half", 2, _) => (x + 14, lane_y(first_tone.octave) + 5 + i32::from(mark) * 7),
                    ("half", 3, 0..=1) => {
                        (x + 14, lane_y(first_tone.octave) + 5 + i32::from(mark) * 7)
                    }
                    ("half", 3, _) => (x + 19, lane_y(first_tone.octave) + 12),
                    _ => (x + 14, lane_y(first_tone.octave) + 9),
                };
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
                    if let Some(position) = pending_tie_paths
                        .iter()
                        .position(|(pending_tone, _, _)| *pending_tone == tone)
                    {
                        let (_, start_x, start_y) = pending_tie_paths.remove(position);
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
                    pending_tie_paths.push((tone, tone_x + 8, tie_y));
                }
                view_top = view_top.min(octave_y - 20);
                view_bottom = view_bottom.max(octave_y + 37);
            }
            right = right.max(x + 28);
            previous_event_geometry = Some(geometry);
            x += (event.duration_units * 5).div_ceil(2) as i32;
            index += 1;
        }
        let mut barline_x = x;
        if let Some(geometry) = &previous_event_geometry {
            let barline_span = ComponentBounds::filled(0.0, 28.0, 0.0, 108.0);
            for component in geometry.all() {
                if component.overlaps_vertically(barline_span) {
                    barline_x = barline_x.max((component.right + 2.5).ceil() as i32);
                }
            }
        }
        scene.add_path(
            DrawLayer::Barline,
            format!("barline-{}", measure_index + 1),
            None,
            Vec::new(),
            PathGeometry::new(None, format!("M{barline_x} 28V108"), PathStyle::StaffLine),
        );
        final_barline_x = barline_x;
        right = right.max(barline_x + 12);
        x = barline_x + 3;
    }
    let view_width = right + 12;
    scene.view_top = view_top;
    scene.view_width = view_width;
    scene.view_height = view_bottom - view_top;
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
        PathGeometry::new(Some((26, 50)), glyph_path(5), PathStyle::FilledGlyph),
    );
    scene.add_path(
        DrawLayer::Staff,
        "octave-4",
        None,
        Vec::new(),
        PathGeometry::new(Some((26, 70)), glyph_path(4), PathStyle::FilledGlyph),
    );
    scene.to_svg()
}

fn rest_glyph_path(duration_name: &str) -> &'static str {
    match duration_name {
        "whole" => "M2 5H10V8H2Z",
        "half" => "M2 8H10V11H2Z",
        "quarter" => "M4 1C6 2 9 3 10 5L6 8L9 11L7 15C6.4 16.1 5.3 16.8 3.8 17L3 15C5 13.5 5.2 12.3 3.2 10L6.5 7L3 4Z",
        "eighth" => "M4 1C6 2 9 3 10 5L6 8L9 11L7 15C6.4 16.1 5.3 16.8 3.8 17L3 15C5 13.5 5.2 12.3 3.2 10L6.5 7L3 4ZM7 10C9 10.8 11 11.8 11.5 13L10 15C9.3 13.4 8.2 12.7 6.5 12Z",
        "16th" => "M4 1C6 2 9 3 10 5L6 8L9 11L7 15C6.4 16.1 5.3 16.8 3.8 17L3 15C5 13.5 5.2 12.3 3.2 10L6.5 7L3 4ZM6.5 8C8.5 8.8 10.5 9.8 11 11L9.5 13C8.8 11.4 7.7 10.7 6 10ZM7 12C9 12.8 11 13.8 11.5 15L10 17C9.3 15.4 8.2 14.7 6.5 14Z",
        "32nd" => "M4 1C6 2 9 3 10 5L6 8L9 11L7 15C6.4 16.1 5.3 16.8 3.8 17L3 15C5 13.5 5.2 12.3 3.2 10L6.5 7L3 4ZM6 6C8 6.8 10 7.8 10.5 9L9 11C8.3 9.4 7.2 8.7 5.5 8ZM6.5 9C8.5 9.8 10.5 10.8 11 12L9.5 14C8.8 12.4 7.7 11.7 6 11ZM7 12C9 12.8 11 13.8 11.5 15L10 17C9.3 15.4 8.2 14.7 6.5 14Z",
        "64th" => "M4 1C6 2 9 3 10 5L6 8L9 11L7 15C6.4 16.1 5.3 16.8 3.8 17L3 15C5 13.5 5.2 12.3 3.2 10L6.5 7L3 4ZM5.5 4C7.5 4.8 9.5 5.8 10 7L8.5 9C7.8 7.4 6.7 6.7 5 6ZM6 7C8 7.8 10 8.8 10.5 10L9 12C8.3 10.4 7.2 9.7 5.5 9ZM6.5 10C8.5 10.8 10.5 11.8 11 13L9.5 15C8.8 13.4 7.7 12.7 6 12ZM7 13C9 13.8 11 14.8 11.5 16L10 18C9.3 16.4 8.2 15.7 6.5 15Z",
        _ => unreachable!("duration was validated while parsing"),
    }
}

fn flag_glyph_path() -> &'static str {
    "M0 0C3 0.7 7.7 1.7 9.4 3.6C11 5.4 9.8 8.2 6.8 10.5L4.4 8.8C6.4 6.9 7.1 5.5 6.1 4.5C5.2 3.6 2.7 3 0 2.5Z"
}

fn lane_y(octave: u8) -> i32 {
    match octave {
        0..=4 => 70 + (4 - i32::from(octave)) * 20,
        _ => 50 - (i32::from(octave) - 5) * 20,
    }
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
        let svg = render_svg(&score);
        let dek = svg
            .lines()
            .find(|line| line.contains("data-tone=\"10\""))
            .expect("rendered dek tonehead");

        assert!(dek.contains(&format!("d=\"{}\"", glyph_path(2))), "{dek}");
        assert!(dek.contains("rotate(180 6 9)"), "{dek}");
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
            assert_eq!(score.measures.len(), 9, "{name} measure count");
            for (index, measure) in score.measures.iter().enumerate() {
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
                    .sum::<u32>();
                assert_eq!(duration_units, 64, "{name} measure {} duration", index + 1);
            }
        }
    }
}
