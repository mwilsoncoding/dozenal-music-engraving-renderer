use std::collections::HashMap;
use std::fmt;
use std::io::Read;
use std::path::Path;

const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_DEPTH: usize = 64;
const MAX_ENTITY_EXPANSION_BYTES: usize = 8 * 1024 * 1024;
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";

#[derive(Debug)]
pub(crate) struct Element {
    pub(crate) name: String,
    pub(crate) namespace: String,
    pub(crate) path: String,
    pub(crate) byte_offset: usize,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) attributes: Vec<(String, String)>,
    pub(crate) text: String,
    pub(crate) children: Vec<usize>,
}

#[derive(Debug)]
pub(crate) struct XmlError {
    category: &'static str,
    message: String,
    offset: usize,
    line: Option<usize>,
    column: Option<usize>,
    path: String,
}

impl fmt::Display for XmlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let (Some(line), Some(column)) = (self.line, self.column) {
            write!(
                formatter,
                "{} XML at byte {}, line {}, column {}: {}",
                self.category, self.offset, line, column, self.message
            )?;
        } else {
            write!(
                formatter,
                "{} XML at byte {}: {} (line/column unavailable)",
                self.category, self.offset, self.message
            )?;
        }
        if !self.path.is_empty() {
            write!(formatter, " at {}", self.path)?;
        }
        Ok(())
    }
}

struct Source {
    text: String,
    byte_offsets: Vec<usize>,
}

impl Source {
    fn decode(input: &[u8]) -> Result<Self, XmlError> {
        if input.len() > MAX_INPUT_BYTES {
            return Err(XmlError {
                category: "resource limit",
                message: format!("input exceeds the {MAX_INPUT_BYTES}-byte limit"),
                offset: MAX_INPUT_BYTES,
                line: Some(1),
                column: Some(1),
                path: String::new(),
            });
        }

        let (encoding, start) = detect_encoding(input)?;
        let mut chars = Vec::<(char, usize)>::new();
        match encoding {
            Encoding::Utf8 => {
                let payload = &input[start..];
                let decoded = std::str::from_utf8(payload).map_err(|error| {
                    let offset = start + error.valid_up_to();
                    raw_error(input, "malformed", "input is not valid UTF-8", offset)
                })?;
                chars.extend(
                    decoded
                        .char_indices()
                        .map(|(index, character)| (character, start + index)),
                );
            }
            Encoding::Utf16Le | Encoding::Utf16Be => {
                if !(input.len() - start).is_multiple_of(2) {
                    return Err(raw_error(
                        input,
                        "malformed",
                        "UTF-16 input has an incomplete code unit",
                        input.len() - 1,
                    ));
                }
                let little_endian = encoding == Encoding::Utf16Le;
                let mut index = start;
                while index < input.len() {
                    let first = read_u16(input, index, little_endian);
                    if (0xd800..=0xdbff).contains(&first) {
                        if index + 3 >= input.len() {
                            return Err(raw_error(
                                input,
                                "malformed",
                                "unpaired UTF-16 high surrogate",
                                index,
                            ));
                        }
                        let second = read_u16(input, index + 2, little_endian);
                        if !(0xdc00..=0xdfff).contains(&second) {
                            return Err(raw_error(
                                input,
                                "malformed",
                                "unpaired UTF-16 high surrogate",
                                index,
                            ));
                        }
                        let scalar = 0x10000
                            + ((u32::from(first) - 0xd800) << 10)
                            + (u32::from(second) - 0xdc00);
                        chars.push((char::from_u32(scalar).expect("valid surrogate pair"), index));
                        index += 4;
                    } else if (0xdc00..=0xdfff).contains(&first) {
                        return Err(raw_error(
                            input,
                            "malformed",
                            "unpaired UTF-16 low surrogate",
                            index,
                        ));
                    } else {
                        chars.push((char::from_u32(u32::from(first)).expect("BMP scalar"), index));
                        index += 2;
                    }
                }
            }
        }

        let mut text = String::new();
        let mut byte_offsets = Vec::new();
        let mut index = 0;
        while index < chars.len() {
            let (character, original) = chars[index];
            let normalized = if character == '\r' {
                if chars.get(index + 1).is_some_and(|(next, _)| *next == '\n') {
                    index += 1;
                }
                '\n'
            } else {
                character
            };
            if !is_xml_char(normalized) {
                return Err(raw_error(
                    input,
                    "malformed",
                    "input contains a character forbidden by XML 1.0",
                    original,
                ));
            }
            let mut encoded = [0; 4];
            let value = normalized.encode_utf8(&mut encoded);
            byte_offsets.extend(std::iter::repeat_n(original, value.len()));
            text.push(normalized);
            index += 1;
        }
        byte_offsets.push(input.len());
        let source = Self { text, byte_offsets };
        source.check_declaration_encoding(encoding, start)?;
        Ok(source)
    }

    fn check_declaration_encoding(&self, actual: Encoding, start: usize) -> Result<(), XmlError> {
        let declaration = self.text.strip_prefix("<?xml");
        let Some(rest) = declaration else {
            if start > 0 && self.text.starts_with("<?xml") {
                return Err(self.error(0, "malformed", "XML declaration must follow the BOM"));
            }
            return Ok(());
        };
        if !rest.starts_with(char::is_whitespace) && !rest.starts_with("?>") {
            return Ok(());
        }
        let end = rest
            .find("?>")
            .ok_or_else(|| self.error(0, "malformed", "unterminated XML declaration"))?;
        let declaration = &rest[..end];
        let attributes = parse_pseudo_attributes(declaration)
            .map_err(|message| self.error(0, "malformed", &message))?;
        if attributes
            .first()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            != Some(("version", "1.0"))
        {
            return Err(self.error(0, "malformed", "only XML version 1.0 is supported"));
        }
        for (name, _) in &attributes {
            if !matches!(name.as_str(), "version" | "encoding" | "standalone") {
                return Err(self.error(0, "malformed", "unknown XML declaration pseudo-attribute"));
            }
        }
        let mut previous_order = 0;
        for (index, (name, _)) in attributes.iter().enumerate() {
            let order = match name.as_str() {
                "version" => 0,
                "encoding" => 1,
                "standalone" => 2,
                _ => unreachable!("unknown pseudo-attributes were rejected"),
            };
            if (index == 0 && order != 0) || (index > 0 && order <= previous_order) {
                return Err(self.error(
                    0,
                    "malformed",
                    "XML declaration pseudo-attributes are out of order",
                ));
            }
            previous_order = order;
        }
        if let Some((_, encoding)) = attributes.iter().find(|(name, _)| name == "encoding") {
            let declared = encoding.to_ascii_lowercase();
            let consistent = match actual {
                Encoding::Utf8 => declared == "utf-8",
                Encoding::Utf16Le | Encoding::Utf16Be => declared == "utf-16",
            };
            if !consistent {
                return Err(self.error(
                    0,
                    "malformed",
                    "XML encoding declaration conflicts with input encoding",
                ));
            }
        }
        if let Some((_, standalone)) = attributes.iter().find(|(name, _)| name == "standalone") {
            if standalone != "yes" && standalone != "no" {
                return Err(self.error(0, "malformed", "standalone must be 'yes' or 'no'"));
            }
        }
        Ok(())
    }

