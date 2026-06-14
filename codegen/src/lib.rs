use std::borrow::Cow;
use std::iter::{Peekable, once};
use std::str::Lines;

pub struct Drvli<'a> {
    pub interfaces: Vec<Interface<'a>>,
    pub structs: Vec<Struct<'a>>,
    pub syscalls: Vec<Syscall<'a>>,
}

pub struct Struct<'a> {
    pub name: &'a str,
    pub members: Vec<(&'a str, &'a str)>,
}

pub struct Interface<'a> {
    pub name: &'a str,
    pub methods: Vec<Method<'a>>,
    pub streams: Vec<Stream<'a>>,
    pub details: InterfaceDetails<'a>,
}

pub struct Method<'a> {
    pub name: &'a str,
    pub args: Vec<(&'a str, &'a str)>,
    pub return_type: Option<&'a str>,
}

pub struct Stream<'a> {
    pub name: &'a str,
    pub type_: &'a str,
}

pub enum InterfaceDetails<'a> {
    App {
        args: Vec<(&'a str, &'a str)>,
        implements: Option<&'a str>,
    },
    Interface,
}

pub struct Syscall<'a> {
    pub name: &'a str,
    pub args: Vec<(&'a str, &'a str)>,
    pub return_type: Option<&'a str>,
}

pub fn rust_arg_type(type_: &str, structs: &[Struct]) -> Cow<'static, str> {
    match type_ {
        "i8" => "i8".into(),
        "i16" => "i16".into(),
        "i32" => "i32".into(),
        "i64" => "i64".into(),
        "u8" => "u8".into(),
        "u16" => "u16".into(),
        "u32" => "u32".into(),
        "u64" => "u64".into(),
        "usize" => "usize".into(),
        "text" => "&str".into(),
        "bytes" => "&[u8]".into(),
        _ if let Some(inner) = type_.strip_prefix("process_spawner ") => {
            format!("Capability<{}Spawner>", camel_case(inner)).into()
        }
        _ if structs.iter().any(|struct_| struct_.name == type_) => camel_case(type_).into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
    }
}

pub fn rust_member_type(type_: &str, structs: &[Struct]) -> Cow<'static, str> {
    match type_ {
        "i8" => "i8".into(),
        "i16" => "i16".into(),
        "i32" => "i32".into(),
        "i64" => "i64".into(),
        "u8" => "u8".into(),
        "u16" => "u16".into(),
        "u32" => "u32".into(),
        "u64" => "u64".into(),
        "usize" => "usize".into(),
        _ if let Some(inner) = type_.strip_prefix("process_spawner ") => {
            format!("Capability<{}Spawner>", camel_case(inner)).into()
        }
        _ if structs.iter().any(|struct_| struct_.name == type_) => camel_case(type_).into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
    }
}

