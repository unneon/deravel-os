#![no_std]
#![no_main]

use deravel_kernel_api::*;
use log::*;

fn main(args: ShellArgs) {
    set_stdio(args.console);
    let mut buf = [0; 128];
    loop {
        print!("> ");
        let Some(cmdline) = getline(&mut buf) else {
            println!("\ncommand line too long");
            continue;
        };

        if cmdline == "hello" {
            println!("Hello world from shell!");
        } else if let Some(file_name) = cmdline.strip_prefix("read ") {
            let file = args.fs.read(file_name);
            print!("{}", str::from_utf8(&file).unwrap());
        } else if let Some(file_name) = cmdline.strip_prefix("write ") {
            let mut file_buf = [0; 512];
            let Some(file) = getmultiline(&mut file_buf) else {
                println!("\nfile contents too long");
                continue;
            };
            args.fs.write(file_name, file.as_bytes());
        } else if let Some(file_name) = cmdline.strip_prefix("image ") {
            let file = args.fs.read_large(file_name);
            let file = forward(file, Actor::Kernel);
            let windowing = forward(args.windowing, Actor::Kernel);
            args.image_viewer.spawn(file, windowing);
        } else if let Some(domain) = cmdline.strip_prefix("dns ") {
            let ip = args.net.dns(domain);
            println!("{ip}");
        } else if cmdline == "shutdown" {
            args.shutdown.shutdown();
        } else if cmdline == "exit" {
            break;
        } else {
            error!("unknown command {cmdline}");
            println!("unknown command: {cmdline}");
        }
    }
}

fn getline(buf: &mut [u8]) -> Option<&str> {
    let mut i = 0;
    loop {
        let ch = getchar();
        if ch != b'\x08' {
            putchar(ch);
        }
        if ch == b'\r' {
            print!("\n");
            break Some(core::str::from_utf8(&buf[..i]).unwrap());
        } else if ch == b'\x08' {
            if i > 0 {
                print!("\x08");
                i -= 1;
            }
        } else if i == buf.len() {
            return None;
        } else {
            buf[i] = ch;
            i += 1;
        }
    }
}

fn getmultiline(buf: &mut [u8]) -> Option<&str> {
    let mut i = 0;
    let mut line_empty = true;
    while i < buf.len() {
        let ch = getchar();
        putchar(ch);
        if ch == b'\r' {
            buf[i] = b'\n';
            if line_empty {
                return Some(core::str::from_utf8(&buf[..i]).unwrap());
            }
            print!("\n");
            line_empty = true;
        } else {
            buf[i] = ch;
            line_empty = false;
        }
        i += 1;
    }
    None
}

app! { main }