    fn error(&self, position: usize, category: &'static str, message: &str) -> XmlError {
        let position = position.min(self.text.len());
        let prefix = &self.text[..position];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix
            .rsplit('\n')
            .next()
            .map_or(1, |last_line| last_line.chars().count() + 1);
        XmlError {
            category,
            message: message.to_owned(),
            offset: self.byte_offsets.get(position).copied().unwrap_or(0),
            line: Some(line),
            column: Some(column),
            path: String::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
}

fn detect_encoding(input: &[u8]) -> Result<(Encoding, usize), XmlError> {
    if input.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Ok((Encoding::Utf8, 3));
    }
    if input.starts_with(&[0xff, 0xfe]) {
        return Ok((Encoding::Utf16Le, 2));
    }
    if input.starts_with(&[0xfe, 0xff]) {
        return Ok((Encoding::Utf16Be, 2));
    }
    if input.starts_with(&[0x00, 0x3c, 0x00, 0x3f]) {
        return Ok((Encoding::Utf16Be, 0));
    }
    if input.starts_with(&[0x3c, 0x00, 0x3f, 0x00]) {
        return Ok((Encoding::Utf16Le, 0));
    }
    Ok((Encoding::Utf8, 0))
}

fn read_u16(input: &[u8], index: usize, little_endian: bool) -> u16 {
    let pair = [input[index], input[index + 1]];
    if little_endian {
        u16::from_le_bytes(pair)
    } else {
        u16::from_be_bytes(pair)
    }
}

fn raw_error(_input: &[u8], category: &'static str, message: &str, offset: usize) -> XmlError {
    XmlError {
        category,
        message: message.to_owned(),
        offset,
        line: None,
        column: None,
        path: String::new(),
    }
}

pub(crate) fn parse(input: &[u8]) -> Result<Vec<Element>, XmlError> {
    let source = Source::decode(input)?;
    Parser::new(&source).document()
}

pub(crate) fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut input = Vec::new();
    file.take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut input)
        .map_err(|error| error.to_string())?;
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "resource limit XML at byte {MAX_INPUT_BYTES}, line 1, column 1: input exceeds the {MAX_INPUT_BYTES}-byte limit"
        ));
    }
    Ok(input)
}

struct Parser<'a> {
    source: &'a Source,
    cursor: usize,
    elements: Vec<Element>,
    entities: HashMap<String, Entity>,
    parameter_entities: HashMap<String, Entity>,
    expansion_bytes: usize,
    doctype_name: Option<String>,
    path: Vec<String>,
}

enum Entity {
    Internal(String),
    External,
}

impl<'a> Parser<'a> {
    fn new(source: &'a Source) -> Self {
        Self {
            source,
            cursor: 0,
            elements: Vec::new(),
            entities: HashMap::new(),
            parameter_entities: HashMap::new(),
            expansion_bytes: 0,
            doctype_name: None,
            path: Vec::new(),
        }
    }

    fn document(mut self) -> Result<Vec<Element>, XmlError> {
        if self.starts_with("<?xml") && self.peek_after("<?xml").is_some_and(is_space) {
            self.processing_instruction(true)?;
            self.space();
        }
        self.misc()?;
        if self.starts_with("<!DOCTYPE") {
            self.doctype()?;
            self.misc()?;
        }
        if self.cursor >= self.source.text.len() || !self.starts_with("<") {
            return Err(self.error("malformed", "document has no root element"));
        }
        let mut namespaces = HashMap::new();
        namespaces.insert("xml".to_owned(), XML_NAMESPACE.to_owned());
        let root = self.element(&namespaces, 1)?;
        if let Some(expected) = &self.doctype_name {
            if self.elements[root].name != *expected {
                return Err(self.error("malformed", "DOCTYPE name does not match document element"));
            }
        }
        self.misc()?;
        if self.cursor != self.source.text.len() {
            return Err(self.error("malformed", "content after the document element"));
        }
        Ok(self.elements)
    }

    fn misc(&mut self) -> Result<(), XmlError> {
        loop {
            self.space();
            if self.starts_with("<!--") {
                self.comment()?;
            } else if self.starts_with("<?") {
                self.processing_instruction(false)?;
            } else {
                return Ok(());
            }
        }
    }

    fn doctype(&mut self) -> Result<(), XmlError> {
        let start = self.cursor;
        self.cursor += "<!DOCTYPE".len();
        if !self.take_space() {
            return Err(self.error("malformed", "expected whitespace after DOCTYPE"));
        }
        let name = self.name()?;
        self.doctype_name = Some(name);
        let external_id_start = self.cursor;
        let mut quote = None;
        let mut subset_start = None;
        let mut subset_end = None;
        let mut bracket_depth = 0usize;
        while self.cursor < self.source.text.len() {
            let character = self.current_char().expect("cursor is in bounds");
            if let Some(delimiter) = quote {
                if character == delimiter {
                    quote = None;
                }
            } else if character == '\'' || character == '"' {
                quote = Some(character);
            } else if self.starts_with("<!--") {
                self.comment()?;
            } else if character == '[' {
                if bracket_depth == 0 {
                    subset_start = Some(self.cursor + 1);
                }
                bracket_depth += 1;
            } else if character == ']' {
                if bracket_depth > 0 {
                    bracket_depth -= 1;
                    if bracket_depth == 0 {
                        subset_end = Some(self.cursor);
                    }
                }
            } else if character == '>' && bracket_depth == 0 {
                let end = self.cursor;
                let external_id_end = subset_start.map_or(end, |subset_start| subset_start - 1);
                DtdDeclarationParser::validate_doctype_external_id(
                    self.source,
                    &self.source.text[external_id_start..external_id_end],
                    external_id_start,
                )?;
                if let Some(subset_start) = subset_start {
                    self.read_internal_subset(subset_start, subset_end.unwrap_or(end))?;
                }
                self.cursor += 1;
                return Ok(());
            }
            self.advance_char();
        }
        self.cursor = start;
        Err(self.error("malformed", "unterminated DOCTYPE"))
    }

    fn read_internal_subset(&mut self, cursor: usize, end: usize) -> Result<(), XmlError> {
        let base = cursor;
        let subset = self.source.text[cursor..end].to_owned();
        self.read_internal_subset_content(&subset, base, &mut Vec::new())
    }

