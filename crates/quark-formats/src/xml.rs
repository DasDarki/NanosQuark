use mlua::prelude::*;
use quark_core::{Options, options_and_callback};
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer, XmlVersion};

use crate::data::text;

const MAX_DEPTH: usize = 256;
const DEFAULT_INDENT: usize = 2;

pub enum Node {
    Element(Element),
    Text(String),
}

pub struct Element {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<Node>,
}

impl IntoLua for Element {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        table.set("Name", self.name)?;

        let attributes = lua.create_table()?;
        for (name, value) in self.attributes {
            attributes.set(name, value)?;
        }
        table.set("Attributes", attributes)?;

        let children = lua.create_table()?;
        for child in self.children {
            match child {
                Node::Element(element) => children.raw_push(element)?,
                Node::Text(content) => children.raw_push(content)?,
            }
        }
        table.set("Children", children)?;
        Ok(LuaValue::Table(table))
    }
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set("Decode", lua.create_function(decode)?)?;
    module.set("Encode", lua.create_function(encode)?)?;

    let helpers = lua
        .load(
            r#"
            local ipairs, type, concat = ipairs, type, table.concat

            local function find(element, name)
                for _, child in ipairs(element.Children or {}) do
                    if type(child) == "table" and child.Name == name then
                        return child
                    end
                end
                return nil
            end

            local function find_all(element, name)
                local found = {}
                for _, child in ipairs(element.Children or {}) do
                    if type(child) == "table" and (name == nil or child.Name == name) then
                        found[#found + 1] = child
                    end
                end
                return found
            end

            local function collect(element, parts)
                for _, child in ipairs(element.Children or {}) do
                    if type(child) == "string" then
                        parts[#parts + 1] = child
                    else
                        collect(child, parts)
                    end
                end
                return parts
            end

            local function get_text(element)
                return concat(collect(element, {}))
            end

            return find, find_all, get_text
            "#,
        )
        .set_name("=quark/xml")
        .call::<(LuaFunction, LuaFunction, LuaFunction)>(())?;
    module.set("Find", helpers.0)?;
    module.set("FindAll", helpers.1)?;
    module.set("GetText", helpers.2)?;

    Ok(module)
}

fn decode(
    lua: &Lua,
    (input, options, callback): (quark_core::Binary, LuaValue, Option<LuaFunction>),
) -> LuaResult<LuaMultiValue> {
    let (options, callback) = options_and_callback(options, callback)?;
    let preserve_whitespace = options.get_or("PreserveWhitespace", false)?;
    quark_core::run(lua, callback, move || {
        parse(text(input.as_bytes())?, preserve_whitespace)
            .map_err(|error| format!("invalid XML: {error}"))
    })
}

fn parse(source: &str, preserve_whitespace: bool) -> Result<Element, String> {
    let mut reader = Reader::from_str(source);
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;

    let push_text = |stack: &mut Vec<Element>, content: String| {
        if let Some(parent) = stack.last_mut() {
            if let Some(Node::Text(previous)) = parent.children.last_mut() {
                previous.push_str(&content);
            } else {
                parent.children.push(Node::Text(content));
            }
        }
    };

    loop {
        let event = reader.read_event().map_err(|error| error.to_string())?;
        match event {
            Event::Start(start) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(format!(
                        "the document is nested deeper than {MAX_DEPTH} levels"
                    ));
                }
                if root.is_some() && stack.is_empty() {
                    return Err("the document has more than one root element".to_owned());
                }
                stack.push(element(&start)?);
            }
            Event::Empty(start) => {
                let element = element(&start)?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Element(element)),
                    None if root.is_none() => root = Some(element),
                    None => return Err("the document has more than one root element".to_owned()),
                }
            }
            Event::End(_) => {
                let mut finished = stack.pop().ok_or("unexpected closing tag")?;
                if !preserve_whitespace {
                    finished.children.retain(
                        |child| !matches!(child, Node::Text(content) if content.trim().is_empty()),
                    );
                }
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Element(finished)),
                    None => root = Some(finished),
                }
            }
            Event::Text(content) => {
                if !stack.is_empty() {
                    push_text(&mut stack, content.xml10_content().into_owned());
                } else if !content.xml10_content().trim().is_empty() {
                    return Err("text outside of the root element".to_owned());
                }
            }
            Event::CData(content) => push_text(&mut stack, content.xml10_content().into_owned()),
            Event::GeneralRef(reference) => {
                let resolved = if reference.is_char_ref() {
                    reference
                        .resolve_char_ref()
                        .map_err(|error| error.to_string())?
                        .map(String::from)
                } else {
                    resolve_predefined_entity(&reference.xml10_content()).map(str::to_owned)
                };
                let resolved = resolved
                    .ok_or_else(|| format!("unknown entity '&{};'", reference.xml10_content()))?;
                push_text(&mut stack, resolved);
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if !stack.is_empty() {
        return Err(format!(
            "the element '{}' is not closed",
            stack[stack.len() - 1].name
        ));
    }
    root.ok_or_else(|| "the document has no root element".to_owned())
}

