#![no_std]
#![no_main]

use deravel_kernel_api::{app, getchar, print, println, putchar};

fn main() {
    let mut buf = [0; 128];
    'prompt: loop {
        print!("> ");
        let Some(cmdline) = getline(&mut buf) else {
            println!("\ncommand line too long");
            continue 'prompt;
        };

        if cmdline == "hello" {
            println!("Hello world from shell!");
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

app! { main }
