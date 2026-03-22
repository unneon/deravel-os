use deravel_codegen::{
    Entity, camel_case, parse_interfaces, rust_arg_type, rust_borrow_or_copy, rust_ret_type,
};
use std::fmt::Write;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let interfaces_path = format!("{manifest_dir}/../interfaces.drvli");
    let interfaces = parse_interfaces(&std::fs::read_to_string(interfaces_path).unwrap());
    let mut output = String::new();
    for interface in &interfaces {
        generate_server_trait(interface, &mut output);
        generate_handler_impl(interface, &mut output);
    }
    std::fs::write(
        format!("{}/drvli.rs", std::env::var("OUT_DIR").unwrap()),
        output,
    )
    .unwrap();
    println!("cargo::rerun-if-changed=../interfaces.drvli");
    println!("cargo::rerun-if-changed=kernel.ld");
    println!("cargo::rustc-link-arg=-Tkernel/kernel.ld");
}

fn generate_server_trait(interface: &Entity, out: &mut String) {
    let name_snake = &interface.name;
    let name_camel = camel_case(name_snake);
    writeln!(out, "#[allow(dead_code)]").unwrap();
    writeln!(out, "pub trait {name_camel}Server {{").unwrap();
    for method in &interface.methods {
        let method_name = &method.name;
        write!(out, "    fn {method_name}(&self").unwrap();
        for (arg_name, arg_type) in &method.args {
            let arg_type = rust_arg_type(arg_type);
            write!(out, ", {arg_name}: {arg_type}").unwrap();
        }
        write!(out, ")").unwrap();
        if let Some(return_type) = &method.return_type {
            let return_type = rust_ret_type(return_type);
            write!(out, " -> {return_type}").unwrap();
        }
        writeln!(out, ";").unwrap();
    }
    writeln!(out, "}}").unwrap();
}

fn generate_handler_impl(interface: &Entity, out: &mut String) {
    let name_snake = &interface.name;
    let name_camel = camel_case(name_snake);
    writeln!(out, "impl Handler for Box<dyn {name_camel}Server> {{").unwrap();
    writeln!(
        out,
        "    fn handle(&self, method: usize, _args: &[u8]) -> Vec<u8> {{"
    )
    .unwrap();
    writeln!(out, "        match method {{").unwrap();
    for (method_index, method) in interface.methods.iter().enumerate() {
        let method_name = &method.name;
        writeln!(out, "            {method_index} => {{").unwrap();
        write!(out, "                let (").unwrap();
        for (arg_name, _) in &method.args {
            write!(out, "{arg_name},").unwrap();
        }
        write!(out, "): (").unwrap();
        for (_, arg_type) in &method.args {
            let arg_type = rust_ret_type(arg_type);
            write!(out, "{arg_type},").unwrap();
        }
        writeln!(out, ") = serde_json::from_slice(_args).unwrap();").unwrap();
        write!(out, "                let result = self.{method_name}(").unwrap();
        for (arg_name, arg_type) in &method.args {
            let borrow = rust_borrow_or_copy(arg_type);
            write!(out, "{borrow}{arg_name},").unwrap();
        }
        writeln!(out, ");").unwrap();
        writeln!(out, "                serde_json::to_vec(&result).unwrap()").unwrap();
        writeln!(out, "            }}").unwrap();
    }
    writeln!(out, "            _ => unreachable!(),").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
}