    fn read_internal_subset_content(
        &mut self,
        content: &str,
        base: usize,
        entity_stack: &mut Vec<String>,
    ) -> Result<(), XmlError> {
        let mut cursor = 0;
        while cursor < content.len() {
            while cursor < content.len() && content[cursor..].chars().next().is_some_and(is_space) {
                cursor += content[cursor..].chars().next().expect("space").len_utf8();
            }
            if cursor >= content.len() {
                break;
            }
            if content[cursor..].starts_with("<!--") {
                let close = content[cursor + 4..].find("-->").ok_or_else(|| {
                    self.source
                        .error(base + cursor, "malformed", "unterminated DTD comment")
                })?;
                let body = &content[cursor + 4..cursor + 4 + close];
                if body.contains("--") {
                    return Err(self.source.error(
                        base + cursor,
                        "malformed",
                        "'--' is not allowed inside a comment",
                    ));
                }
                cursor += close + 7;
                continue;
            }
            if content[cursor..].starts_with("<!ENTITY") {
                let (next, name, entity, parameter) =
                    self.entity_declaration(content, cursor, base)?;
                let entities = if parameter {
                    &mut self.parameter_entities
                } else {
                    &mut self.entities
                };
                if entities.insert(name.clone(), entity).is_some() {
                    return Err(self.source.error(
                        base + cursor,
                        "malformed",
                        &format!("duplicate entity declaration {name:?}"),
                    ));
                }
                cursor = next;
                continue;
            }
            if content[cursor..].starts_with("<!") {
                cursor = self.skip_dtd_declaration(content, cursor, base)?;
                continue;
            }
            if content[cursor..].starts_with('%') {
                let (name, after_name) = parse_name_at(content, cursor + 1).ok_or_else(|| {
                    self.source.error(
                        base + cursor,
                        "malformed",
                        "invalid parameter entity reference",
                    )
                })?;
                if !content[after_name..].starts_with(';') {
                    return Err(self.source.error(
                        base + cursor,
                        "malformed",
                        "unterminated parameter entity reference",
                    ));
                }
                let entity = self.parameter_entities.get(&name).ok_or_else(|| {
                    self.source.error(
                        base + cursor,
                        "malformed",
                        &format!("reference to undeclared parameter entity {name:?}"),
                    )
                })?;
                let Entity::Internal(replacement) = entity else {
                    return Err(self.source.error(
                        base + cursor,
                        "unsupported",
                        "external parameter entity reference is not resolved",
                    ));
                };
                let replacement = replacement.clone();
                if entity_stack.iter().any(|active| active == &name) {
                    return Err(self.source.error(
                        base + cursor,
                        "malformed",
                        "recursive parameter entity reference",
                    ));
                }
                if entity_stack.len() >= 16 {
                    return Err(self.source.error(
                        base + cursor,
                        "resource limit",
                        "parameter entity nesting exceeds the 16-level limit",
                    ));
                }
                if self
                    .expansion_bytes
                    .checked_add(replacement.len())
                    .is_none_or(|size| size > MAX_ENTITY_EXPANSION_BYTES)
                {
                    return Err(self.source.error(base + cursor, "resource limit", &format!("parameter entity expansion exceeds the {MAX_ENTITY_EXPANSION_BYTES}-byte aggregate limit")));
                }
                self.expansion_bytes += replacement.len();
                entity_stack.push(name);
                self.read_internal_subset_content(&replacement, base + cursor, entity_stack)?;
                entity_stack.pop();
                cursor = after_name + 1;
                continue;
            }
            return Err(self.source.error(
                base + cursor,
                "malformed",
                "unexpected content in internal subset",
            ));
        }
        Ok(())
    }

    fn entity_declaration(
        &mut self,
        content: &str,
        start: usize,
        base: usize,
    ) -> Result<(usize, String, Entity, bool), XmlError> {
        let mut cursor = start + "<!ENTITY".len();
        if !content[cursor..].chars().next().is_some_and(is_space) {
            return Err(self.source.error(
                base + cursor,
                "malformed",
                "expected whitespace in entity declaration",
            ));
        }
        while content[cursor..].chars().next().is_some_and(is_space) {
            cursor += content[cursor..].chars().next().expect("space").len_utf8();
        }
        let parameter = content[cursor..].starts_with('%');
        if parameter {
            cursor += 1;
            if !content[cursor..].chars().next().is_some_and(is_space) {
                return Err(self.source.error(
                    base + cursor,
                    "malformed",
                    "expected whitespace after '%' in parameter entity declaration",
                ));
            }
            while content[cursor..].chars().next().is_some_and(is_space) {
                cursor += content[cursor..].chars().next().expect("space").len_utf8();
            }
        }
        let (name, after_name) = parse_name_at(content, cursor).ok_or_else(|| {
            self.source
                .error(base + cursor, "malformed", "invalid entity name")
        })?;
        cursor = after_name;
        while content[cursor..].chars().next().is_some_and(is_space) {
            cursor += content[cursor..].chars().next().expect("space").len_utf8();
        }
        let value = content[cursor..].chars().next();
        let entity = if value == Some('\'') || value == Some('"') {
            let quote = value.expect("quoted value");
            cursor += quote.len_utf8();
            let value_start = cursor;
            while cursor < content.len() && !content[cursor..].starts_with(quote) {
                cursor += content[cursor..]
                    .chars()
                    .next()
                    .expect("character")
                    .len_utf8();
            }
            if cursor >= content.len() {
                return Err(self.source.error(
                    base + value_start,
                    "malformed",
                    "unterminated entity value",
                ));
            }
            let value = content[value_start..cursor].to_owned();
            cursor += quote.len_utf8();
            let value = if value.contains('%') {
                let mut expanded = String::new();
                self.expand_parameter_entity_value(
                    &value,
                    base + value_start,
                    &mut Vec::new(),
                    &mut expanded,
                )?;
                self.expansion_bytes = self
                    .expansion_bytes
                    .checked_add(expanded.len())
                    .filter(|size| *size <= MAX_ENTITY_EXPANSION_BYTES)
                    .ok_or_else(|| self.source.error(base + value_start, "resource limit", &format!("parameter entity expansion exceeds the {MAX_ENTITY_EXPANSION_BYTES}-byte aggregate limit")))?;
                expanded
            } else {
                value
            };
            Entity::Internal(value)
        } else if content[cursor..].starts_with("SYSTEM") || content[cursor..].starts_with("PUBLIC")
        {
            let external_id_start = cursor;
            let mut quote = None;
            while cursor < content.len() {
                let character = content[cursor..]
                    .chars()
                    .next()
                    .expect("cursor is in bounds");
                if let Some(delimiter) = quote {
                    if character == delimiter {
                        quote = None;
                    }
                } else if character == '\'' || character == '"' {
                    quote = Some(character);
                } else if character == '>' {
                    break;
                }
                cursor += character.len_utf8();
            }
            if cursor >= content.len() {
                return Err(self.source.error(
                    base + external_id_start,
                    "malformed",
                    "unterminated external entity declaration",
                ));
            }
            let external_id = &content[external_id_start..cursor];
            let mut identifier =
                DtdDeclarationParser::new(self.source, external_id, base + external_id_start);
            identifier.external_id(false)?;
            let separated = identifier.take_space();
            if identifier.consume_keyword("NDATA") {
                if parameter || !separated {
                    return Err(self.source.error(
                        base + external_id_start + identifier.cursor,
                        "malformed",
                        "invalid NDATA declaration",
                    ));
                }
                identifier.require_space()?;
                identifier.name()?;
                identifier.space();
            }
            if identifier.cursor != external_id.len() {
                return Err(self.source.error(
                    base + external_id_start + identifier.cursor,
                    "malformed",
                    "unexpected content in external entity identifier",
                ));
            }
            Entity::External
        } else {
            return Err(self.source.error(
                base + cursor,
                "malformed",
                "invalid entity declaration",
            ));
        };
        while content[cursor..].chars().next().is_some_and(is_space) {
            cursor += content[cursor..].chars().next().expect("space").len_utf8();
        }
        if !content[cursor..].starts_with('>') {
            return Err(self.source.error(
                base + cursor,
                "malformed",
                "unexpected content in entity declaration",
            ));
        }
        Ok((cursor + 1, name, entity, parameter))
    }

