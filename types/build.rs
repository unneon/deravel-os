use std::fmt::Write;
use std::str::Lines;

struct Entity {
    name: String,
    methods: Vec<Method>,
    details: EntityDetails,
}

struct Method {
    name: String,
    args: Vec<(String, String)>,
    return_type: Option<String>,
}

enum EntityDetails {
    App {
        args: Vec<(String, String)>,
        implements: Option<String>,
    },
    Interface,
}

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let interfaces_path = format!("{manifest_dir}/../interfaces.drvli");
    let interfaces = parse_interfaces(&std::fs::read_to_string(interfaces_path).unwrap());
    let mut output = String::new();
    for interface in &interfaces {
        let name = &interface.name;
        writeln!(&mut output, "#[derive(Clone, Copy)]").unwrap();
        writeln!(&mut output, "pub struct {name};").unwrap();
        if let EntityDetails::App { args, .. } = &interface.details {
            writeln!(&mut output, "pub mod {name}_prelude {{").unwrap();
            writeln!(&mut output, "    pub struct Capabilities {{").unwrap();
            for (arg_name, arg_type) in args {
                writeln!(&mut output, "        pub {arg_name}: crate::capability::CallableCapability<crate::interfaces::{arg_type}>,").unwrap();
            }
            writeln!(&mut output, "    }}").unwrap();
            writeln!(&mut output, "}}").unwrap();
            writeln!(&mut output, "#[repr(C)]").unwrap();
            writeln!(&mut output, "pub struct {name}Args {{").unwrap();
            for (arg_name, _arg_type) in args {
                writeln!(&mut output, "    pub {arg_name}: Capability,").unwrap();
            }
            writeln!(&mut output, "}}").unwrap();
        }
        writeln!(&mut output, "impl CallableCapability<{name}> {{").unwrap();
        for method in &interface.methods {
            let name = &method.name;
            write!(&mut output, "    pub fn {name}(self").unwrap();
            for (arg_name, arg_type) in &method.args {
                let arg_type = rust_type(arg_type);
                write!(&mut output, ", {arg_name}: {arg_type}").unwrap();
            }
            write!(&mut output, ")").unwrap();
            if let Some(return_type) = &method.return_type {
                let return_type = rust_type(return_type);
                write!(&mut output, " -> {return_type}").unwrap();
            }
            writeln!(&mut output, " {{").unwrap();
            writeln!(&mut output, "        todo!()").unwrap();
            writeln!(&mut output, "    }}").unwrap();
        }
        writeln!(&mut output, "}}").unwrap();
    }
    std::fs::write(
        format!("{}/interfaces.rs", std::env::var("OUT_DIR").unwrap()),
        output,
    )
    .unwrap();
}

fn rust_type(type_: &str) -> &str {
    match type_ {
        "text" => "&str",
        "bytes" => "&[u8]",
        _ => "Capability",
    }
}

fn parse_interfaces(text: &str) -> Vec<Entity> {
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
            let entity = parse_entity(name, EntityDetails::App { args, implements }, &mut lines);
            parsed.push(entity);
        } else if let Some(name) = line.strip_prefix("interface ") {
            let entity = parse_entity(name, EntityDetails::Interface, &mut lines);
            parsed.push(entity);
        }
    }
    parsed
}

fn parse_entity(name: &str, details: EntityDetails, lines: &mut Lines) -> Entity {
    let mut methods = Vec::new();
    while let Some(line) = lines.next()
        && let Some(line) = line.strip_prefix("    ")
    {
        if let Some(line) = line.strip_prefix("func ") {
            let (method_name, line) = line.split_once('(').unwrap();
            let (method_args, line) = line.split_once(")").unwrap();
            let method_args: Vec<_> = method_args
                .split(", ")
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
