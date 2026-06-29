extern crate core;

use deravel_codegen::RustTypeCtx::*;
use deravel_codegen::parse::parse_drvli;
use deravel_codegen::{
    Drvli, Interface, Type, camel_case, rust_escape_name, split_syscall_arg, split_syscall_ret,
};
use std::fmt::Write;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let drvli_path = format!("{manifest_dir}/../interfaces.drvli");
    let drvli_text = std::fs::read_to_string(drvli_path).unwrap();
    let drvli = parse_drvli(&drvli_text);
    let mut output = String::new();
    for interface in &drvli.interfaces {
        generate_server_trait(interface, &mut output);
        generate_handler_impl(interface, &mut output);
    }
    generate_syscall_trait(&drvli, &mut output);
    generate_syscall_dispatch(&drvli, &mut output);
    std::fs::write(
        format!("{}/drvli.rs", std::env::var("OUT_DIR").unwrap()),
        output,
    )
    .unwrap();
    println!("cargo::rerun-if-changed=../interfaces.drvli");
    println!("cargo::rerun-if-changed=kernel.ld");
    println!("cargo::rustc-link-arg=-Tkernel/kernel.ld");
}

fn generate_server_trait(interface: &Interface, out: &mut String) {
    let name_snake = &interface.name;
    let name_camel = camel_case(name_snake);
    writeln!(out, "#[allow(dead_code)]").unwrap();
    writeln!(out, "pub trait {name_camel}Server {{").unwrap();
    for method in &interface.methods {
        let method_name = &method.name;
        write!(out, "    fn {method_name}(&self, sender: ProcessId").unwrap();
        for (arg_name, arg_type) in &method.args {
            let arg_type = arg_type.rust(Arg);
            write!(out, ", {arg_name}: {arg_type}").unwrap();
        }
        write!(out, ")").unwrap();
        if let Some(return_type) = &method.return_type {
            let return_type = return_type.rust(NormalRet);
            write!(out, " -> {return_type}").unwrap();
        }
        writeln!(out, ";").unwrap();
    }
    for stream in &interface.streams {
        let stream_name = &stream.name;
        let type_ = stream.type_.rust(Member);
        writeln!(
            out,
            "    fn {stream_name}(&self) -> &'static RingBuffer<{type_}>;"
        )
        .unwrap();
    }
    writeln!(out, "}}").unwrap();
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
            let arg_type = arg_type.rust(NormalRet);
            write!(out, "{arg_type},").unwrap();
        }
        writeln!(out, ") = serde_json::from_slice(_args).unwrap();").unwrap();
        write!(
            out,
            "                let _result = self.{method_name}(_sender, "
        )
        .unwrap();
        for (arg_name, arg_type) in &method.args {
            let borrow = arg_type.rust_borrow_or_copy();
            write!(out, "{borrow}{arg_name},").unwrap();
        }
        writeln!(out, ");").unwrap();
        if method.return_type != Some(Type::Never) {
            writeln!(out, "                serde_json::to_vec(&_result).unwrap()").unwrap();
        }
        writeln!(out, "            }}").unwrap();
    }
    writeln!(out, "            _ => unreachable!(),").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(
        out,
        "    fn map_stream(&self, stream: usize) -> &'static UntypedRingBuffer {{"
    )
    .unwrap();
    writeln!(out, "        match stream {{").unwrap();
    for (stream_index, stream) in interface.streams.iter().enumerate() {
        let stream_name = &stream.name;
        writeln!(
            out,
            "            {stream_index} => self.{stream_name}().untype(),"
        )
        .unwrap();
    }
    writeln!(out, "            _ => unreachable!(),").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "    fn shared_memory(&self) -> (usize, usize) {{").unwrap();
    writeln!(out, "        unreachable!()").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
}

