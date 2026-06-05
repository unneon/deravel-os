use deravel_codegen::{
    Interface, camel_case, parse_drvli, rust_arg_type, rust_borrow_or_copy, rust_ret_type,
    rust_stream_type,
};
use std::fmt::Write;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let drvli_path = format!("{manifest_dir}/../interfaces.drvli");
    let drvli_text = std::fs::read_to_string(drvli_path).unwrap();
    let drvli = parse_drvli(&drvli_text);
    let mut output = String::new();
    for interface in &drvli.interfaces {
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
        for stream in &interface.streams {
            let stream_name = &stream.name;
            let stream_type = rust_stream_type(&stream.type_);
            writeln!(
                &mut output,
                "    fn {stream_name}(self) -> UserRingBuffer<{stream_type}>;"
            )
            .unwrap();
        }
        writeln!(&mut output, "}}").unwrap();
        writeln!(&mut output, "pub trait {name_camel}Server {{").unwrap();
        for method in &interface.methods {
            let method_name = &method.name;
            write!(&mut output, "    fn {method_name}(&self, sender: ProcessId").unwrap();
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
        for (stream_id, stream) in interface.streams.iter().enumerate() {
            let name = &stream.name;
            let type_ = camel_case(&stream.type_);
            writeln!(
                &mut output,
                "    fn {name}(self) -> UserRingBuffer<{type_}> {{"
            )
            .unwrap();
            writeln!(&mut output, "        let (data, len, state) = unsafe {{ ipc_map_ring_buffer(self.0, {stream_id}) }};").unwrap();
            writeln!(&mut output, "        let data = data as *mut {type_};").unwrap();
            writeln!(&mut output, "        UserRingBuffer {{ data, len, state }}").unwrap();
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
                "                let result = server.{method_name}(sender,"
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
        generate_handler_impl(interface, &mut output);
    }
    std::fs::write(
        format!("{}/drvli.rs", std::env::var("OUT_DIR").unwrap()),
        output,
    )
    .unwrap();
    println!("cargo::rerun-if-changed=../interfaces.drvli");
}

fn generate_handler_impl(interface: &Interface, out: &mut String) {
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
            let arg_type = rust_ret_type(arg_type);
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
    writeln!(out, "}}").unwrap();
}
