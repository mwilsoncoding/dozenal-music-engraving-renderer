use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

mod xml;
use xml::Element;

#[derive(Debug)]
struct Tone {
    octave: u8,
    value: u8,
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

fn parse_score(source: &[u8]) -> Result<Tone, String> {
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
    let value_errors = collect_value_errors(&elements);
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
    let [measure] = exact_children(&elements, part, &["measure"])?;
    require_attribute(&elements[measure], "number", "1")?;
    require_container_text(&elements[measure])?;
    let [attributes, note] = exact_children(&elements, measure, &["attributes", "note"])?;

    require_no_attributes(&elements[attributes])?;
    require_container_text(&elements[attributes])?;
    let attribute_children = &elements[attributes].children;
    let divisions_ids = attribute_children
        .iter()
        .copied()
        .filter(|child| elements[*child].name == "divisions")
        .collect::<Vec<_>>();
    let transpose_ids = attribute_children
        .iter()
        .copied()
        .filter(|child| elements[*child].name == "transpose")
        .collect::<Vec<_>>();
    if divisions_ids.len() != 1
        || transpose_ids.len() > 1
        || attribute_children
            .iter()
            .any(|child| !matches!(elements[*child].name.as_str(), "divisions" | "transpose"))
    {
        return Err(element_diagnostic(
            &elements[attributes],
            "unsupported shape: <attributes> requires one <divisions> and optional <transpose>"
                .to_owned(),
        ));
    }
    let divisions = divisions_ids[0];
    if leaf_text(&elements[divisions])? != "1" {
        return Err(element_diagnostic(
            &elements[divisions],
            "unsupported score: divisions must be 1".to_owned(),
        ));
    }
    let transposition = transpose_ids
        .first()
        .map(|transpose| transpose_semitones(&elements, *transpose))
        .transpose()?
        .unwrap_or(0);

    require_no_attributes(&elements[note])?;
    require_container_text(&elements[note])?;
    let [pitch, duration, voice, note_type] =
        exact_children(&elements, note, &["pitch", "duration", "voice", "type"])?;
    if leaf_text(&elements[duration])? != "1"
        || leaf_text(&elements[voice])? != "1"
        || leaf_text(&elements[note_type])? != "quarter"
    {
        return Err(element_diagnostic(
            &elements[note],
            "unsupported score: only one quarter-note event in voice 1 is supported".to_owned(),
        ));
    }

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

fn collect_value_errors(elements: &[Element]) -> Vec<String> {
    let mut errors = Vec::new();
    for element in elements {
        if !element.children.is_empty() || !element.attributes.is_empty() {
            continue;
        }
        let value = element.text.trim();
        let message = match element.name.as_str() {
            "divisions" if value != "1" => Some("unsupported score: <divisions> must be 1"),
            "duration" if value != "1" => Some("unsupported score: <duration> must be 1"),
            "voice" if value != "1" => Some("unsupported score: <voice> must be 1"),
            "type" if value != "quarter" => Some("unsupported score: <type> must be quarter"),
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
        return Err(format!(
            "unsupported score: <{}> {name} must be {expected:?}",
            element.name
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

fn render_svg(tone: &Tone) -> String {
    let octave_y = lane_y(tone.octave);
    let note_y = octave_y + 9;
    let stem_end = if tone.octave < 5 {
        octave_y - 20
    } else {
        octave_y + 37
    };
    let stem = format!(
        "<path id=\"stem\" d=\"M102 {note_y}V{stem_end}\" stroke=\"#171717\" stroke-width=\"1.5\"/>"
    );
    let mut ledger_lines = String::new();
    let ledger_octaves: Vec<u8> = if tone.octave < 4 {
        (tone.octave..=3).rev().collect()
    } else if tone.octave > 5 {
        (6..=tone.octave).collect()
    } else {
        Vec::new()
    };
    for octave in ledger_octaves {
        let ledger_y = lane_y(octave) + 9;
        ledger_lines.push_str(&format!(
            "<path id=\"ledger-line-octave-{octave}\" d=\"M84 {ledger_y}H112\" stroke=\"#171717\"/>"
        ));
    }
    let view_top = 0.min(octave_y - 4);
    let view_bottom = 88.max(octave_y + 20);
    let view_height = view_bottom - view_top;
    format!(
		"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 {view_top} 140 {view_height}\" role=\"img\">\
		 <path id=\"staff-line-1\" d=\"M12 28H128\" stroke=\"#171717\"/>\
		 <path id=\"staff-line-2\" d=\"M12 48H128\" stroke=\"#171717\"/>\
		 <path id=\"staff-line-3\" d=\"M12 68H128\" stroke=\"#171717\"/>\
		 <path id=\"octave-5\" d=\"{}\" transform=\"translate(26 30)\" fill=\"none\" stroke=\"#171717\" stroke-width=\"1.5\"/>\
		 <path id=\"octave-4\" d=\"{}\" transform=\"translate(26 50)\" fill=\"none\" stroke=\"#171717\" stroke-width=\"1.5\"/>\
		 {ledger_lines}{stem}\
		 <path id=\"tonehead\" data-tone=\"{}\" data-octave=\"{}\" transform=\"translate(92 {octave_y})\" d=\"{}\" fill=\"none\" stroke=\"#171717\" stroke-width=\"1.8\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\
		 </svg>\n",
		glyph_path(5),
		glyph_path(4),
		tone.value,
		tone.octave,
		glyph_path(tone.value)
	)
}

fn lane_y(octave: u8) -> i32 {
    29 + (5 - i32::from(octave)) * 20
}

fn glyph_path(value: u8) -> &'static str {
    match value {
        0 => "M6 2C1 2 1 16 6 16C11 16 11 2 6 2Z",
        1 => "M4 5L8 2V16",
        2 => "M2 2H10V9H2V16H10",
        3 => "M2 2H10V16H2M2 9H10",
        4 => "M2 2V9H10M10 2V16",
        5 => "M10 2H2V9H10V16H2",
        6 => "M10 2H2V16H10V9H2",
        7 => "M2 2H10L4 16",
        8 => "M2 2H10V16H2ZM2 9H10",
        9 => "M10 9H2V2H10V16H2",
        10 => "M10 16H2V9H10V2H2",
        11 => "M10 16H2V2H10M10 9H2",
        _ => unreachable!("tone values are reduced to one dozenal digit"),
    }
}
