use fontdue::{Font, FontSettings};
use std::fmt::Write;

fn main() {
    println!("cargo::rerun-if-changed=../kernel-api/user.ld");
    println!("cargo::rustc-link-arg=-Tkernel-api/user.ld");

    let font = include_bytes!("/usr/share/fonts/TTF/FiraCode-Regular.ttf");
    let font = Font::from_bytes(font.as_slice(), FontSettings::default()).unwrap();
    let mut out = String::new();
    writeln!(
        out,
        r#"struct Font {{
    width: usize,
    height: usize,
    characters: [Character; 94],
}}

struct Character {{
    ascii: u8,
    xmin: i32,
    ymin: i32,
    width: usize,
    height: usize,
    bitmap: &'static [u8],
}}

static FONT: Font = Font {{
    width: 11,
    height: 17,
    characters: ["#
    )
    .unwrap();
    for c in u8::MIN..=u8::MAX {
        if c.is_ascii_graphic() {
            let (metrics, bitmap) = font.rasterize(c as char, 17.0);
            writeln!(out, "        Character {{ ascii: {c}, xmin: {}, ymin: {}, width: {}, height: {}, bitmap: &{bitmap:?} }},", metrics.xmin, metrics.ymin, metrics.width, metrics.height).unwrap();
        }
    }
    writeln!(
        out,
        r#"    ],
}};"#
    )
    .unwrap();
    std::fs::write(
        format!("{}/font.rs", std::env::var("OUT_DIR").unwrap()),
        out,
    )
    .unwrap();
}
