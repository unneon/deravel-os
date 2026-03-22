use std::borrow::Cow;
use std::str::Lines;

pub struct Entity {
    pub name: String,
    pub methods: Vec<Method>,
    pub details: EntityDetails,
}

pub struct Method {
    pub name: String,
    pub args: Vec<(String, String)>,
    pub return_type: Option<String>,
}

pub enum EntityDetails {
    App {
        args: Vec<(String, String)>,
        implements: Option<String>,
    },
    Interface,
}

pub fn rust_arg_type(type_: &str) -> Cow<'static, str> {
    match type_ {
        "u64" => "u64".into(),
        "text" => "&str".into(),
        "bytes" => "&[u8]".into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
    }
}

pub fn rust_ret_type(type_: &str) -> Cow<'static, str> {
    match type_ {
        "u64" => "u64".into(),
        "text" => "String".into(),
        "bytes" => "Vec<u8>".into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
    }
}

pub fn rust_borrow_or_copy(type_: &str) -> &'static str {
    match type_ {
        "text" => "&",
        "bytes" => "&",
        _ => "",
    }
}

pub fn parse_interfaces(text: &str) -> Vec<Entity> {
    let mut lines = text.lines();
    let mut parsed = Vec::new();
    while let Some(line) = lines.next() {
        if let Some(line) = line.strip_prefix("app ") {
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
            let entity = parse_entity(name, &mut lines, EntityDetails::App { args, implements });
            parsed.push(entity);
        } else if let Some(name) = line.strip_prefix("interface ") {
            let entity = parse_entity(name, &mut lines, EntityDetails::Interface);
            parsed.push(entity);
        }
    }
    parsed
}

pub fn parse_entity(name: &str, lines: &mut Lines, details: EntityDetails) -> Entity {
    let mut methods = Vec::new();
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
        }
    }
    Entity {
        name: name.to_owned(),
        methods,
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