    fn expand_parameter_entity_value(
        &mut self,
        value: &str,
        position: usize,
        entity_stack: &mut Vec<String>,
        output: &mut String,
    ) -> Result<(), XmlError> {
        let mut cursor = 0;
        while let Some(relative) = value[cursor..].find('%') {
            let reference_start = cursor + relative;
            self.append_parameter_value(output, &value[cursor..reference_start], position)?;
            let (name, after_name) =
                parse_name_at(value, reference_start + 1).ok_or_else(|| {
                    self.source.error(
                        position,
                        "malformed",
                        "invalid parameter entity reference in entity value",
                    )
                })?;
            if !value[after_name..].starts_with(';') {
                return Err(self.source.error(
                    position,
                    "malformed",
                    "unterminated parameter entity reference in entity value",
                ));
            }
            let entity = self.parameter_entities.get(&name).ok_or_else(|| {
                self.source.error(
                    position,
                    "malformed",
                    &format!("reference to undeclared parameter entity {name:?}"),
                )
            })?;
            let Entity::Internal(replacement) = entity else {
                return Err(self.source.error(
                    position,
                    "unsupported",
                    "external parameter entity reference is not resolved",
                ));
            };
            let replacement = replacement.clone();
            if entity_stack.len() >= 16 {
                return Err(self.source.error(
                    position,
                    "resource limit",
                    "parameter entity nesting exceeds the 16-level limit",
                ));
            }
            if entity_stack.iter().any(|active| active == &name) {
                return Err(self.source.error(
                    position,
                    "malformed",
                    "recursive parameter entity reference",
                ));
            }
            entity_stack.push(name);
            self.expand_parameter_entity_value(&replacement, position, entity_stack, output)?;
            entity_stack.pop();
            cursor = after_name + 1;
        }
        self.append_parameter_value(output, &value[cursor..], position)
    }

    fn append_parameter_value(
        &self,
        output: &mut String,
        value: &str,
        position: usize,
    ) -> Result<(), XmlError> {
        let size = self
            .expansion_bytes
            .checked_add(output.len())
            .and_then(|size| size.checked_add(value.len()))
            .filter(|size| *size <= MAX_ENTITY_EXPANSION_BYTES)
            .ok_or_else(|| self.source.error(position, "resource limit", &format!("parameter entity expansion exceeds the {MAX_ENTITY_EXPANSION_BYTES}-byte aggregate limit")))?;
        let _ = size;
        output.push_str(value);
        Ok(())
    }

    fn skip_dtd_declaration(
        &self,
        content: &str,
        start: usize,
        base: usize,
    ) -> Result<usize, XmlError> {
        let mut cursor = start + 2;
        let mut quote = None;
        while cursor < content.len() {
            let character = content[cursor..]
                .chars()
                .next()
                .expect("cursor is in bounds");
            if let Some(delimiter) = quote {
                if character == delimiter {
                    quote = None;
                }
            } else if character == '\'' || character == '"' {
                quote = Some(character);
            } else if character == '>' {
                let end = cursor + 1;
                DtdDeclarationParser::new(self.source, &content[start..end], base + start)
                    .parse()?;
                return Ok(end);
            }
            cursor += character.len_utf8();
        }
        Err(self
            .source
            .error(base + start, "malformed", "unterminated DTD declaration"))
    }

    fn element(
        &mut self,
        inherited_namespaces: &HashMap<String, String>,
        depth: usize,
    ) -> Result<usize, XmlError> {
        if depth > MAX_DEPTH {
            return Err(self.error(
                "resource limit",
                &format!("element nesting exceeds the {MAX_DEPTH}-level limit"),
            ));
        }
        let start = self.cursor;
        self.expect("<")?;
        if self.starts_with("!") || self.starts_with("?") || self.starts_with("/") {
            return Err(self.error("malformed", "expected start-tag"));
        }
        let qualified_name = self.name()?;
        if !is_qname(&qualified_name) {
            return Err(self.error("malformed", &format!("invalid QName {qualified_name:?}")));
        }
        self.path.push(qualified_name.clone());
        let mut raw_attributes = Vec::<(String, String)>::new();
        let mut self_closing = false;
        loop {
            let had_space = self.take_space();
            if self.starts_with("/>") {
                self.cursor += 2;
                self_closing = true;
                break;
            }
            if self.starts_with(">") {
                self.cursor += 1;
                break;
            }
            if !had_space {
                return Err(self.error("malformed", "expected whitespace before attribute"));
            }
            let attribute_name = self.name()?;
            if !is_qname(&attribute_name) {
                return Err(self.error("malformed", &format!("invalid QName {attribute_name:?}")));
            }
            if raw_attributes
                .iter()
                .any(|(name, _)| name == &attribute_name)
            {
                return Err(self.error(
                    "malformed",
                    &format!("duplicate attribute {attribute_name:?}"),
                ));
            }
            self.space();
            self.expect("=")?;
            self.space();
            let value = self.attribute_value()?;
            raw_attributes.push((attribute_name, value));
        }

        let mut namespaces = inherited_namespaces.clone();
        for (name, value) in &raw_attributes {
            if name == "xmlns" {
                if value == XMLNS_NAMESPACE {
                    return Err(self.error_at(
                        start,
                        "malformed",
                        "the xmlns namespace cannot be the default namespace",
                    ));
                }
                if value == XML_NAMESPACE {
                    return Err(self.error_at(
                        start,
                        "malformed",
                        "only the xml prefix may use the XML namespace",
                    ));
                }
                namespaces.insert(String::new(), value.clone());
            } else if let Some(prefix) = name.strip_prefix("xmlns:") {
                if prefix == "xmlns" || prefix.is_empty() {
                    return Err(self.error_at(
                        start,
                        "malformed",
                        "invalid namespace declaration prefix",
                    ));
                }
                if prefix == "xml" && value != XML_NAMESPACE {
                    return Err(self.error_at(
                        start,
                        "malformed",
                        "the xml prefix has a reserved namespace",
                    ));
                }
                if prefix != "xml" && value == XML_NAMESPACE {
                    return Err(self.error_at(
                        start,
                        "malformed",
                        "only the xml prefix may use the XML namespace",
                    ));
                }
                if value == XMLNS_NAMESPACE {
                    return Err(self.error_at(
                        start,
                        "malformed",
                        "the xmlns namespace cannot be bound to a prefix",
                    ));
                }
                if value.is_empty() {
                    return Err(self.error_at(
                        start,
                        "malformed",
                        "a namespace prefix cannot be undeclared in Namespaces 1.0",
                    ));
                }
                namespaces.insert(prefix.to_owned(), value.clone());
            }
        }
        let namespace =
            resolve_namespace(&qualified_name, &namespaces, false).ok_or_else(|| {
                self.error_at(
                    start,
                    "malformed",
                    "element uses an undeclared namespace prefix",
                )
            })?;
        let mut attributes = Vec::new();
        let mut expanded = Vec::<(String, String)>::new();
        for (name, value) in raw_attributes {
            if name == "xmlns" || name.starts_with("xmlns:") {
                continue;
            }
            let attribute_namespace =
                resolve_namespace(&name, &namespaces, true).ok_or_else(|| {
                    self.error_at(
                        start,
                        "malformed",
                        "attribute uses an undeclared namespace prefix",
                    )
                })?;
            let local = local_part(&name).to_owned();
            if expanded
                .iter()
                .any(|(uri, prior_local)| uri == &attribute_namespace && prior_local == &local)
            {
                return Err(self.error_at(start, "malformed", "duplicate attribute expanded name"));
            }
            expanded.push((attribute_namespace, local));
            attributes.push((name, value));
        }
        if let Some((attribute, identifier)) = attributes
            .iter()
            .find(|(name, _)| name == "id" || name == "number")
        {
            *self
                .path
                .last_mut()
                .expect("element path has a current element") =
                format!("{qualified_name}[@{attribute}='{identifier}']");
        }
        let location = self.source.error(start, "malformed", "");
        let index = self.elements.len();
        self.elements.push(Element {
            name: qualified_name,
            namespace,
            path: format!("/{}", self.path.join("/")),
            byte_offset: location.offset,
            line: location.line.expect("decoded source has a line position"),
            column: location
                .column
                .expect("decoded source has a column position"),
            attributes,
            text: String::new(),
            children: Vec::new(),
        });

        if !self_closing {
            loop {
                if self.cursor >= self.source.text.len() {
                    return Err(self.source.error(start, "malformed", "unclosed element"));
                }
                if self.starts_with("</") {
                    self.cursor += 2;
                    let close_name = self.name()?;
                    self.space();
                    self.expect(">")?;
                    if close_name != self.elements[index].name {
                        return Err(self.error(
                            "malformed",
                            &format!(
                                "expected </{}>, found </{close_name}>",
                                self.elements[index].name
                            ),
                        ));
                    }
                    break;
                }
                if self.starts_with("<!--") {
                    self.comment()?;
                } else if self.starts_with("<?") {
                    self.processing_instruction(false)?;
                } else if self.starts_with("<![CDATA[") {
                    self.cdata(index)?;
                } else if self.starts_with("<!") {
                    return Err(
                        self.error("malformed", "declaration is not allowed in element content")
                    );
                } else if self.starts_with("<") {
                    let child = self.element(&namespaces, depth + 1)?;
                    self.elements[index].children.push(child);
                } else {
                    self.character_data(index, &namespaces)?;
                }
            }
        }
        self.path.pop();
        Ok(index)
    }

