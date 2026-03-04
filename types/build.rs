use std::fmt::Write;
use std::str::Lines;

struct Entity {
    name: String,
    details: EntityDetails,
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
        let name_snake = &interface.name;
        let name_camel = camel_case(name_snake);
        writeln!(&mut output, "#[derive(Clone, Copy)]").unwrap();
        writeln!(&mut output, "pub struct {name_camel};").unwrap();
        if let EntityDetails::App { args, implements } = &interface.details {
            writeln!(&mut output, "#[repr(C)]").unwrap();
            writeln!(&mut output, "pub struct {name_camel}Args {{").unwrap();
            for (arg_name, arg_type) in args {
                let arg_type = camel_case(arg_type);
                writeln!(&mut output, "    pub {arg_name}: Capability<{arg_type}>,").unwrap();
            }
            writeln!(&mut output, "}}").unwrap();
            writeln!(&mut output, "impl ProcessArgs for {name_camel}Args {{").unwrap();
            writeln!(
                &mut output,
                "    fn for_all(&self, mut _f: impl FnMut(RawCapability)) {{"
            )
            .unwrap();
            for (arg_name, _) in args {
                writeln!(&mut output, "        _f(self.{arg_name}.0);").unwrap();
            }
            writeln!(&mut output, "    }}").unwrap();
            writeln!(&mut output, "}}").unwrap();
            writeln!(&mut output, "impl ProcessTag for {name_camel} {{").unwrap();
            writeln!(&mut output, "    type Args = {name_camel}Args;").unwrap();
            if let Some(implements) = implements {
                let implements_camel = camel_case(implements);
                writeln!(&mut output, "    type Export = {implements_camel};").unwrap();
            } else {
                writeln!(&mut output, "    type Export = {name_camel};").unwrap();
            }
            writeln!(
                &mut output,
                "    const NAME: &'static str = \"{name_snake}\";"
            )
            .unwrap();
            writeln!(&mut output, "}}").unwrap();
        }
    }
    std::fs::write(
        format!("{}/drvli.rs", std::env::var("OUT_DIR").unwrap()),
        output,
    )
    .unwrap();
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
    while let Some(line) = lines.next()
        && line.starts_with("    ")
    {}
    Entity {
        name: name.to_owned(),
        details,
    }
}

fn camel_case(name: &str) -> String {
    let mut camel = String::new();
    for segment in name.split('_') {
        camel.push(segment.as_bytes()[0].to_ascii_uppercase() as char);
        camel += &segment[1..];
    }
    camel
}
