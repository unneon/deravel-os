#![no_std]
#![no_main]

use deravel_kernel_api::*;
use log::debug;

fn main(args: Args) {
    let _window = args.windowing.create_window();
    debug!("created window");
    loop {
        yield_();
    }
}

app! { main Terminal }