    fn character_data(
        &mut self,
        parent: usize,
        namespaces: &HashMap<String, String>,
    ) -> Result<(), XmlError> {
        let start = self.cursor;
        while self.cursor < self.source.text.len() && !self.starts_with("<") {
            if self.starts_with("]]>") {
                return Err(self.error("malformed", "']]>' is not allowed in character data"));
            }
            if self.starts_with("&") {
                let reference_start = self.cursor;
                let (value, markup) = self.reference(false, 0)?;
                if markup {
                    self.append_entity_fragment(parent, &value, namespaces, reference_start)?;
                } else {
                    self.elements[parent].text.push_str(&value);
                }
            } else {
                let character = self.current_char().expect("cursor is in bounds");
                self.elements[parent].text.push(character);
                self.advance_char();
            }
        }
        if self.cursor == start {
            return Err(self.error("malformed", "parser made no progress in character data"));
        }
        Ok(())
    }

    fn attribute_value(&mut self) -> Result<String, XmlError> {
        let quote = self
            .current_char()
            .ok_or_else(|| self.error("malformed", "missing attribute value"))?;
        if quote != '\'' && quote != '"' {
            return Err(self.error("malformed", "attribute values must be quoted"));
        }
        self.advance_char();
        let mut value = String::new();
        loop {
            if self.cursor >= self.source.text.len() {
                return Err(self.error("malformed", "unterminated attribute value"));
            }
            if self.current_char() == Some(quote) {
                self.advance_char();
                return Ok(value);
            }
            if self.starts_with("<") {
                return Err(self.error("malformed", "'<' is not allowed in an attribute value"));
            }
            if self.starts_with("&") {
                value.push_str(&self.reference(true, 0)?.0);
            } else {
                let character = self.current_char().expect("cursor is in bounds");
                value.push(if matches!(character, '\t' | '\n' | '\r') {
                    ' '
                } else {
                    character
                });
                self.advance_char();
            }
        }
    }

    fn reference(&mut self, in_attribute: bool, depth: usize) -> Result<(String, bool), XmlError> {
        let start = self.cursor;
        self.expect("&")?;
        let end = self.source.text[self.cursor..]
            .find(';')
            .map(|relative| self.cursor + relative)
            .ok_or_else(|| self.error_at(start, "malformed", "unterminated entity reference"))?;
        let name = &self.source.text[self.cursor..end];
        self.cursor = end + 1;
        let mut entity_stack = Vec::new();
        let mut expanded = String::new();
        let markup =
            self.expand_reference_value(name, start, depth, &mut entity_stack, &mut expanded)?;
        if in_attribute && markup {
            return Err(self.error_at(
                start,
                "malformed",
                "markup in an entity replacement is not allowed in an attribute value",
            ));
        }
        self.expansion_bytes = self
            .expansion_bytes
            .checked_add(expanded.len())
            .filter(|size| *size <= MAX_ENTITY_EXPANSION_BYTES)
            .ok_or_else(|| self.error_at(start, "resource limit", &format!("entity expansion exceeds the {MAX_ENTITY_EXPANSION_BYTES}-byte aggregate limit")))?;
        Ok((expanded, markup))
    }

    fn expand_reference_value(
        &mut self,
        name: &str,
        position: usize,
        depth: usize,
        entity_stack: &mut Vec<String>,
        output: &mut String,
    ) -> Result<bool, XmlError> {
        match name {
            "amp" => {
                self.append_expansion(output, "&", position)?;
                Ok(false)
            }
            "lt" => {
                self.append_expansion(output, "<", position)?;
                Ok(false)
            }
            "gt" => {
                self.append_expansion(output, ">", position)?;
                Ok(false)
            }
            "apos" => {
                self.append_expansion(output, "'", position)?;
                Ok(false)
            }
            "quot" => {
                self.append_expansion(output, "\"", position)?;
                Ok(false)
            }
            _ if name.starts_with("#x") => u32::from_str_radix(&name[2..], 16)
                .ok()
                .and_then(char::from_u32)
                .filter(|character| is_xml_char(*character))
                .ok_or_else(|| {
                    self.error_at(
                        position,
                        "malformed",
                        "invalid hexadecimal character reference",
                    )
                })
                .and_then(|character| {
                    self.append_expansion(output, &character.to_string(), position)?;
                    Ok(false)
                }),
            _ if name.starts_with('#') => name[1..]
                .parse::<u32>()
                .ok()
                .and_then(char::from_u32)
                .filter(|character| is_xml_char(*character))
                .ok_or_else(|| {
                    self.error_at(position, "malformed", "invalid decimal character reference")
                })
                .and_then(|character| {
                    self.append_expansion(output, &character.to_string(), position)?;
                    Ok(false)
                }),
            _ => {
                let entity = self.entities.get(name).ok_or_else(|| {
                    self.error_at(
                        position,
                        "malformed",
                        &format!("reference to undeclared entity {name:?}"),
                    )
                })?;
                let Entity::Internal(replacement) = entity else {
                    return Err(self.error_at(
                        position,
                        "unsupported",
                        "external entity reference is not resolved",
                    ));
                };
                let replacement = replacement.clone();
                if depth >= 16 {
                    return Err(self.error_at(
                        position,
                        "resource limit",
                        "entity nesting exceeds the 16-level limit",
                    ));
                }
                if entity_stack.iter().any(|active| active == name) {
                    return Err(self.error_at(position, "malformed", "recursive entity reference"));
                }
                entity_stack.push(name.to_owned());
                let mut markup = replacement.contains('<');
                let mut cursor = 0;
                while let Some(relative) = replacement[cursor..].find('&') {
                    let ampersand = cursor + relative;
                    self.append_expansion(output, &replacement[cursor..ampersand], position)?;
                    let semi = replacement[ampersand..]
                        .find(';')
                        .map(|relative| ampersand + relative)
                        .ok_or_else(|| {
                            self.error_at(
                                position,
                                "malformed",
                                "unterminated reference in entity value",
                            )
                        })?;
                    let nested_name = &replacement[ampersand + 1..semi];
                    markup |= self.expand_reference_value(
                        nested_name,
                        position,
                        depth + 1,
                        entity_stack,
                        output,
                    )?;
                    cursor = semi + 1;
                }
                self.append_expansion(output, &replacement[cursor..], position)?;
                entity_stack.pop();
                Ok(markup)
            }
        }
    }

