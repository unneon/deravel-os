use std::borrow::Cow;
use std::iter::Peekable;
use std::str::Lines;

pub struct Drvli<'a> {
    pub interfaces: Vec<Interface<'a>>,
    pub structs: Vec<Struct<'a>>,
}

pub struct Struct<'a> {
    pub name: &'a str,
    pub members: Vec<(&'a str, &'a str)>,
}

pub struct Interface<'a> {
    pub name: &'a str,
    pub methods: Vec<Method<'a>>,
    pub streams: Vec<Stream<'a>>,
    pub details: InterfaceDetails<'a>,
}

pub struct Method<'a> {
    pub name: &'a str,
    pub args: Vec<(&'a str, &'a str)>,
    pub return_type: Option<&'a str>,
}

pub struct Stream<'a> {
    pub name: &'a str,
    pub type_: &'a str,
}

pub enum InterfaceDetails<'a> {
    App {
        args: Vec<(&'a str, &'a str)>,
        implements: Option<&'a str>,
    },
    Interface,
}

pub fn rust_arg_type(type_: &str) -> Cow<'static, str> {
    match type_ {
        "u8" => "u8".into(),
        "u32" => "u32".into(),
        "u64" => "u64".into(),
        "text" => "&str".into(),
        "bytes" => "&[u8]".into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
    }
}

pub fn rust_member_type(type_: &str) -> Cow<'static, str> {
    match type_ {
        "u16" => "u16".into(),
        "u32" => "u32".into(),
        _ => unimplemented!(),
    }
}

pub fn rust_ret_type(type_: &str) -> Cow<'static, str> {
    match type_ {
        "u8" => "u8".into(),
        "u32" => "u32".into(),
        "u64" => "u64".into(),
        "text" => "String".into(),
        "bytes" => "Vec<u8>".into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
    }
}

pub fn rust_stream_type(type_: &str) -> Cow<'static, str> {
    match type_ {
        _ => camel_case(type_).into(),
    }
}

pub fn rust_borrow_or_copy(type_: &str) -> &'static str {
    match type_ {
        "text" => "&",
        "bytes" => "&",
        _ => "",
    }
}

pub fn rust_escape_name(name: &str) -> &str {
    match name {
        "type" => "type_",
        _ => name,
    }
}

pub fn parse_drvli(text: &str) -> Drvli<'_> {
    let mut lines = text.lines().peekable();
    let mut structs = Vec::new();
    let mut interfaces = Vec::new();
    while let Some(line) = lines.next() {
        if let Some(name) = line.strip_prefix("struct ") {
            let mut members = Vec::new();
            while let Some(line) = lines.peek()
                && let Some(line) = line.strip_prefix("    ")
            {
                members.push(line.split_once(' ').unwrap());
                lines.next();
            }
            structs.push(Struct { name, members });
        } else if let Some(line) = line.strip_prefix("app ") {
            let name_len = line.find(['(', ' ']).unwrap_or(line.len());
            let name = &line[..name_len];
            let line = &line[name_len..];
            let (args, line) = if let Some(line) = line.strip_prefix("(") {
                let (args, line) = line.split_once(')').unwrap();
                let args = args
                    .split(", ")
                    .filter(|arg| !arg.is_empty())
                    .map(|arg| arg.split_once(' ').unwrap())
                    .collect();
                (args, line)
            } else {
                (Vec::new(), line)
            };
            let line = line.trim();
            let implements = line.strip_prefix("implements ");
            let interface =
                parse_interface(name, &mut lines, InterfaceDetails::App { args, implements });
            interfaces.push(interface);
        } else if let Some(name) = line.strip_prefix("interface ") {
            let interface = parse_interface(name, &mut lines, InterfaceDetails::Interface);
            interfaces.push(interface);
        }
    }
    Drvli {
        interfaces,
        structs,
    }
}

pub fn parse_interface<'a>(
    name: &'a str,
    lines: &mut Peekable<Lines<'a>>,
    details: InterfaceDetails<'a>,
) -> Interface<'a> {
    let mut methods = Vec::new();
    let mut streams = Vec::new();
    while let Some(line) = lines.next()
        && let Some(line) = line.strip_prefix("    ")
    {
        if let Some(line) = line.strip_prefix("func ") {
            let (name, line) = line.split_once('(').unwrap();
            let (method_args, line) = line.split_once(")").unwrap();
            let args: Vec<_> = method_args
                .split(", ")
                .filter(|s| !s.is_empty())
                .map(|arg| arg.split_once(' ').unwrap())
                .collect();
            let return_type = line.strip_prefix(" ");
            methods.push(Method {
                name,
                args,
                return_type,
            });
        } else if let Some(line) = line.strip_prefix("stream ") {
            let (name, type_) = line.split_once(' ').unwrap();
            streams.push(Stream { name, type_ });
        }
    }
    Interface {
        name,
        methods,
        streams,
        details,
    }
}

pub fn camel_case(name: &str) -> String {
    let mut camel = String::new();
    for segment in name.split('_') {
        camel.push(segment.as_bytes()[0].to_ascii_uppercase() as char);
        camel += &segment[1..];
    }
    camel
}
