use deravel_codegen::{EntityDetails, camel_case, parse_interfaces};
use std::fmt::Write;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let interfaces_path = format!("{manifest_dir}/../interfaces.drvli");
    let interfaces = parse_interfaces(&std::fs::read_to_string(interfaces_path).unwrap());
    let mut output = String::new();
    for interface in &interfaces {
        let name_snake = &interface.name;
        let name_camel = camel_case(name_snake);
        writeln!(&mut output, "#[derive(Clone, Copy)]").unwrap();
        writeln!(&mut output, "pub struct {name_camel};").unwrap();
        if let EntityDetails::App { args, implements } = &interface.details {
            writeln!(&mut output, "#[repr(C)]").unwrap();
            writeln!(&mut output, "pub struct {name_camel}Args {{").unwrap();
            for (arg_name, arg_type) in args {
                let arg_type = camel_case(arg_type);
                writeln!(&mut output, "    pub {arg_name}: Capability<{arg_type}>,").unwrap();
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
            writeln!(
                &mut output,
                "    const NAME: &'static str = \"{name_snake}\";"
            )
            .unwrap();
            writeln!(&mut output, "}}").unwrap();
        }
    }
    std::fs::write(
        format!("{}/drvli.rs", std::env::var("OUT_DIR").unwrap()),
        output,
    )
    .unwrap();
    println!("cargo::rerun-if-changed=../interfaces.drvli");
}
