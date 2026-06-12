#![no_std]
#![no_main]

use deravel_kernel_api::*;

fn main(args: Args) {
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
            println!("{}", str::from_utf8(&file).unwrap());
        } else if let Some(file_name) = cmdline.strip_prefix("write ") {
            let mut file_buf = [0; 512];
            let Some(file) = getmultiline(&mut file_buf) else {
                println!("\nfile contents too long");
                continue;
            };
            args.fs.write(file_name, file.as_bytes());
        } else if let Some(domain) = cmdline.strip_prefix("dns ") {
            let ip = args.net.dns(domain);
            println!("{ip}");
        } else if cmdline == "shutdown" {
            args.shutdown.shutdown();
        } else if cmdline == "exit" {
            break;
        } else {
            println!("unknown command: {cmdline}");
        }
    }
}

fn getline(buf: &mut [u8]) -> Option<&str> {
    let mut i = 0;
    loop {
        let ch = getchar();
        putchar(ch);
        if ch == b'\r' {
            print!("\n");
            break Some(core::str::from_utf8(&buf[..i]).unwrap());
        } else if i == buf.len() {
            return None;
        } else {
            buf[i] = ch;
        }
        i += 1;
    }
}

fn getmultiline(buf: &mut [u8]) -> Option<&str> {
    let mut i = 0;
    let mut line_empty = true;
    loop {
        let ch = getchar();
        putchar(ch);
        if ch == b'\r' {
            if line_empty {
                break Some(core::str::from_utf8(&buf[..i]).unwrap());
            }
            print!("\n");
            line_empty = true;
        } else if i == buf.len() {
            return None;
        } else {
            buf[i] = ch;
            line_empty = false;
        }
        i += 1;
    }
}

app! { main Shell }
