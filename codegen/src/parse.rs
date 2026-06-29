use crate::{
    ContainsTypes, Drvli, Interface, InterfaceDetails, Method, Stream, Struct, Syscall, Type,
};
use std::iter::Peekable;
use std::str::Lines;

pub fn parse_drvli(text: &str) -> Drvli<'_> {
    let mut lines = text.lines().peekable();
    let mut structs = Vec::new();
    let mut interfaces = Vec::new();
    let mut syscalls = Vec::new();
    while let Some(line) = lines.next() {
        if let Some(name) = line.strip_prefix("struct ") {
            let mut members = Vec::new();
            while let Some(line) = lines.peek()
                && let Some(line) = line.strip_prefix("    ")
            {
                let (member_name, member_type) = line.split_once(' ').unwrap();
                members.push((member_name, parse_type(member_type)));
                lines.next();
            }
            structs.push(Struct { name, members });
        } else if let Some(line) = line.strip_prefix("app ") {
            let name_len = line.find(['(', ' ']).unwrap_or(line.len());
            let name = &line[..name_len];
            let line = &line[name_len..];
            let (args, line) = if let Some(line) = line.strip_prefix("(") {
                let (args, line) = line.split_once(')').unwrap();
                let args = args
                    .split(", ")
                    .filter(|arg| !arg.is_empty())
                    .map(|arg| {
                        let (arg_name, arg_type) = arg.split_once(' ').unwrap();
                        (arg_name, parse_type(arg_type))
                    })
                    .collect();
                (args, line)
            } else {
                (Vec::new(), line)
            };
            let line = line.trim();
            let implements = line.strip_prefix("implements ");
            let interface =
                parse_interface(name, &mut lines, InterfaceDetails::App { args, implements });
            interfaces.push(interface);
        } else if let Some(name) = line.strip_prefix("interface ") {
            let interface = parse_interface(name, &mut lines, InterfaceDetails::Interface);
            interfaces.push(interface);
        } else if let Some(line) = line.strip_prefix("syscall ") {
            syscalls.push(parse_syscall(line));
        }
    }
    let mut drvli = Drvli {
        interfaces,
        structs,
        syscalls,
    };
    let interfaces = drvli.interfaces.iter().map(|i| i.name).collect();
    let structs = drvli.structs.iter().map(|s| s.name).collect();
    drvli.fix_types(&interfaces, &structs);
    drvli
}

pub fn parse_interface<'a>(
    name: &'a str,
    lines: &mut Peekable<Lines<'a>>,
    details: InterfaceDetails<'a>,
) -> Interface<'a> {
    let mut methods = Vec::new();
    let mut streams = Vec::new();
    while let Some(line) = lines.next()
        && let Some(line) = line.strip_prefix("    ")
    {
        if let Some(line) = line.strip_prefix("func ") {
            let (name, line) = line.split_once('(').unwrap();
            let (method_args, line) = line.split_once(")").unwrap();
            let args: Vec<_> = method_args
                .split(", ")
                .filter(|s| !s.is_empty())
                .map(|arg| {
                    let (arg_name, arg_type) = arg.split_once(' ').unwrap();
                    (arg_name, parse_type(arg_type))
                })
                .collect();
            let return_type = line.strip_prefix(" ").map(parse_type);
            methods.push(Method {
                name,
                args,
                return_type,
            });
        } else if let Some(line) = line.strip_prefix("stream ") {
            let (name, type_) = line.split_once(' ').unwrap();
            let type_ = parse_type(type_);
            streams.push(Stream { name, type_ });
        }
    }
    Interface {
        name,
        methods,
        streams,
        details,
    }
}

fn parse_syscall(line: &str) -> Syscall<'_> {
    let (name, line) = line.split_once('(').unwrap();
    let (args, return_type) = line.split_once(')').unwrap();
    let args = args
        .split(", ")
        .filter(|arg| !arg.is_empty())
        .map(|arg| {
            let (arg_name, arg_type) = arg.split_once(' ').unwrap();
            (arg_name, parse_type(arg_type))
        })
        .collect();
    let return_type = return_type.strip_prefix(' ').map(parse_type);
    Syscall {
        name,
        args,
        return_type,
    }
}

pub fn parse_type(src: &str) -> Type<'_> {
    use crate::Type::*;
    if src.contains(",") {
        return Tuple(src.split(", ").map(parse_type).collect());
    }
    if let Some(inner) = src.strip_prefix("array ") {
        return Array(Box::new(parse_type(inner)));
    } else if let Some(inner) = src.strip_prefix("const_array ") {
        return ConstArray(Box::new(parse_type(inner)));
    } else if let Some(inner) = src.strip_prefix("const_ptr ") {
        return ConstPtr(Box::new(parse_type(inner)));
    } else if let Some(inner) = src.strip_prefix("option ") {
        return Option(Box::new(parse_type(inner)));
    } else if let Some(interface) = src.strip_prefix("process_spawner ") {
        return ProcessSpawner(interface);
    } else if let Some(inner) = src.strip_prefix("ptr ") {
        return Ptr(Box::new(parse_type(inner)));
    }
    match src {
        "bytes" => Bytes,
        "capability" => UntypedCapability,
        "i8" => I8,
        "i16" => I16,
        "i32" => I32,
        "i64" => I64,
        "isize" => Isize,
        "never" => Never,
        "pid" => ProcessId,
        "ptr" => UntypedPointer,
        "shared_memory" => SharedMemory,
        "text" => Text,
        "u8" => U8,
        "u16" => U16,
        "u32" => U32,
        "u64" => U64,
        "usize" => Usize,
        "input_event" | "windowing" | "window" | "input_device" | "display" | "drive"
        | "console" | "shutdown" | "filesystem" | "network" => Unknown(src),
        _ => unimplemented!("unimplemented type {src:?}"),
    }
}
