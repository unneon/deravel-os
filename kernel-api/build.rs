use std::borrow::Cow;
use std::fmt::Write;
use std::str::Lines;

struct Entity {
    name: String,
    methods: Vec<Method>,
}

struct Method {
    name: String,
    args: Vec<(String, String)>,
    return_type: Option<String>,
}

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let interfaces_path = format!("{manifest_dir}/../interfaces.drvli");
    let interfaces = parse_interfaces(&std::fs::read_to_string(interfaces_path).unwrap());
    let mut output = String::new();
    for interface in &interfaces {
        let name_snake = &interface.name;
        let name_camel = camel_case(name_snake);
        writeln!(&mut output, "pub trait {name_camel}Client {{").unwrap();
        for method in &interface.methods {
            let method_name = &method.name;
            write!(&mut output, "    fn {method_name}(self").unwrap();
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
        writeln!(&mut output, "pub trait {name_camel}Server {{").unwrap();
        for method in &interface.methods {
            let method_name = &method.name;
            write!(
                &mut output,
                "    fn {method_name}(&mut self, cap: Capability<{name_camel}>, sender: ProcessId"
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
        writeln!(
            &mut output,
            "impl {name_camel}Client for Capability<{name_camel}> {{"
        )
        .unwrap();
        for (method_id, method) in interface.methods.iter().enumerate() {
            let name = &method.name;
            write!(&mut output, "    fn {name}(self").unwrap();
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
            writeln!(&mut output, "        let result_len = unsafe {{ ipc_call(self.0, {method_id}, data.as_ptr(), data.len(), buf.as_mut_ptr(), buf.len()) }};").unwrap();
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
            "pub fn ipc_serve_{name_snake}(mut server: impl {name_camel}Server) -> ! {{"
        )
        .unwrap();
        writeln!(&mut output, "    loop {{").unwrap();
        writeln!(&mut output, "        let mut buf = [0u8; 4096];").unwrap();
        writeln!(&mut output, "        let (cap, method, args_len, sender) = unsafe {{ ipc_receive(buf.as_mut_ptr(), buf.len()) }};").unwrap();
        writeln!(
            &mut output,
            "        let cap = Capability::<{name_camel}>(cap, PhantomData);"
        )
        .unwrap();
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
        "u64" => "u64".into(),
        "text" => "&str".into(),
        "bytes" => "&[u8]".into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
    }
}

fn rust_ret_type(type_: &str) -> Cow<'static, str> {
    match type_ {
        "u64" => "u64".into(),
        "text" => "String".into(),
        "bytes" => "Vec<u8>".into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
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
            let entity = parse_entity(name, &mut lines);
            parsed.push(entity);
        } else if let Some(name) = line.strip_prefix("interface ") {
            let entity = parse_entity(name, &mut lines);
            parsed.push(entity);
        }
    }
    parsed
}

fn parse_entity(name: &str, lines: &mut Lines) -> Entity {
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