fn generate_syscall_trait(drvli: &Drvli, out: &mut String) {
    writeln!(out, "pub trait SyscallHandler {{").unwrap();
    for syscall in &drvli.syscalls {
        let syscall_name = rust_escape_name(syscall.name);
        write!(out, "    fn {syscall_name}(user_pc: usize, registers: &mut RiscvRegisters, hart: &mut HartContext").unwrap();
        for (arg_name, arg_type) in &syscall.args {
            let arg_type = arg_type.rust(SyscallKernelArg);
            write!(out, ", {arg_name}: {arg_type}").unwrap();
        }
        write!(out, ")").unwrap();
        if let Some(return_type) = &syscall.return_type {
            if syscall
                .return_type
                .iter()
                .flat_map(split_syscall_ret)
                .count()
                == 1
            {
                let return_type = return_type.rust(SyscallRet);
                write!(out, " -> {return_type}").unwrap();
            } else {
                write!(out, " -> (").unwrap();
                for ret_type in split_syscall_ret(return_type) {
                    let ret_type = ret_type.rust(SyscallRet);
                    write!(out, "{ret_type}, ").unwrap();
                }
                writeln!(out, "            )").unwrap();
            }
        }
        writeln!(out, ";").unwrap();
    }
    writeln!(out, "}}").unwrap();
}

fn generate_syscall_dispatch(drvli: &Drvli, out: &mut String) {
    writeln!(out, "pub fn dispatch_syscall(user_pc: usize, registers: &mut RiscvRegisters, hart: &mut HartContext) -> ! {{").unwrap();
    writeln!(out, "    #![allow(clippy::diverging_sub_expression)]").unwrap();
    writeln!(out, "    match registers.a6 {{").unwrap();
    for (syscall_number, syscall) in drvli.syscalls.iter().enumerate() {
        let syscall_name = rust_escape_name(syscall.name);
        writeln!(out, "        {syscall_number} => {{").unwrap();
        write!(
            out,
            "            let _result = <() as SyscallHandler>::{syscall_name}(user_pc, registers, hart").unwrap();
        let mut used_arg_registers = 0;
        for (arg_name, arg_type) in &syscall.args {
            let value = match arg_type {
                Type::UntypedCapability => format!(
                    "RawCapability::from_ptr(registers.a{used_arg_registers} as *mut CapabilityCertificate)"
                ),
                Type::SharedMemory => format!(
                    "unsafe {{ Capability::new(RawCapability::from_ptr(registers.a{used_arg_registers} as *mut CapabilityCertificate)) }}"
                ),
                Type::U64 => format!("registers.a{used_arg_registers} as u64"),
                Type::Usize => format!("registers.a{used_arg_registers}"),
                Type::Array(inner) | Type::ConstArray(inner) if **inner == Type::U8 => {
                    let ap = format!("registers.a{used_arg_registers}");
                    let as_ = format!("registers.a{}", used_arg_registers + 1);
                    format!("unsafe {{ core::slice::from_raw_parts_mut({ap} as *mut u8, {as_}) }}")
                }
                _ => unimplemented!("syscall argument {arg_name:?} {arg_type:?}"),
            };
            used_arg_registers += split_syscall_arg(arg_type).count();
            write!(out, ", {value}").unwrap();
        }
        writeln!(out, ");").unwrap();
        if let Some(return_type) = &syscall.return_type
            && return_type != &Type::Never
        {
            if syscall
                .return_type
                .iter()
                .flat_map(split_syscall_ret)
                .count()
                == 1
            {
                writeln!(
                    out,
                    "            registers.a0 = unsafe {{ to_reg(_result) }};"
                )
                .unwrap();
            } else {
                for (ret_register, _ret_type) in split_syscall_ret(return_type).enumerate() {
                    writeln!(
                        out,
                        "            registers.a{ret_register} = unsafe {{ to_reg(_result.{ret_register}) }};"
                    )
                    .unwrap();
                }
            }
        }
        writeln!(out, "        }}").unwrap();
    }
    writeln!(
        out,
        "        _ => core::panic!(\"invalid syscall number {{}}\", registers.a6),"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(
        out,
        "    unsafe {{ riscv::register::sepc::write(user_pc + 4) }}"
    )
    .unwrap();
    writeln!(
        out,
        "    crate::arch::switch_to_userspace_registers_only(registers);"
    )
    .unwrap();
    writeln!(out, "}}").unwrap();
}