    fn append_expansion(
        &self,
        output: &mut String,
        value: &str,
        position: usize,
    ) -> Result<(), XmlError> {
        let expanded_size = self
            .expansion_bytes
            .checked_add(output.len())
            .and_then(|size| size.checked_add(value.len()))
            .ok_or_else(|| {
                self.error_at(position, "resource limit", "entity expansion size overflow")
            })?;
        if expanded_size > MAX_ENTITY_EXPANSION_BYTES {
            return Err(self.error_at(position, "resource limit", &format!("entity expansion exceeds the {MAX_ENTITY_EXPANSION_BYTES}-byte aggregate limit")));
        }
        output.push_str(value);
        Ok(())
    }

    fn append_entity_fragment(
        &mut self,
        parent: usize,
        replacement: &str,
        namespaces: &HashMap<String, String>,
        position: usize,
    ) -> Result<(), XmlError> {
        let mut fragment = String::from("<__entity_wrapper");
        for (prefix, namespace) in namespaces {
            if prefix == "xml" {
                continue;
            }
            fragment.push_str(" xmlns");
            if !prefix.is_empty() {
                fragment.push(':');
                fragment.push_str(prefix);
            }
            fragment.push_str("=\"");
            fragment.push_str(&escape_xml_attribute(namespace));
            fragment.push('"');
        }
        fragment.push('>');
        fragment.push_str(replacement);
        fragment.push_str("</__entity_wrapper>");
        let fragment_source = Source {
            byte_offsets: (0..=fragment.len()).collect(),
            text: fragment,
        };
        let parsed = Parser::new(&fragment_source)
            .document()
            .map_err(|error| self.error_at(position, error.category, &error.message))?;
        let wrapper = parsed.first().ok_or_else(|| {
            self.error_at(position, "malformed", "empty entity replacement fragment")
        })?;
        self.elements[parent].text.push_str(&wrapper.text);
        let insertion = self.elements.len();
        let parent_path = self.elements[parent].path.clone();
        let wrapper_prefix = format!("{}/__entity_wrapper", parent_path);
        let location = self.source.error(position, "malformed", "");
        let child_roots = wrapper
            .children
            .iter()
            .map(|child| insertion + child - 1)
            .collect::<Vec<_>>();
        for mut element in parsed.into_iter().skip(1) {
            for child in &mut element.children {
                *child = insertion + *child - 1;
            }
            if let Some(suffix) = element.path.strip_prefix(&wrapper_prefix) {
                element.path = format!("{parent_path}{suffix}");
            }
            element.byte_offset = location.offset;
            element.line = location.line.expect("decoded source has a line position");
            element.column = location
                .column
                .expect("decoded source has a column position");
            self.elements.push(element);
        }
        self.elements[parent].children.extend(child_roots);
        Ok(())
    }

    fn cdata(&mut self, parent: usize) -> Result<(), XmlError> {
        self.cursor += "<![CDATA[".len();
        let end = self.source.text[self.cursor..]
            .find("]]>")
            .map(|relative| self.cursor + relative)
            .ok_or_else(|| self.error("malformed", "unterminated CDATA section"))?;
        self.elements[parent]
            .text
            .push_str(&self.source.text[self.cursor..end]);
        self.cursor = end + 3;
        Ok(())
    }

    fn comment(&mut self) -> Result<(), XmlError> {
        let start = self.cursor;
        self.cursor += 4;
        let end = self.source.text[self.cursor..]
            .find("-->")
            .map(|relative| self.cursor + relative)
            .ok_or_else(|| {
                self.source
                    .error(start, "malformed", "unterminated comment")
            })?;
        if self.source.text[self.cursor..end].contains("--") {
            return Err(self.source.error(
                start,
                "malformed",
                "'--' is not allowed inside a comment",
            ));
        }
        self.cursor = end + 3;
        Ok(())
    }

    fn processing_instruction(&mut self, declaration: bool) -> Result<(), XmlError> {
        let start = self.cursor;
        self.cursor += 2;
        let target = self.name()?;
        if target.eq_ignore_ascii_case("xml") && !declaration {
            return Err(self.source.error(
                start,
                "malformed",
                "processing-instruction target 'xml' is reserved",
            ));
        }
        if declaration && target != "xml" {
            return Err(self
                .source
                .error(start, "malformed", "invalid XML declaration"));
        }
        let end = self.source.text[self.cursor..]
            .find("?>")
            .map(|relative| self.cursor + relative)
            .ok_or_else(|| {
                self.source
                    .error(start, "malformed", "unterminated processing instruction")
            })?;
        if declaration {
            let raw = &self.source.text[self.cursor..end];
            let attributes = parse_pseudo_attributes(raw)
                .map_err(|message| self.source.error(start, "malformed", &message))?;
            if attributes
                .first()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                != Some(("version", "1.0"))
            {
                return Err(self.source.error(
                    start,
                    "malformed",
                    "XML declaration must begin with version='1.0'",
                ));
            }
            for (name, value) in &attributes {
                if name == "standalone" && value != "yes" && value != "no" {
                    return Err(self.source.error(
                        start,
                        "malformed",
                        "standalone must be 'yes' or 'no'",
                    ));
                }
            }
        }
        self.cursor = end + 2;
        Ok(())
    }

    fn name(&mut self) -> Result<String, XmlError> {
        let (name, end) = parse_name_at(&self.source.text, self.cursor)
            .ok_or_else(|| self.error("malformed", "expected XML name"))?;
        self.cursor = end;
        Ok(name)
    }

    fn space(&mut self) {
        while self.current_char().is_some_and(is_space) {
            self.advance_char();
        }
    }

    fn take_space(&mut self) -> bool {
        let start = self.cursor;
        self.space();
        self.cursor != start
    }

    fn expect(&mut self, expected: &str) -> Result<(), XmlError> {
        if self.starts_with(expected) {
            self.cursor += expected.len();
            Ok(())
        } else {
            Err(self.error("malformed", &format!("expected {expected:?}")))
        }
    }

    fn starts_with(&self, value: &str) -> bool {
        self.source.text[self.cursor..].starts_with(value)
    }

    fn peek_after(&self, value: &str) -> Option<char> {
        self.source.text[self.cursor + value.len()..].chars().next()
    }

    fn current_char(&self) -> Option<char> {
        self.source.text[self.cursor..].chars().next()
    }

    fn advance_char(&mut self) {
        if let Some(character) = self.current_char() {
            self.cursor += character.len_utf8();
        }
    }

    fn error(&self, category: &'static str, message: &str) -> XmlError {
        self.error_at(self.cursor, category, message)
    }

    fn error_at(&self, position: usize, category: &'static str, message: &str) -> XmlError {
        let mut error = self.source.error(position, category, message);
        error.path = self.path.join("/");
        if !error.path.is_empty() {
            error.path.insert(0, '/');
        }
        error
    }
}

struct DtdDeclarationParser<'a> {
    source: &'a Source,
    text: &'a str,
    base: usize,
    cursor: usize,
}

impl<'a> DtdDeclarationParser<'a> {
    fn new(source: &'a Source, text: &'a str, base: usize) -> Self {
        Self {
            source,
            text,
            base,
            cursor: 0,
        }
    }

