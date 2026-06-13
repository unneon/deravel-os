use deravel_codegen::{
    Interface, InterfaceDetails, camel_case, parse_drvli, rust_escape_name, rust_member_type,
};
use std::fmt::Write;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let drvli_text =
        std::fs::read_to_string(format!("{manifest_dir}/../interfaces.drvli")).unwrap();
    let drvli = parse_drvli(&drvli_text);
    let mut output = String::new();
    for struct_ in &drvli.structs {
        let name_camel = camel_case(struct_.name);
        writeln!(
            &mut output,
            "#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]"
        )
        .unwrap();
        writeln!(&mut output, "#[repr(C)]").unwrap();
        writeln!(&mut output, "pub struct {name_camel} {{").unwrap();
        for (member_name, member_type) in &struct_.members {
            let member_name = rust_escape_name(member_name);
            let member_type = rust_member_type(member_type, &drvli.structs);
            writeln!(&mut output, "    pub {member_name}: {member_type},").unwrap();
        }
        writeln!(&mut output, "}}").unwrap();
    }
    for interface in &drvli.interfaces {
        let name_snake = &interface.name;
        let name_camel = camel_case(name_snake);
        writeln!(&mut output, "#[derive(Clone, Copy)]").unwrap();
        writeln!(&mut output, "pub struct {name_camel};").unwrap();
        writeln!(&mut output, "impl Interface for {name_camel} {{").unwrap();
        writeln!(
            &mut output,
            "    const NAME: &'static str = \"{name_snake}\";"
        )
        .unwrap();
        writeln!(&mut output, "}}").unwrap();
        if let InterfaceDetails::App { args, implements } = &interface.details {
            writeln!(&mut output, "#[repr(C)]").unwrap();
            writeln!(&mut output, "#[derive(Debug, Deserialize)]").unwrap();
            writeln!(&mut output, "pub struct {name_camel}Args {{").unwrap();
            for (arg_name, arg_type) in args {
                let arg_type = rust_member_type(arg_type, &drvli.structs);
                writeln!(&mut output, "    pub {arg_name}: {arg_type},").unwrap();
            }
            writeln!(&mut output, "}}").unwrap();
            writeln!(&mut output, "impl ProcessArgs for {name_camel}Args {{").unwrap();
            writeln!(
                &mut output,
                "    fn for_all(&self, mut _f: impl FnMut(RawCapability)) {{"
            )
            .unwrap();
            for (arg_name, _) in args {
                writeln!(&mut output, "        _f(self.{arg_name}.0);").unwrap();
            }
            writeln!(&mut output, "    }}").unwrap();
            writeln!(&mut output, "}}").unwrap();
            writeln!(&mut output, "impl ProcessTag for {name_camel} {{").unwrap();
            writeln!(&mut output, "    type Args = {name_camel}Args;").unwrap();
            if let Some(implements) = implements {
                let implements_camel = camel_case(implements);
                writeln!(&mut output, "    type Export = {implements_camel};").unwrap();
            } else {
                writeln!(&mut output, "    type Export = {name_camel};").unwrap();
            }
            writeln!(&mut output, "    type Spawner = {name_camel}Spawner;").unwrap();
            writeln!(
                &mut output,
                "    const NAME: &'static str = \"{name_snake}\";"
            )
            .unwrap();
            writeln!(&mut output, "}}").unwrap();
            generate_spawner(interface, &mut output);
        }
    }
    std::fs::write(
        format!("{}/drvli.rs", std::env::var("OUT_DIR").unwrap()),
        output,
    )
    .unwrap();
    println!("cargo::rerun-if-changed=../interfaces.drvli");
}

fn generate_spawner(interface: &Interface, out: &mut String) {
    let camel_name = camel_case(interface.name);
    writeln!(out, "pub struct {camel_name}Spawner;").unwrap();
}