fn element(start: &BytesStart) -> Result<Element, String> {
    let name = start.name().as_ref().to_owned();
    let mut attributes = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| error.to_string())?;
        let value = attribute
            .normalized_value(XmlVersion::default())
            .map_err(|error| error.to_string())?;
        attributes.push((attribute.key.as_ref().to_owned(), value.into_owned()));
    }
    Ok(Element {
        name,
        attributes,
        children: Vec::new(),
    })
}

fn encode(
    lua: &Lua,
    (value, options, callback): (LuaTable, LuaValue, Option<LuaFunction>),
) -> LuaResult<LuaMultiValue> {
    let (options, callback) = options_and_callback(options, callback)?;
    let root = read_element(&value, 0)?;
    let indent = indent(&options)?;
    let declaration = options.get_or("Declaration", true)?;

    quark_core::run(lua, callback, move || {
        let output = Vec::new();
        let mut writer = match indent {
            Some(indent) => Writer::new_with_indent(output, b' ', indent),
            None => Writer::new(output),
        };
        let failed = |error: std::io::Error| format!("cannot encode XML: {error}");
        if declaration {
            writer
                .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
                .map_err(failed)?;
        }
        write_element(&mut writer, &root).map_err(failed)?;
        String::from_utf8(writer.into_inner()).map_err(|error| error.to_string())
    })
}

fn indent(options: &Options) -> LuaResult<Option<usize>> {
    Ok(
        match (
            options.get_or("Pretty", false)?,
            options.get::<usize>("Indent")?,
        ) {
            (_, Some(indent)) => Some(indent),
            (true, None) => Some(DEFAULT_INDENT),
            (false, None) => None,
        },
    )
}

fn read_element(table: &LuaTable, depth: usize) -> LuaResult<Element> {
    if depth >= MAX_DEPTH {
        return Err(LuaError::runtime(format!(
            "the element tree is nested deeper than {MAX_DEPTH} levels"
        )));
    }

    let name: String = table
        .get::<Option<String>>("Name")?
        .ok_or_else(|| LuaError::runtime("every element needs a Name"))?;
    if name.is_empty()
        || name
            .contains(|character: char| character.is_whitespace() || "<>&\"'/=".contains(character))
    {
        return Err(LuaError::runtime(format!(
            "'{name}' is not a valid element name"
        )));
    }

    let mut attributes = Vec::new();
    if let Some(table) = table.get::<Option<LuaTable>>("Attributes")? {
        for pair in table.pairs::<String, LuaValue>() {
            let (key, value) = pair?;
            attributes.push((key, value.to_string()?));
        }
    }
    attributes.sort();

    let mut children = Vec::new();
    if let Some(list) = table.get::<Option<LuaTable>>("Children")? {
        for child in list.sequence_values::<LuaValue>() {
            match child? {
                LuaValue::Table(child) => {
                    children.push(Node::Element(read_element(&child, depth + 1)?))
                }
                value @ (LuaValue::String(_)
                | LuaValue::Integer(_)
                | LuaValue::Number(_)
                | LuaValue::Boolean(_)) => children.push(Node::Text(value.to_string()?)),
                other => {
                    return Err(LuaError::runtime(format!(
                        "children have to be elements or text, got {}",
                        other.type_name()
                    )));
                }
            }
        }
    }

    Ok(Element {
        name,
        attributes,
        children,
    })
}

fn write_element<W: std::io::Write>(
    writer: &mut Writer<W>,
    element: &Element,
) -> std::io::Result<()> {
    let mut start = BytesStart::new(element.name.as_str());
    for (key, value) in &element.attributes {
        start.push_attribute((key.as_str(), value.as_str()));
    }

    if element.children.is_empty() {
        return writer.write_event(Event::Empty(start));
    }

    writer.write_event(Event::Start(start))?;
    for child in &element.children {
        match child {
            Node::Element(child) => write_element(writer, child)?,
            Node::Text(content) => writer.write_event(Event::Text(BytesText::new(content)))?,
        }
    }
    writer.write_event(Event::End(BytesEnd::new(element.name.as_str())))
}
