use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

#[derive(Debug)]
struct Element {
    name: String,
    attributes: Vec<(String, String)>,
    text: String,
    children: Vec<usize>,
}

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
    let source = fs::read_to_string(&input_path)
        .map_err(|error| format!("cannot read {}: {error}", input_path.display()))?;
    let score = parse_score(&source)?;
    let svg = render_svg(&score);

    if let Some(extension) = output_path.extension() {
        if extension != "svg" {
            return Err("output path must have the .svg extension".to_owned());
        }
    } else {
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

fn parse_score(source: &str) -> Result<Tone, String> {
    let elements = parse_xml_subset(source)?;
    let root = root_element(&elements)?;
    if elements[root].name != "score-partwise" {
        return Err(format!(
            "unsupported root <{}>; expected <score-partwise>",
            elements[root].name
        ));
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
        return Err("unsupported shape: part id does not match score-part id".to_owned());
    }
    require_container_text(&elements[part])?;
    let [measure] = exact_children(&elements, part, &["measure"])?;
    require_attribute(&elements[measure], "number", "1")?;
    require_container_text(&elements[measure])?;
    let [attributes, note] = exact_children(&elements, measure, &["attributes", "note"])?;

    require_no_attributes(&elements[attributes])?;
    require_container_text(&elements[attributes])?;
    let [divisions] = exact_children(&elements, attributes, &["divisions"])?;
    if leaf_text(&elements[divisions])? != "1" {
        return Err("unsupported score: divisions must be 1".to_owned());
    }

    require_no_attributes(&elements[note])?;
    require_container_text(&elements[note])?;
    let [pitch, duration, voice, note_type] =
        exact_children(&elements, note, &["pitch", "duration", "voice", "type"])?;
    if leaf_text(&elements[duration])? != "1"
        || leaf_text(&elements[voice])? != "1"
        || leaf_text(&elements[note_type])? != "quarter"
    {
        return Err(
            "unsupported score: only one quarter-note event in voice 1 is supported".to_owned(),
        );
    }

    require_no_attributes(&elements[pitch])?;
    require_container_text(&elements[pitch])?;
    let pitch_children = &elements[pitch].children;
    let (step_id, octave_id) = match pitch_children.as_slice() {
		[step_id, octave_id] if elements[*step_id].name == "step" => (*step_id, *octave_id),
		[step_id, alter_id, octave_id]
			if elements[*step_id].name == "step" && elements[*alter_id].name == "alter" =>
		{
			if leaf_text(&elements[*alter_id])? != "0" {
				return Err("unsupported pitch: only natural tones are supported".to_owned());
			}
			(*step_id, *octave_id)
		}
		_ => {
			return Err(
				"unsupported shape: <pitch> must contain <step>, optional natural <alter>, and <octave>"
					.to_owned(),
			)
		}
	};
    if elements[octave_id].name != "octave" {
        return Err("unsupported shape: <pitch> must end with <octave>".to_owned());
    }

    let step = leaf_text(&elements[step_id])?;
    let value = match step {
        "C" => 0,
        "D" => 2,
        "E" => 4,
        "F" => 5,
        "G" => 7,
        "A" => 9,
        "B" => 11,
        _ => return Err(format!("unsupported pitch step {step:?}")),
    };
    let octave = leaf_text(&elements[octave_id])?
        .parse::<u8>()
        .map_err(|_| "unsupported pitch octave; expected 4 or 5".to_owned())?;
    if !(4..=5).contains(&octave) {
        return Err("unsupported pitch octave; this bootstrap supports octaves 4 and 5".to_owned());
    }

    Ok(Tone { octave, value })
}

fn parse_xml_subset(source: &str) -> Result<Vec<Element>, String> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut elements = Vec::new();
    let mut stack = Vec::<usize>::new();
    let mut cursor = 0;
    let mut declaration_seen = false;

    while cursor < source.len() {
        let Some(relative_open) = source[cursor..].find('<') else {
            append_text(&mut elements, &stack, &source[cursor..])?;
            break;
        };
        let open = cursor + relative_open;
        append_text(&mut elements, &stack, &source[cursor..open])?;
        let relative_close = source[open..]
            .find('>')
            .ok_or_else(|| "malformed XML: unterminated tag".to_owned())?;
        let close = open + relative_close;
        let raw = source[open + 1..close].trim();
        cursor = close + 1;

        if raw.starts_with("?xml") {
            if declaration_seen || !elements.is_empty() || !stack.is_empty() || !raw.ends_with('?')
            {
                return Err("unsupported XML declaration placement or syntax".to_owned());
            }
            if !matches!(
                raw,
                "?xml version=\"1.0\" encoding=\"UTF-8\"?"
                    | "?xml version='1.0' encoding='UTF-8'?"
                    | "?xml version=\"1.0\"?"
                    | "?xml version='1.0'?"
            ) {
                return Err("unsupported XML declaration; expected XML 1.0 UTF-8".to_owned());
            }
            declaration_seen = true;
            continue;
        }
        if raw.starts_with('!') {
            return Err(
                "unsupported XML construct; DTDs, comments, and CDATA are not supported".to_owned(),
            );
        }
        if raw.starts_with('?') {
            return Err("unsupported XML processing instruction".to_owned());
        }

        if let Some(closing) = raw.strip_prefix('/') {
            let name = closing.trim();
            if name.is_empty() || name.chars().any(char::is_whitespace) {
                return Err("malformed XML: invalid closing tag".to_owned());
            }
            let Some(open_element) = stack.pop() else {
                return Err(format!("malformed XML: unexpected closing tag </{name}>"));
            };
            if elements[open_element].name != name {
                return Err(format!(
                    "malformed XML: expected </{}>, found </{name}>",
                    elements[open_element].name
                ));
            }
            continue;
        }

        let self_closing = raw.ends_with('/');
        let body = if self_closing {
            raw[..raw.len() - 1].trim_end()
        } else {
            raw
        };
        let (name, attributes) = parse_open_tag(body)?;
        let index = elements.len();
        elements.push(Element {
            name,
            attributes,
            text: String::new(),
            children: Vec::new(),
        });
        if let Some(parent) = stack.last().copied() {
            elements[parent].children.push(index);
        }
        if !self_closing {
            stack.push(index);
        }
    }

    if let Some(open_element) = stack.last().copied() {
        return Err(format!(
            "malformed XML: unclosed <{}>",
            elements[open_element].name
        ));
    }
    root_element(&elements)?;
    Ok(elements)
}

