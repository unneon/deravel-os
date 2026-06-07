use deravel_codegen::{
    Drvli, Interface, camel_case, parse_drvli, rust_arg_type, rust_borrow_or_copy,
    rust_grantable_ret_type, rust_normal_ret_type, rust_stream_type,
};
use std::fmt::Write;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let drvli_path = format!("{manifest_dir}/../interfaces.drvli");
    let drvli_text = std::fs::read_to_string(drvli_path).unwrap();
    let drvli = parse_drvli(&drvli_text);
    let mut out = String::new();
    for interface in &drvli.interfaces {
        generate_client_trait(interface, &drvli, &mut out);
        generate_client_impl(interface, &drvli, &mut out);
        generate_server_trait(interface, &drvli, &mut out);
        generate_server_handler_impl(interface, &drvli, &mut out);
    }
    let out_path = format!("{}/drvli.rs", std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_path, out).unwrap();
    println!("cargo::rerun-if-changed=../interfaces.drvli");
}

fn generate_client_impl(interface: &Interface, drvli: &Drvli, out: &mut String) {
    let name_camel = camel_case(interface.name);
    writeln!(
        out,
        "impl {name_camel}Client for Capability<{name_camel}> {{"
    )
    .unwrap();
    for (method_id, method) in interface.methods.iter().enumerate() {
        let name = &method.name;
        write!(out, "    fn {name}(self").unwrap();
        for (arg_name, arg_type) in &method.args {
            let arg_type = rust_arg_type(arg_type, &drvli.structs);
            write!(out, ", {arg_name}: {arg_type}").unwrap();
        }
        write!(out, ")").unwrap();
        if let Some(return_type) = &method.return_type {
            let return_type = rust_normal_ret_type(return_type, &drvli.structs);
            write!(out, " -> {return_type}").unwrap();
        }
        writeln!(out, " {{").unwrap();
        writeln!(out, "        let data = serde_json::to_vec(&(").unwrap();
        for (arg_name, _) in &method.args {
            writeln!(out, "            {arg_name},",).unwrap();
        }
        writeln!(out, "        )).unwrap();").unwrap();
        writeln!(out, "        let mut buf = [0u8; 4096];").unwrap();
        writeln!( out, "        let result_len = unsafe {{ ipc_call(self.0, {method_id}, data.as_ptr(), data.len(), buf.as_mut_ptr(), buf.len()) }};").unwrap();
        writeln!(
            out,
            "        serde_json::from_slice(&buf[..result_len]).unwrap()"
        )
        .unwrap();
        writeln!(out, "    }}").unwrap();
    }
    for (stream_id, stream) in interface.streams.iter().enumerate() {
        let name = &stream.name;
        let type_ = camel_case(stream.type_);
        writeln!(
            out,
            "    fn {name}(self) -> &'static RingBuffer<{type_}> {{"
        )
        .unwrap();
        writeln!(
             out,
            "        let (ring_buffer, byte_count) = unsafe {{ ipc_map_ring_buffer(self.0, {stream_id}) }};"
        )
            .unwrap();
        writeln!(
             out,
            "        let ring_buffer = unsafe {{ &*core::ptr::from_raw_parts::<UntypedRingBuffer>(ring_buffer, byte_count) }};"
        )
            .unwrap();
        writeln!(out, "        unsafe {{ ring_buffer.cast::<{type_}>() }}").unwrap();
        writeln!(out, "    }}").unwrap();
    }
    writeln!(out, "}}").unwrap();
}

fn generate_client_trait(interface: &Interface, drvli: &Drvli, out: &mut String) {
    let name_camel = camel_case(interface.name);
    writeln!(out, "pub trait {name_camel}Client {{").unwrap();
    for method in &interface.methods {
        let method_name = &method.name;
        write!(out, "    fn {method_name}(self").unwrap();
        for (arg_name, arg_type) in &method.args {
            let arg_type = rust_arg_type(arg_type, &drvli.structs);
            write!(out, ", {arg_name}: {arg_type}").unwrap();
        }
        write!(out, ")").unwrap();
        if let Some(return_type) = &method.return_type {
            let return_type = rust_normal_ret_type(return_type, &drvli.structs);
            write!(out, " -> {return_type}").unwrap();
        }
        writeln!(out, ";").unwrap();
    }
    for stream in &interface.streams {
        let stream_name = &stream.name;
        let stream_type = rust_stream_type(stream.type_, &drvli.structs);
        writeln!(
            out,
            "    fn {stream_name}(self) -> &'static RingBuffer<{stream_type}>;"
        )
        .unwrap();
    }
    writeln!(out, "}}").unwrap();
}

fn generate_server_trait(interface: &Interface, drvli: &Drvli, out: &mut String) {
    let name_camel = camel_case(interface.name);
    writeln!(out, "pub trait {name_camel}Server<O = ()> {{").unwrap();
    for method in &interface.methods {
        let method_name = &method.name;
        write!(
            out,
            "    fn {method_name}(&mut self, ctx: &mut Ctx<Self>, object: O"
        )
        .unwrap();
        for (arg_name, arg_type) in &method.args {
            let arg_type = rust_arg_type(arg_type, &drvli.structs);
            write!(out, ", {arg_name}: {arg_type}").unwrap();
        }
        write!(out, ")").unwrap();
        if let Some(return_type) = &method.return_type {
            let return_type = rust_grantable_ret_type(return_type, &drvli.structs);
            write!(out, " -> {return_type}").unwrap();
        }
        writeln!(out, ";").unwrap();
    }
    for stream in &interface.streams {
        let stream_name = &stream.name;
        writeln!(
            out,
            "    fn {stream_name}(&mut self, object: O) -> (Capability<SharedMemory>, usize);"
        )
        .unwrap();
    }
    writeln!(out, "}}").unwrap();
}

fn generate_server_handler_impl(interface: &Interface, drvli: &Drvli, out: &mut String) {
    let name_snake = &interface.name;
    let name_camel = camel_case(name_snake);
    writeln!(
        out,
        "impl<S: {name_camel}Server<O>, O: Copy> Handler<{name_camel}, O> for S {{"
    )
    .unwrap();
    writeln!(
        out,
        "    fn call_method(&mut self, _ctx: &mut Ctx<Self>, method: usize, _args: &[u8], _object: O, _sender: ProcessId) -> Vec<u8> {{"
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
            let arg_type = rust_normal_ret_type(arg_type, &drvli.structs);
            write!(out, "{arg_type},").unwrap();
        }
        writeln!(out, ") = serde_json::from_slice(_args).unwrap();").unwrap();
        write!(
            out,
            "                let result = self.{method_name}(_ctx, _object, "
        )
        .unwrap();
        for (arg_name, arg_type) in &method.args {
            let borrow = rust_borrow_or_copy(arg_type);
            write!(out, "{borrow}{arg_name},").unwrap();
        }
        writeln!(out, ");").unwrap();
        writeln!(
            out,
            "                (serde_json::to_vec(&result).unwrap())"
        )
        .unwrap();
        writeln!(out, "            }}").unwrap();
    }
    for (stream_index, stream) in interface.streams.iter().enumerate() {
        let stream_name = &stream.name;
        let pseudo_method_index = stream_index + 1000;
        writeln!(out, "            {pseudo_method_index} => {{").unwrap();
        writeln!(
            out,
            "                let result = self.{stream_name}(_object);"
        )
        .unwrap();
        writeln!(out, "                serde_json::to_vec(&result).unwrap()").unwrap();
        writeln!(out, "            }}").unwrap();
    }
    writeln!(
        out,
        "            _ => unreachable!(\"unknown method {{method}}\"),"
    )
    .unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
}