    fn parse(mut self) -> Result<(), XmlError> {
        self.expect("<!")?;
        if self.consume_keyword("ELEMENT") {
            self.require_space()?;
            self.name()?;
            self.require_space()?;
            self.content_spec()?;
        } else if self.consume_keyword("ATTLIST") {
            self.require_space()?;
            self.name()?;
            loop {
                if !self.take_space() {
                    break;
                }
                if self.starts_with(">") {
                    break;
                }
                self.name()?;
                self.require_space()?;
                self.attribute_type()?;
                self.require_space()?;
                self.default_declaration()?;
            }
        } else if self.consume_keyword("NOTATION") {
            self.require_space()?;
            self.name()?;
            self.require_space()?;
            self.external_id(true)?;
        } else {
            return Err(self.error("unsupported markup declaration in internal subset"));
        }

        self.space();
        self.expect(">")?;
        self.space();
        if self.cursor != self.text.len() {
            return Err(self.error("unexpected content after DTD markup declaration"));
        }
        Ok(())
    }

    fn validate_doctype_external_id(
        source: &'a Source,
        header: &'a str,
        base: usize,
    ) -> Result<(), XmlError> {
        let mut parser = Self::new(source, header, base);
        if parser.cursor == parser.text.len() {
            return Ok(());
        }
        parser.require_space()?;
        if parser.cursor == parser.text.len() {
            return Ok(());
        }
        parser.external_id(false)?;
        parser.space();
        if parser.cursor != parser.text.len() {
            return Err(parser.error("unexpected content in DOCTYPE external identifier"));
        }
        Ok(())
    }

    fn content_spec(&mut self) -> Result<(), XmlError> {
        if self.consume_keyword("EMPTY") || self.consume_keyword("ANY") {
            return Ok(());
        }
        self.expect("(")?;
        self.space();
        if self.consume_keyword("#PCDATA") {
            let mut has_names = false;
            self.space();
            while self.consume("|") {
                self.space();
                self.name()?;
                has_names = true;
                self.space();
            }
            self.expect(")")?;
            if has_names {
                self.expect("*")?;
            } else {
                self.occurrence();
            }
            return Ok(());
        }

        self.content_particle()?;
        let mut separator = None;
        loop {
            self.space();
            let next_separator = if self.consume(",") {
                Some(',')
            } else if self.consume("|") {
                Some('|')
            } else {
                None
            };
            let Some(next_separator) = next_separator else {
                break;
            };
            if separator.is_some_and(|prior| prior != next_separator) {
                return Err(self.error("mixed ',' and '|' separators in content model"));
            }
            separator = Some(next_separator);
            self.space();
            self.content_particle()?;
        }
        self.space();
        self.expect(")")?;
        self.occurrence();
        Ok(())
    }

    fn content_particle(&mut self) -> Result<(), XmlError> {
        if self.consume("(") {
            self.space();
            self.content_particle()?;
            let mut separator = None;
            loop {
                self.space();
                let next_separator = if self.consume(",") {
                    Some(',')
                } else if self.consume("|") {
                    Some('|')
                } else {
                    None
                };
                let Some(next_separator) = next_separator else {
                    break;
                };
                if separator.is_some_and(|prior| prior != next_separator) {
                    return Err(self.error("mixed ',' and '|' separators in content model"));
                }
                separator = Some(next_separator);
                self.space();
                self.content_particle()?;
            }
            self.space();
            self.expect(")")?;
        } else {
            self.name()?;
        }
        self.occurrence();
        Ok(())
    }

    fn occurrence(&mut self) {
        for marker in ["?", "*", "+"] {
            if self.consume(marker) {
                break;
            }
        }
    }

    fn attribute_type(&mut self) -> Result<(), XmlError> {
        for kind in [
            "CDATA", "IDREFS", "IDREF", "ID", "ENTITIES", "ENTITY", "NMTOKENS", "NMTOKEN",
        ] {
            if self.consume_keyword(kind) {
                return Ok(());
            }
        }
        if self.consume_keyword("NOTATION") {
            self.require_space()?;
            return self.name_enumeration();
        }
        self.name_token_enumeration()
    }

    fn name_enumeration(&mut self) -> Result<(), XmlError> {
        self.expect("(")?;
        self.space();
        self.name()?;
        self.space();
        while self.consume("|") {
            self.space();
            self.name()?;
            self.space();
        }
        self.expect(")")
    }

    fn name_token_enumeration(&mut self) -> Result<(), XmlError> {
        self.expect("(")?;
        self.space();
        self.nmtoken()?;
        self.space();
        while self.consume("|") {
            self.space();
            self.nmtoken()?;
            self.space();
        }
        self.expect(")")
    }

    fn default_declaration(&mut self) -> Result<(), XmlError> {
        if self.consume_keyword("#REQUIRED") || self.consume_keyword("#IMPLIED") {
            return Ok(());
        }
        if self.consume_keyword("#FIXED") {
            self.require_space()?;
        }
        self.attribute_value()
    }

    fn attribute_value(&mut self) -> Result<(), XmlError> {
        let quote = self
            .current()
            .ok_or_else(|| self.error("missing attribute value"))?;
        if quote != '\'' && quote != '"' {
            return Err(self.error("attribute values must be quoted"));
        }
        self.advance();
        while let Some(character) = self.current() {
            if character == quote {
                self.advance();
                return Ok(());
            }
            if character == '<' {
                return Err(self.error("'<' is not allowed in an attribute value"));
            }
            if character == '&' {
                self.advance();
                let start = self.cursor;
                while self
                    .current()
                    .is_some_and(|value| value != ';' && !is_space(value))
                {
                    self.advance();
                }
                if start == self.cursor || !self.consume(";") {
                    return Err(self.error("malformed reference in attribute value"));
                }
                continue;
            }
            self.advance();
        }
        Err(self.error("unterminated attribute value"))
    }

    fn external_id(&mut self, optional_system_literal: bool) -> Result<(), XmlError> {
        if self.consume_keyword("SYSTEM") {
            self.require_space()?;
            self.system_literal()
        } else if self.consume_keyword("PUBLIC") {
            self.require_space()?;
            self.public_literal()?;
            let had_space = self.take_space();
            if had_space
                && self
                    .current()
                    .is_some_and(|character| character == '\'' || character == '"')
            {
                self.system_literal()?;
            } else if !optional_system_literal {
                return Err(self.error("PUBLIC DOCTYPE identifier requires a system literal"));
            }
            Ok(())
        } else {
            Err(self.error("expected SYSTEM or PUBLIC external identifier"))
        }
    }

    fn system_literal(&mut self) -> Result<(), XmlError> {
        self.quoted_literal(false)
    }

    fn public_literal(&mut self) -> Result<(), XmlError> {
        self.quoted_literal(true)
    }

    fn quoted_literal(&mut self, public_id: bool) -> Result<(), XmlError> {
        let quote = self
            .current()
            .ok_or_else(|| self.error("missing quoted literal"))?;
        if quote != '\'' && quote != '"' {
            return Err(self.error("literal must be quoted"));
        }
        self.advance();
        while let Some(character) = self.current() {
            if character == quote {
                self.advance();
                return Ok(());
            }
            if public_id && !is_pubid_char(character) {
                return Err(self.error("invalid character in public identifier"));
            }
            self.advance();
        }
        Err(self.error("unterminated quoted literal"))
    }