pub fn rust_normal_ret_type(type_: &str, structs: &[Struct<'_>]) -> Cow<'static, str> {
    match type_ {
        "i8" => "i8".into(),
        "i16" => "i16".into(),
        "i32" => "i32".into(),
        "i64" => "i64".into(),
        "u8" => "u8".into(),
        "u16" => "u16".into(),
        "u32" => "u32".into(),
        "u64" => "u64".into(),
        "usize" => "usize".into(),
        "text" => "String".into(),
        "bytes" => "Vec<u8>".into(),
        "never" => "!".into(),
        _ if structs.iter().any(|struct_| struct_.name == type_) => camel_case(type_).into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
    }
}

pub fn rust_grantable_ret_type(type_: &str, structs: &[Struct<'_>]) -> Cow<'static, str> {
    match type_ {
        "i8" => "i8".into(),
        "i16" => "i16".into(),
        "i32" => "i32".into(),
        "i64" => "i64".into(),
        "u8" => "u8".into(),
        "u16" => "u16".into(),
        "u32" => "u32".into(),
        "u64" => "u64".into(),
        "usize" => "usize".into(),
        "text" => "String".into(),
        "bytes" => "Vec<u8>".into(),
        "never" => "!".into(),
        _ if structs.iter().any(|struct_| struct_.name == type_) => camel_case(type_).into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
    }
}

pub fn rust_syscall_arg_type(type_: &str, structs: &[Struct<'_>]) -> Cow<'static, str> {
    match type_ {
        "i8" => "i8".into(),
        "i16" => "i16".into(),
        "i32" => "i32".into(),
        "i64" => "i64".into(),
        "capability" => "RawCapability".into(),
        "u8" => "u8".into(),
        "u16" => "u16".into(),
        "u32" => "u32".into(),
        "u64" => "u64".into(),
        "usize" => "usize".into(),
        "const_ptr u8" => "*const u8".into(),
        "ptr u8" => "*mut u8".into(),
        // "text" => "String".into(),
        // "bytes" => "Vec<u8>".into(),
        _ if structs.iter().any(|struct_| struct_.name == type_) => camel_case(type_).into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
    }
}

pub fn rust_syscall_kernel_arg_type(type_: &str, structs: &[Struct<'_>]) -> Cow<'static, str> {
    match type_ {
        "i8" => "i8".into(),
        "i16" => "i16".into(),
        "i32" => "i32".into(),
        "i64" => "i64".into(),
        "capability" => "RawCapability".into(),
        "u8" => "u8".into(),
        "u16" => "u16".into(),
        "u32" => "u32".into(),
        "u64" => "u64".into(),
        "usize" => "usize".into(),
        "const_array u8" => "&mut [u8]".into(),
        "array u8" => "&mut [u8]".into(),
        // "ptr u8" => "*mut u8".into(),
        // "text" => "String".into(),
        // "bytes" => "Vec<u8>".into(),
        _ if structs.iter().any(|struct_| struct_.name == type_) => camel_case(type_).into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
    }
}

pub fn rust_syscall_ret_type(type_: &str, structs: &[Struct<'_>]) -> Cow<'static, str> {
    match type_ {
        "i8" => "i8".into(),
        "i16" => "i16".into(),
        "i32" => "i32".into(),
        "i64" => "i64".into(),
        "never" => "!".into(),
        "const_ptr u8" => "*const u8".into(),
        "ptr" => "*mut ()".into(),
        "ptr u8" => "*mut u8".into(),
        "ptr u8, u64" => "(*mut u8, u64)".into(),
        "ptr u8, shared_memory" => "(*mut u8, Capability<SharedMemory>)".into(),
        "ptr, u64" => "(*mut (), u64)".into(),
        "capability" => "RawCapability".into(),
        "capability, u64, u64, pid" => "(RawCapability, u64, u64, ProcessId)".into(),
        "u8" => "u8".into(),
        "u16" => "u16".into(),
        "u32" => "u32".into(),
        "u64" => "u64".into(),
        "usize" => "usize".into(),
        "pid" => "ProcessId".into(),
        // "text" => "String".into(),
        // "bytes" => "Vec<u8>".into(),
        _ if structs.iter().any(|struct_| struct_.name == type_) => camel_case(type_).into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
    }
}

pub fn is_capability(type_: &str, structs: &[Struct<'_>]) -> bool {
    match type_ {
        "i8" => false,
        "i16" => false,
        "i32" => false,
        "i64" => false,
        "u8" => false,
        "u16" => false,
        "u32" => false,
        "u64" => false,
        "usize" => false,
        "text" => false,
        "bytes" => false,
        _ if structs.iter().any(|struct_| struct_.name == type_) => false,
        _ => true,
    }
}

pub fn rust_stream_type(type_: &str, structs: &[Struct]) -> Cow<'static, str> {
    match type_ {
        "i8" => "i8".into(),
        "i16" => "i16".into(),
        "i32" => "i32".into(),
        "i64" => "i64".into(),
        "u8" => "u8".into(),
        "u16" => "u16".into(),
        "u32" => "u32".into(),
        "u64" => "u64".into(),
        "usize" => "usize".into(),
        "text" => unimplemented!(),
        "bytes" => unimplemented!(),
        _ if structs.iter().any(|struct_| struct_.name == type_) => camel_case(type_).into(),
        _ => format!("Capability<{}>", camel_case(type_)).into(),
    }
}

pub fn rust_borrow_or_copy(type_: &str) -> &'static str {
    match type_ {
        "text" => "&",
        "bytes" => "&",
        _ => "",
    }
}

pub fn rust_escape_name(name: &str) -> &str {
    match name {
        "type" => "type_",
        "yield" => "yield_",
        _ => name,
    }
}

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
                members.push(line.split_once(' ').unwrap());
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
                    .map(|arg| arg.split_once(' ').unwrap())
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
    Drvli {
        interfaces,
        structs,
        syscalls,
    }
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
                .map(|arg| arg.split_once(' ').unwrap())
                .collect();
            let return_type = line.strip_prefix(" ");
            methods.push(Method {
                name,
                args,
                return_type,
            });
        } else if let Some(line) = line.strip_prefix("stream ") {
            let (name, type_) = line.split_once(' ').unwrap();
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
        .map(|arg| arg.split_once(' ').unwrap())
        .collect();
    let return_type = return_type.strip_prefix(' ');
    Syscall {
        name,
        args,
        return_type,
    }
}

pub fn camel_case(name: &str) -> String {
    let mut camel = String::new();
    for segment in name.split('_') {
        camel.push(segment.as_bytes()[0].to_ascii_uppercase() as char);
        camel += &segment[1..];
    }
    camel
}

pub fn split_syscall_arg(type_: &str) -> impl Iterator<Item = (&'static str, &str)> {
    match type_ {
        "array u8" => Box::new([("_ptr", "ptr u8"), ("_size", "usize")].into_iter())
            as Box<dyn Iterator<Item = (&'static str, &str)>>,
        "const_array u8" => Box::new([("_ptr", "const_ptr u8"), ("_size", "usize")].into_iter()),
        _ => Box::new(once(("", type_))),
    }
}

pub fn split_syscall_ret(type_: &str) -> impl Iterator<Item = &str> {
    match type_ {
        "array u8" => Box::new(["ptr u8", "usize"].into_iter()) as Box<dyn Iterator<Item = &str>>,
        "const_array u8" => Box::new(["const_ptr u8", "usize"].into_iter()),
        "ptr u8, usize" => Box::new(["ptr u8", "usize"].into_iter()),
        "ptr u8, shared_memory" => Box::new(["ptr u8", "shared_memory"].into_iter()),
        "ptr, usize" => Box::new(["ptr", "usize"].into_iter()),
        "capability, usize, usize, pid" => {
            Box::new(["capability", "usize", "usize", "pid"].into_iter())
        }
        _ => Box::new(once(type_)),
    }
}
