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
        let name = &interface.name;
        writeln!(&mut output, "#[derive(Clone, Copy)]").unwrap();
        writeln!(&mut output, "pub struct {name};").unwrap();
        if let EntityDetails::App { args, implements } = &interface.details {
            writeln!(&mut output, "#[repr(C)]").unwrap();
            writeln!(&mut output, "pub struct {name}ArgsRaw {{").unwrap();
            for (arg_name, _) in args {
                writeln!(&mut output, "    pub {arg_name}: Capability,").unwrap();
            }
            writeln!(&mut output, "}}").unwrap();
            writeln!(&mut output, "impl CapabilityContainer for {name}ArgsRaw {{").unwrap();
            writeln!(
                &mut output,
                "    fn for_all(&self, mut _f: impl FnMut(Capability)) {{"
            )
            .unwrap();
            for (arg_name, _) in args {
                writeln!(&mut output, "        _f(self.{arg_name});").unwrap();
            }
            writeln!(&mut output, "    }}").unwrap();
            writeln!(&mut output, "}}").unwrap();
            writeln!(&mut output, "impl ProcessTag for {name} {{").unwrap();
            writeln!(&mut output, "    type Capabilities = {name}ArgsRaw;").unwrap();
            if let Some(implements) = implements {
                writeln!(&mut output, "    type Export = {implements};").unwrap();
            } else {
                writeln!(&mut output, "    type Export = {name};").unwrap();
            }
            writeln!(&mut output, "    const NAME: &'static str = \"{name}\";").unwrap();
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