    fn name(&mut self) -> Result<(), XmlError> {
        let (_, end) = parse_name_at(self.text, self.cursor)
            .ok_or_else(|| self.error("expected XML Name in DTD declaration"))?;
        self.cursor = end;
        Ok(())
    }

    fn nmtoken(&mut self) -> Result<(), XmlError> {
        let start = self.cursor;
        while self.current().is_some_and(is_name_char) {
            self.advance();
        }
        if start == self.cursor {
            Err(self.error("expected XML Nmtoken in DTD declaration"))
        } else {
            Ok(())
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        if !self.starts_with(keyword) {
            return false;
        }
        let end = self.cursor + keyword.len();
        if self.text[end..].chars().next().is_some_and(is_name_char) {
            return false;
        }
        self.cursor = end;
        true
    }

    fn consume(&mut self, value: &str) -> bool {
        if self.starts_with(value) {
            self.cursor += value.len();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, value: &str) -> Result<(), XmlError> {
        if self.consume(value) {
            Ok(())
        } else {
            Err(self.error(&format!("expected {value:?} in DTD declaration")))
        }
    }

    fn require_space(&mut self) -> Result<(), XmlError> {
        if self.take_space() {
            Ok(())
        } else {
            Err(self.error("expected whitespace in DTD declaration"))
        }
    }

    fn take_space(&mut self) -> bool {
        let start = self.cursor;
        self.space();
        start != self.cursor
    }

    fn space(&mut self) {
        while self.current().is_some_and(is_space) {
            self.advance();
        }
    }

    fn current(&self) -> Option<char> {
        self.text[self.cursor..].chars().next()
    }

    fn advance(&mut self) {
        if let Some(character) = self.current() {
            self.cursor += character.len_utf8();
        }
    }

    fn starts_with(&self, value: &str) -> bool {
        self.text[self.cursor..].starts_with(value)
    }

    fn error(&self, message: &str) -> XmlError {
        self.source
            .error(self.base + self.cursor, "malformed", message)
    }
}

fn is_pubid_char(character: char) -> bool {
    matches!(
        character,
        ' ' | '\r' | '\n' | 'a'..='z' | 'A'..='Z' | '0'..='9'
            | '-' | '\'' | '(' | ')' | '+' | ',' | '.' | '/' | ':' | '=' | '?'
            | ';' | '!' | '*' | '#' | '@' | '$' | '_' | '%'
    )
}

fn parse_pseudo_attributes(raw: &str) -> Result<Vec<(String, String)>, String> {
    let mut cursor = 0;
    let mut values = Vec::new();
    while cursor < raw.len() {
        let whitespace_start = cursor;
        while raw[cursor..].chars().next().is_some_and(is_space) {
            cursor += raw[cursor..].chars().next().expect("space").len_utf8();
        }
        if cursor == raw.len() {
            break;
        }
        if !values.is_empty() && cursor == whitespace_start {
            return Err(
                "XML declaration pseudo-attributes must be separated by whitespace".to_owned(),
            );
        }
        let start = cursor;
        while raw[cursor..].chars().next().is_some_and(is_name_char) {
            cursor += raw[cursor..].chars().next().expect("name char").len_utf8();
        }
        if start == cursor {
            return Err("malformed XML declaration pseudo-attribute".to_owned());
        }
        let name = &raw[start..cursor];
        while raw[cursor..].chars().next().is_some_and(is_space) {
            cursor += raw[cursor..].chars().next().expect("space").len_utf8();
        }
        if !raw[cursor..].starts_with('=') {
            return Err("expected '=' in XML declaration".to_owned());
        }
        cursor += 1;
        while raw[cursor..].chars().next().is_some_and(is_space) {
            cursor += raw[cursor..].chars().next().expect("space").len_utf8();
        }
        let quote = raw[cursor..]
            .chars()
            .next()
            .ok_or_else(|| "missing XML declaration value".to_owned())?;
        if quote != '\'' && quote != '"' {
            return Err("XML declaration values must be quoted".to_owned());
        }
        cursor += quote.len_utf8();
        let value_start = cursor;
        while !raw[cursor..].starts_with(quote) {
            let Some(character) = raw[cursor..].chars().next() else {
                return Err("unterminated XML declaration value".to_owned());
            };
            cursor += character.len_utf8();
        }
        let value = raw[value_start..cursor].to_owned();
        cursor += quote.len_utf8();
        if values.iter().any(|(prior, _)| prior == name) {
            return Err("duplicate XML declaration pseudo-attribute".to_owned());
        }
        values.push((name.to_owned(), value));
    }
    Ok(values)
}

fn parse_name_at(text: &str, start: usize) -> Option<(String, usize)> {
    let mut chars = text[start..].char_indices();
    let (_, first) = chars.next()?;
    if !is_name_start(first) {
        return None;
    }
    let mut end = start + first.len_utf8();
    for (relative, character) in chars {
        if !is_name_char(character) {
            break;
        }
        end = start + relative + character.len_utf8();
    }
    Some((text[start..end].to_owned(), end))
}

fn is_qname(name: &str) -> bool {
    let valid_ncname = |part: &str| {
        let mut characters = part.chars();
        characters.next().is_some_and(is_ncname_start) && characters.all(is_ncname_char)
    };
    match name.split_once(':') {
        Some((prefix, local)) => valid_ncname(prefix) && valid_ncname(local),
        None => valid_ncname(name),
    }
}

fn is_ncname_start(character: char) -> bool {
    character != ':' && is_name_start(character)
}

fn is_ncname_char(character: char) -> bool {
    character != ':' && is_name_char(character)
}

fn resolve_namespace(
    qualified: &str,
    namespaces: &HashMap<String, String>,
    attribute: bool,
) -> Option<String> {
    match qualified.split_once(':') {
        Some((prefix, _)) => namespaces.get(prefix).cloned(),
        None if attribute => Some(String::new()),
        None => Some(namespaces.get("").cloned().unwrap_or_default()),
    }
}

fn local_part(qualified: &str) -> &str {
    qualified
        .rsplit_once(':')
        .map_or(qualified, |(_, local)| local)
}

fn escape_xml_attribute(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        escaped.push_str(match character {
            '&' => "&amp;",
            '"' => "&quot;",
            '<' => "&lt;",
            '\t' => "&#x9;",
            '\n' => "&#xA;",
            '\r' => "&#xD;",
            _ => {
                escaped.push(character);
                continue;
            }
        });
    }
    escaped
}

fn is_space(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\r')
}

fn is_xml_char(character: char) -> bool {
    matches!(character, '\u{9}' | '\u{a}' | '\u{d}' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

fn is_name_start(character: char) -> bool {
    matches!(
        character,
        ':' | 'A'..='Z'
            | '_'
            | 'a'..='z'
            | '\u{c0}'..='\u{d6}'
            | '\u{d8}'..='\u{f6}'
            | '\u{f8}'..='\u{2ff}'
            | '\u{370}'..='\u{37d}'
            | '\u{37f}'..='\u{1fff}'
            | '\u{200c}'..='\u{200d}'
            | '\u{2070}'..='\u{218f}'
            | '\u{2c00}'..='\u{2fef}'
            | '\u{3001}'..='\u{d7ff}'
            | '\u{f900}'..='\u{fdcf}'
            | '\u{fdf0}'..='\u{fffd}'
            | '\u{10000}'..='\u{effff}'
    )
}

fn is_name_char(character: char) -> bool {
    is_name_start(character)
        || matches!(
            character,
            '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}'
        )
}
