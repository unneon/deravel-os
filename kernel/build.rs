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
        writeln!(out, ") = postcard::from_bytes(_args).unwrap();").unwrap();
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
            writeln!(out, "                let mut buf = vec![0; 4096];").unwrap();
            writeln!(out, "                let buf_len = postcard::to_slice(&_result, &mut buf).unwrap().len();").unwrap();
            writeln!(out, "                buf.resize(buf_len, 0);").unwrap();
            writeln!(out, "                buf").unwrap();
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
    writeln!(
        out,
        "    fn shared_memory_map(&self, _: usize, _: &mut TopPageTable, _: &mut Vec<(Range<usize>, &'static (dyn Handler<SharedMemory> + Sync))>) {{"
    )
    .unwrap();
    writeln!(out, "        unreachable!()").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "    fn shared_memory_size(&self) -> usize {{").unwrap();
    writeln!(out, "        unreachable!()").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(
        out,
        "    fn virtual_memory_load(&self, _: usize, _: usize, _: &mut TopPageTable) {{"
    )
    .unwrap();
    writeln!(out, "        unreachable!()").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
}

fn generate_syscall_trait(drvli: &Drvli, out: &mut String) {
    writeln!(out, "pub trait SyscallHandler {{").unwrap();
    for syscall in &drvli.syscalls {
        let syscall_name = rust_escape_name(syscall.name);
        write!(out, "    fn {syscall_name}(user: &mut UserCtx").unwrap();
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
    writeln!(out, "pub fn dispatch_syscall(registers: &mut RiscvRegisters, user: &mut UserCtx) -> Result<!, UserSyscallError> {{").unwrap();
    writeln!(out, "    #![allow(clippy::diverging_sub_expression)]").unwrap();
    writeln!(out, "    match registers.a6 {{").unwrap();
    for (syscall_number, syscall) in drvli.syscalls.iter().enumerate() {
        let syscall_name = rust_escape_name(syscall.name);
        writeln!(out, "        {syscall_number} => {{").unwrap();
        write!(
            out,
            "            let _result = <() as SyscallHandler>::{syscall_name}(user"
        )
        .unwrap();
        let mut used_arg_registers = 0;
        for (arg_name, arg_type) in &syscall.args {
            let value = match arg_type {
                Type::UntypedCapability => format!(
                    "RawCapability::try_from(registers.a{used_arg_registers} as *const CapabilityCertificate)?"
                ),
                Type::SharedMemory => format!(
                    "unsafe {{ Capability::new(RawCapability::try_from(registers.a{used_arg_registers} as *const CapabilityCertificate)?) }}"
                ),
                Type::U64 => format!("registers.a{used_arg_registers} as u64"),
                Type::Usize => format!("registers.a{used_arg_registers}"),
                Type::Array(inner) | Type::ConstArray(inner) if **inner == Type::U8 => {
                    let ap = format!("registers.a{used_arg_registers}");
                    let as_ = format!("registers.a{}", used_arg_registers + 1);
                    format!("UserPtr::from_slice({ap} as *mut u8, {as_})?")
                }
                Type::Ptr(inner) if **inner == Type::U8 => {
                    format!("UserPtr::from_ptr(registers.a{used_arg_registers} as *mut u8)?")
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
        "        _ => return Err(UserSyscallError::InvalidSyscallNumber),"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "    crate::arch::return_to_user(registers);").unwrap();
    writeln!(out, "}}").unwrap();
}
