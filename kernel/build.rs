use deravel_codegen::{
    Interface, Struct, camel_case, parse_drvli, rust_arg_type, rust_borrow_or_copy, rust_ret_type,
};
use std::fmt::Write;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let drvli_path = format!("{manifest_dir}/../interfaces.drvli");
    let drvli_text = std::fs::read_to_string(drvli_path).unwrap();
    let drvli = parse_drvli(&drvli_text);
    let mut output = String::new();
    for interface in &drvli.interfaces {
        generate_server_trait(interface, &drvli.structs, &mut output);
        generate_handler_impl(interface, &drvli.structs, &mut output);
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

fn generate_server_trait(interface: &Interface, structs: &[Struct], out: &mut String) {
    let name_snake = &interface.name;
    let name_camel = camel_case(name_snake);
    writeln!(out, "#[allow(dead_code)]").unwrap();
    writeln!(out, "pub trait {name_camel}Server {{").unwrap();
    for method in &interface.methods {
        let method_name = &method.name;
        write!(out, "    fn {method_name}(&self, sender: ProcessId").unwrap();
        for (arg_name, arg_type) in &method.args {
            let arg_type = rust_arg_type(arg_type, structs);
            write!(out, ", {arg_name}: {arg_type}").unwrap();
        }
        write!(out, ")").unwrap();
        if let Some(return_type) = &method.return_type {
            let return_type = rust_ret_type(return_type, structs);
            write!(out, " -> {return_type}").unwrap();
        }
        writeln!(out, ";").unwrap();
    }
    for stream in &interface.streams {
        let stream_name = &stream.name;
        writeln!(
            out,
            "    fn {stream_name}(&self) -> (*mut u8, usize, *mut RingBufferState);"
        )
        .unwrap();
    }
    writeln!(out, "}}").unwrap();
}

fn generate_handler_impl(interface: &Interface, structs: &[Struct], out: &mut String) {
    let name_snake = &interface.name;
    let name_camel = camel_case(name_snake);
    writeln!(
        out,
        "impl<T: {name_camel}Server> Handler<{name_camel}> for T {{"
    )
    .unwrap();
    writeln!(
        out,
        "    fn call_method(&self, method: usize, _args: &[u8], _sender: ProcessId) -> Vec<u8> {{"
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
            let arg_type = rust_ret_type(arg_type, structs);
            write!(out, "{arg_type},").unwrap();
        }
        writeln!(out, ") = serde_json::from_slice(_args).unwrap();").unwrap();
        write!(
            out,
            "                let result = self.{method_name}(_sender, "
        )
        .unwrap();
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
    writeln!(
        out,
        "    fn map_stream(&self, stream: usize) -> (*mut u8, usize, *mut RingBufferState) {{"
    )
    .unwrap();
    writeln!(out, "        match stream {{").unwrap();
    for (stream_index, stream) in interface.streams.iter().enumerate() {
        let stream_name = &stream.name;
        writeln!(out, "            {stream_index} => self.{stream_name}(),").unwrap();
    }
    writeln!(out, "            _ => unreachable!(),").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
}