fn append_text(elements: &mut [Element], stack: &[usize], text: &str) -> Result<(), String> {
    if text.contains('&') {
        return Err("unsupported XML entity reference".to_owned());
    }
    if let Some(parent) = stack.last() {
        elements[*parent].text.push_str(text);
    } else if !text.trim().is_empty() {
        return Err("malformed XML: text outside the document element".to_owned());
    }
    Ok(())
}

fn parse_open_tag(raw: &str) -> Result<(String, Vec<(String, String)>), String> {
    let name_end = raw.find(char::is_whitespace).unwrap_or(raw.len());
    let name = &raw[..name_end];
    validate_name(name)?;
    let mut remaining = raw[name_end..].trim();
    let mut attributes = Vec::new();
    while !remaining.is_empty() {
        let equals = remaining
            .find('=')
            .ok_or_else(|| "malformed XML: expected '=' in attribute".to_owned())?;
        let attribute_name = remaining[..equals].trim();
        if attribute_name.is_empty() || attribute_name.chars().any(char::is_whitespace) {
            return Err("malformed XML: invalid attribute name".to_owned());
        }
        validate_name(attribute_name)?;
        remaining = remaining[equals + 1..].trim_start();
        let quote = remaining
            .chars()
            .next()
            .ok_or_else(|| "malformed XML: missing attribute value".to_owned())?;
        if quote != '\'' && quote != '"' {
            return Err("malformed XML: attribute values must be quoted".to_owned());
        }
        let value_start = 1;
        let value_end = remaining[value_start..]
            .find(quote)
            .map(|end| end + value_start)
            .ok_or_else(|| "malformed XML: unterminated attribute value".to_owned())?;
        let value = &remaining[value_start..value_end];
        if value.contains('&') {
            return Err("unsupported XML entity reference".to_owned());
        }
        if attributes.iter().any(|(name, _)| name == attribute_name) {
            return Err(format!(
                "malformed XML: duplicate attribute {attribute_name:?}"
            ));
        }
        attributes.push((attribute_name.to_owned(), value.to_owned()));
        remaining = remaining[value_end + 1..].trim_start();
    }
    Ok((name.to_owned(), attributes))
}

