pub mod parse;

use std::borrow::Cow;
use std::collections::HashSet;
use std::iter::once;

trait ContainsTypes<'a> {
    fn fix_types(&mut self, interfaces: &HashSet<&'a str>, structs: &HashSet<&'a str>);
}

pub struct Drvli<'a> {
    pub interfaces: Vec<Interface<'a>>,
    pub structs: Vec<Struct<'a>>,
    pub syscalls: Vec<Syscall<'a>>,
}

pub struct Struct<'a> {
    pub name: &'a str,
    pub members: Vec<(&'a str, Type<'a>)>,
}

pub struct Interface<'a> {
    pub name: &'a str,
    pub methods: Vec<Method<'a>>,
    pub streams: Vec<Stream<'a>>,
    pub details: InterfaceDetails<'a>,
}

pub struct Method<'a> {
    pub name: &'a str,
    pub args: Vec<(&'a str, Type<'a>)>,
    pub return_type: Option<Type<'a>>,
}

pub struct Stream<'a> {
    pub name: &'a str,
    pub type_: Type<'a>,
}

pub enum InterfaceDetails<'a> {
    App {
        args: Vec<(&'a str, Type<'a>)>,
        implements: Option<&'a str>,
    },
    Interface,
}

pub struct Syscall<'a> {
    pub name: &'a str,
    pub args: Vec<(&'a str, Type<'a>)>,
    pub return_type: Option<Type<'a>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type<'a> {
    Array(Box<Type<'a>>),
    Bytes,
    ConstArray(Box<Type<'a>>),
    ConstPtr(Box<Type<'a>>),
    I8,
    I16,
    I32,
    I64,
    Isize,
    Never,
    Option(Box<Type<'a>>),
    ProcessId,
    ProcessSpawner(&'a str),
    Ptr(Box<Type<'a>>),
    SharedMemory,
    Struct(&'a str),
    Text,
    Tuple(Vec<Type<'a>>),
    TypedCapability(&'a str),
    U8,
    U16,
    U32,
    U64,
    Unknown(&'a str),
    UntypedCapability,
    UntypedPointer,
    Usize,
}

#[derive(Clone, Copy, Debug)]
pub enum RustTypeCtx {
    Arg,
    Member,
    NormalRet,
    GrantableRet,
    SyscallArg,
    SyscallKernelArg,
    SyscallRet,
    Stream,
}

impl Type<'_> {
    pub fn rust(&self, ctx: RustTypeCtx) -> Cow<'static, str> {
        use RustTypeCtx::*;
        use Type::*;
        match (self, ctx) {
            (Array(inner) | ConstArray(inner), SyscallKernelArg) => {
                format!("&mut [{}]", inner.rust(ctx)).into()
            }
            (Bytes, Arg) => "&[u8]".into(),
            (Bytes, NormalRet | GrantableRet) => "Vec<u8>".into(),
            (ConstPtr(inner), _) => format!("*const {}", inner.rust(ctx)).into(),
            (I8, _) => "i8".into(),
            (I16, _) => "i16".into(),
            (I32, _) => "i32".into(),
            (I64, _) => "i64".into(),
            (Isize, _) => "isize".into(),
            (Never, _) => "!".into(),
            (Option(inner), _) => format!("Option<{}>", inner.rust(ctx)).into(),
            (Ptr(inner), _) => format!("*mut {}", inner.rust(ctx)).into(),
            (ProcessId, SyscallRet) => "ProcessId".into(),
            (ProcessSpawner(name), _) => format!("Capability<{}Spawner>", camel_case(name)).into(),
            (SharedMemory, _) => "Capability<SharedMemory>".into(),
            (Struct(name), _) => camel_case(name).into(),
            (TypedCapability(name), _) => format!("Capability<{}>", camel_case(name)).into(),
            (Text, Arg) => "&str".into(),
            (Text, NormalRet | GrantableRet) => "String".into(),
            (U8, _) => "u8".into(),
            (U16, _) => "u16".into(),
            (U32, _) => "u32".into(),
            (U64, _) => "u64".into(),
            (UntypedCapability, _) => "RawCapability".into(),
            (UntypedPointer, _) => "*mut ()".into(),
            (Usize, _) => "usize".into(),
            _ => unimplemented!("rust({self:?}, {ctx:?})"),
        }
    }

    pub fn rust_borrow_or_copy(&self) -> &'static str {
        match self {
            Type::Text | Type::Bytes => "&",
            _ => "",
        }
    }
}

impl<'a> ContainsTypes<'a> for Drvli<'a> {
    fn fix_types(&mut self, interfaces: &HashSet<&'a str>, structs: &HashSet<&'a str>) {
        self.interfaces.fix_types(interfaces, structs);
        self.structs.fix_types(interfaces, structs);
        self.syscalls.fix_types(interfaces, structs);
    }
}

impl<'a> ContainsTypes<'a> for Interface<'a> {
    fn fix_types(&mut self, interfaces: &HashSet<&'a str>, structs: &HashSet<&'a str>) {
        self.methods.fix_types(interfaces, structs);
        self.streams.fix_types(interfaces, structs);
        self.details.fix_types(interfaces, structs);
    }
}

impl<'a> ContainsTypes<'a> for Method<'a> {
    fn fix_types(&mut self, interfaces: &HashSet<&'a str>, structs: &HashSet<&'a str>) {
        for (_, type_) in &mut self.args {
            type_.fix_types(interfaces, structs);
        }
        if let Some(type_) = &mut self.return_type {
            type_.fix_types(interfaces, structs);
        }
    }
}

impl<'a> ContainsTypes<'a> for Stream<'a> {
    fn fix_types(&mut self, interfaces: &HashSet<&'a str>, structs: &HashSet<&'a str>) {
        self.type_.fix_types(interfaces, structs);
    }
}

impl<'a> ContainsTypes<'a> for InterfaceDetails<'a> {
    fn fix_types(&mut self, interfaces: &HashSet<&'a str>, structs: &HashSet<&'a str>) {
        match self {
            InterfaceDetails::App { args, .. } => {
                for (_, type_) in args {
                    type_.fix_types(interfaces, structs);
                }
            }
            InterfaceDetails::Interface => {}
        }
    }
}

impl<'a> ContainsTypes<'a> for Struct<'a> {
    fn fix_types(&mut self, interfaces: &HashSet<&'a str>, structs: &HashSet<&'a str>) {
        for (_, type_) in &mut self.members {
            type_.fix_types(interfaces, structs);
        }
    }
}

impl<'a> ContainsTypes<'a> for Syscall<'a> {
    fn fix_types(&mut self, interfaces: &HashSet<&'a str>, structs: &HashSet<&'a str>) {
        for (_, type_) in &mut self.args {
            type_.fix_types(interfaces, structs);
        }
        if let Some(type_) = &mut self.return_type {
            type_.fix_types(interfaces, structs);
        }
    }
}

