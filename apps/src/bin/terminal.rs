#![no_std]
#![no_main]

use deravel_kernel_api::*;
use log::debug;

fn main(args: Args) {
    let window = args.windowing.create_window();
    debug!("created window");
    let width = window.width();
    let height = window.height();
    debug!("window is {width}x{height}");
    loop {
        yield_();
    }
}

app! { main Terminal }