fn validate_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err("malformed XML: empty element or attribute name".to_owned());
    };
    if first == ':'
        || !first.is_ascii_alphabetic() && first != '_'
        || chars.any(|character| {
            !character.is_ascii_alphanumeric() && !matches!(character, '_' | '-' | '.')
        })
    {
        return Err(format!("unsupported XML name {name:?}"));
    }
    Ok(())
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
        return Err(format!(
            "unsupported shape: <{}> requires only the {name:?} attribute",
            element.name
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
        return Err(format!(
            "unsupported shape: <{}> must contain {}",
            elements[parent].name,
            expected.join(", ")
        ));
    }
    Ok(std::array::from_fn(|index| children[index]))
}

fn require_container_text(element: &Element) -> Result<(), String> {
    if element.text.trim().is_empty() {
        Ok(())
    } else {
        Err(format!(
            "unsupported shape: unexpected text inside <{}>",
            element.name
        ))
    }
}

fn require_no_attributes(element: &Element) -> Result<(), String> {
    if element.attributes.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "unsupported shape: attributes on <{}> are not supported",
            element.name
        ))
    }
}

fn leaf_text(element: &Element) -> Result<&str, String> {
    if !element.children.is_empty() || !element.attributes.is_empty() {
        return Err(format!(
            "unsupported shape: <{}> must be a plain text element",
            element.name
        ));
    }
    let text = element.text.trim();
    if text.is_empty() {
        return Err(format!(
            "unsupported score: <{}> may not be empty",
            element.name
        ));
    }
    Ok(text)
}

fn render_svg(tone: &Tone) -> String {
    let (tone_path, octave_y) = (
        glyph_path(tone.value),
        if tone.octave == 4 { 49 } else { 29 },
    );
    let stem = if tone.octave == 4 {
        "<path id=\"stem\" d=\"M102 58V30\" stroke=\"#171717\" stroke-width=\"1.5\"/>"
    } else {
        "<path id=\"stem\" d=\"M102 38V66\" stroke=\"#171717\" stroke-width=\"1.5\"/>"
    };
    format!(
		"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 140 96\" role=\"img\">\
		 <path id=\"staff-line-1\" d=\"M12 28H128\" stroke=\"#171717\"/>\
		 <path id=\"staff-line-2\" d=\"M12 48H128\" stroke=\"#171717\"/>\
		 <path id=\"staff-line-3\" d=\"M12 68H128\" stroke=\"#171717\"/>\
		 <path id=\"octave-5\" d=\"{}\" transform=\"translate(26 30)\" fill=\"none\" stroke=\"#171717\" stroke-width=\"1.5\"/>\
		 <path id=\"octave-4\" d=\"{}\" transform=\"translate(26 50)\" fill=\"none\" stroke=\"#171717\" stroke-width=\"1.5\"/>\
		 {stem}\
		 <path id=\"tonehead\" data-tone=\"{}\" transform=\"translate(92 {octave_y})\" d=\"{tone_path}\" fill=\"none\" stroke=\"#171717\" stroke-width=\"1.8\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\
		 </svg>\n",
		glyph_path(5),
		glyph_path(4),
		tone.value
	)
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