impl<'a> ContainsTypes<'a> for Type<'a> {
    fn fix_types(&mut self, interfaces: &HashSet<&'a str>, structs: &HashSet<&'a str>) {
        use Type::*;
        match self {
            Bytes | I8 | I16 | I32 | I64 | Isize | Never | ProcessId | ProcessSpawner(_)
            | SharedMemory | Struct(_) | Text | TypedCapability(_) | U8 | U16 | U32 | U64
            | UntypedCapability | UntypedPointer | Usize => {}
            Array(t) | ConstArray(t) | ConstPtr(t) | Option(t) | Ptr(t) => {
                t.fix_types(interfaces, structs)
            }
            Tuple(ts) => {
                ts.fix_types(interfaces, structs);
            }
            Unknown(name) => {
                if structs.contains(name) {
                    *self = Struct(name);
                } else if interfaces.contains(name) {
                    *self = TypedCapability(name);
                } else {
                    panic!("unknown type {name:?}");
                }
            }
        }
    }
}

impl<'a, T: ContainsTypes<'a>> ContainsTypes<'a> for Vec<T> {
    fn fix_types(&mut self, interfaces: &HashSet<&'a str>, structs: &HashSet<&'a str>) {
        for x in self {
            x.fix_types(interfaces, structs);
        }
    }
}

pub fn rust_escape_name(name: &str) -> &str {
    match name {
        "type" => "type_",
        "yield" => "yield_",
        _ => name,
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

pub fn split_syscall_arg<'a>(type_: &Type<'a>) -> impl Iterator<Item = (&'static str, Type<'a>)> {
    match type_ {
        Type::Array(inner) => {
            Box::new([("_ptr", Type::Ptr(inner.clone())), ("_size", Type::Usize)].into_iter())
                as Box<dyn Iterator<Item = (&'static str, Type<'a>)>>
        }
        Type::ConstArray(inner) => Box::new(
            [
                ("_ptr", Type::ConstPtr(inner.clone())),
                ("_size", Type::Usize),
            ]
            .into_iter(),
        ),
        _ => Box::new(once(("", type_.clone()))),
    }
}

pub fn split_syscall_ret<'a, 'b>(type_: &'b Type<'a>) -> impl Iterator<Item = &'b Type<'a>> {
    match type_ {
        Type::Tuple(inner) => inner.as_slice().iter(),
        _ => core::slice::from_ref(type_).iter(),
    }
}
