use std::borrow::Cow;
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
    App { args: Vec<(String, String)> },
    Interface,
}

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let interfaces_path = format!("{manifest_dir}/../interfaces.drvli");
    let interfaces = parse_interfaces(&std::fs::read_to_string(interfaces_path).unwrap());
    let mut output = String::new();
    for interface in &interfaces {
        let name = &interface.name;
        writeln!(&mut output, "pub trait {name}Server {{").unwrap();
        for method in &interface.methods {
            let method_name = &method.name;
            write!(
                &mut output,
                "    fn {method_name}(&mut self, cap: Capability, sender: ProcessId"
            )
            .unwrap();
            for (arg_name, arg_type) in &method.args {
                let arg_type = rust_arg_type(arg_type);
                write!(&mut output, ", {arg_name}: {arg_type}").unwrap();
            }
            write!(&mut output, ")").unwrap();
            if let Some(return_type) = &method.return_type {
                let return_type = rust_ret_type(return_type);
                write!(&mut output, " -> {return_type}").unwrap();
            }
            writeln!(&mut output, ";").unwrap();
        }
        writeln!(&mut output, "}}").unwrap();
        if let EntityDetails::App { args, .. } = &interface.details {
            writeln!(&mut output, "impl App for {name} {{").unwrap();
            writeln!(&mut output, "    type Args = {name}Args;").unwrap();
            writeln!(&mut output, "}}").unwrap();
            writeln!(&mut output, "#[repr(C)]").unwrap();
            writeln!(&mut output, "pub struct {name}Args {{").unwrap();
            for (arg_name, arg_type) in args {
                writeln!(
                    &mut output,
                    "    pub {arg_name}: CallableCapability<{arg_type}>,"
                )
                .unwrap();
            }
            writeln!(&mut output, "}}").unwrap();
        }
        writeln!(&mut output, "impl CallableCapability<{name}> {{").unwrap();
        for (method_id, method) in interface.methods.iter().enumerate() {
            let name = &method.name;
            write!(&mut output, "    pub fn {name}(self").unwrap();
            for (arg_name, arg_type) in &method.args {
                let arg_type = rust_arg_type(arg_type);
                write!(&mut output, ", {arg_name}: {arg_type}").unwrap();
            }
            write!(&mut output, ")").unwrap();
            if let Some(return_type) = &method.return_type {
                let return_type = rust_ret_type(return_type);
                write!(&mut output, " -> {return_type}").unwrap();
            }
            writeln!(&mut output, " {{").unwrap();
            writeln!(&mut output, "        let data = serde_json::to_vec(&(").unwrap();
            for (arg_name, _) in &method.args {
                writeln!(&mut output, "            {arg_name},",).unwrap();
            }
            writeln!(&mut output, "        )).unwrap();").unwrap();
            writeln!(&mut output, "        let mut buf = [0u8; 4096];").unwrap();
            writeln!(&mut output, "        let result_len = unsafe {{ ipc_call(Capability(self.0), {method_id}, data.as_ptr(), data.len(), buf.as_mut_ptr(), buf.len()) }};").unwrap();
            writeln!(
                &mut output,
                "        serde_json::from_slice(&buf[..result_len]).unwrap()"
            )
            .unwrap();
            writeln!(&mut output, "    }}").unwrap();
        }
        writeln!(&mut output, "}}").unwrap();
        writeln!(
            &mut output,
            "pub fn ipc_serve_{name}(mut server: impl {name}Server) -> ! {{"
        )
        .unwrap();
        writeln!(&mut output, "    loop {{").unwrap();
        writeln!(&mut output, "        let mut buf = [0u8; 4096];").unwrap();
        writeln!(&mut output, "        let (cap, method, args_len, sender) = unsafe {{ ipc_receive(buf.as_mut_ptr(), buf.len()) }};").unwrap();
        writeln!(&mut output, "        match method {{").unwrap();
        for (method_id, method) in interface.methods.iter().enumerate() {
            let method_name = &method.name;
            writeln!(&mut output, "            {method_id} => {{").unwrap();
            writeln!(&mut output, "                let (").unwrap();
            for (arg_name, _) in &method.args {
                writeln!(&mut output, "                    {arg_name},").unwrap();
            }
            writeln!(&mut output, "                ): (").unwrap();
            for (_, arg_type) in &method.args {
                let arg_type = rust_ret_type(arg_type);
                writeln!(&mut output, "                    {arg_type},").unwrap();
            }
            writeln!(
                &mut output,
                "                ) = serde_json::from_slice(&buf[..args_len]).unwrap();"
            )
            .unwrap();
            writeln!(
                &mut output,
                "                let result = server.{method_name}(cap, sender,"
            )
            .unwrap();
            for (arg_name, arg_type) in &method.args {
                let borrow = rust_borrow_or_copy(arg_type);
                writeln!(&mut output, "                    {borrow}{arg_name},").unwrap();
            }
            writeln!(&mut output, "                );").unwrap();
            writeln!(
                &mut output,
                "                let result = serde_json::to_vec(&result).unwrap();"
            )
            .unwrap();
            writeln!(
                &mut output,
                "                unsafe {{ ipc_reply(result.as_ptr(), result.len()) }};"
            )
            .unwrap();
            writeln!(&mut output, "            }},").unwrap();
        }
        writeln!(&mut output, "            _ => unreachable!(),").unwrap();
        writeln!(&mut output, "        }}").unwrap();
        writeln!(&mut output, "        ").unwrap();
        writeln!(&mut output, "    }}").unwrap();
        writeln!(&mut output, "}}").unwrap();
    }
    std::fs::write(
        format!("{}/drvli.rs", std::env::var("OUT_DIR").unwrap()),
        output,
    )
    .unwrap();
}

fn rust_arg_type(type_: &str) -> Cow<'static, str> {
    match type_ {
        "text" => "&str".into(),
        "bytes" => "&[u8]".into(),
        _ => format!("CallableCapability<{type_}>").into(),
    }
}

fn rust_ret_type(type_: &str) -> Cow<'static, str> {
    match type_ {
        "text" => "String".into(),
        "bytes" => "Vec<u8>".into(),
        _ => format!("CallableCapability<{type_}>").into(),
    }
}

fn rust_borrow_or_copy(type_: &str) -> &'static str {
    match type_ {
        "text" => "&",
        "bytes" => "&",
        _ => "",
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
            let (args, _) = if let Some(line) = line.strip_prefix("(") {
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
            let entity = parse_entity(name, EntityDetails::App { args }, &mut lines);
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
