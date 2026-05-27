use std::borrow::Cow;
use std::iter::Peekable;
use std::str::Lines;

pub struct Drvli {
    pub interfaces: Vec<Interface>,
    pub structs: Vec<Struct>,
}

pub struct Struct {
    pub name: String,
    pub members: Vec<(String, String)>,
}

pub struct Interface {
    pub name: String,
    pub methods: Vec<Method>,
    pub streams: Vec<Stream>,
    pub details: InterfaceDetails,
}

pub struct Method {
    pub name: String,
    pub args: Vec<(String, String)>,
    pub return_type: Option<String>,
}

pub struct Stream {
    pub name: String,
    pub type_: String,
}

pub enum InterfaceDetails {
    App {
        args: Vec<(String, String)>,
        implements: Option<String>,
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

pub fn parse_drvli(text: &str) -> Drvli {
    let mut lines = text.lines().peekable();
    let mut structs = Vec::new();
    let mut interfaces = Vec::new();
    while let Some(line) = lines.next() {
        if let Some(struct_name) = line.strip_prefix("struct ") {
            let mut members = Vec::new();
            while let Some(line) = lines.peek()
                && let Some(line) = line.strip_prefix("    ")
            {
                let (member_name, member_type) = line.split_once(' ').unwrap();
                members.push((member_name.to_owned(), member_type.to_owned()));
                lines.next();
            }
            structs.push(Struct {
                name: struct_name.to_owned(),
                members,
            });
        } else if let Some(line) = line.strip_prefix("app ") {
            let name_len = line.find(['(', ' ']).unwrap_or(line.len());
            let name = &line[..name_len];
            let line = &line[name_len..];
            let (args, line) = if let Some(line) = line.strip_prefix("(") {
                let (args, line) = line.split_once(')').unwrap();
                let args = args
                    .split(", ")
                    .filter(|arg| !arg.is_empty())
                    .map(|arg| {
                        let (name, type_) = arg.split_once(' ').unwrap();
                        (name.to_owned(), type_.to_owned())
                    })
                    .collect();
                (args, line)
            } else {
                (Vec::new(), line)
            };
            let line = line.trim();
            let implements = line.strip_prefix("implements ").map(str::to_owned);
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

pub fn parse_interface(
    name: &str,
    lines: &mut Peekable<Lines>,
    details: InterfaceDetails,
) -> Interface {
    let mut methods = Vec::new();
    let mut streams = Vec::new();
    while let Some(line) = lines.next()
        && let Some(line) = line.strip_prefix("    ")
    {
        if let Some(line) = line.strip_prefix("func ") {
            let (method_name, line) = line.split_once('(').unwrap();
            let (method_args, line) = line.split_once(")").unwrap();
            let method_args: Vec<_> = method_args
                .split(", ")
                .filter(|s| !s.is_empty())
                .map(|arg| {
                    let (name, type_) = arg.split_once(' ').unwrap();
                    (name.to_owned(), type_.to_owned())
                })
                .collect();
            let method_return_type = line.strip_prefix(" ").map(str::to_owned);
            methods.push(Method {
                name: method_name.to_owned(),
                args: method_args,
                return_type: method_return_type,
            });
        } else if let Some(line) = line.strip_prefix("stream ") {
            let (stream_name, stream_type) = line.split_once(' ').unwrap();
            streams.push(Stream {
                name: stream_name.to_owned(),
                type_: stream_type.to_owned(),
            });
        }
    }
    Interface {
        name: name.to_owned(),
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
