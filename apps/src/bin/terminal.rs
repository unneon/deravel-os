#![no_std]
#![no_main]

use deravel_kernel_api::*;
use log::debug;

fn main(args: Args) {
    debug!("creating window...");
    let _window = args.windowing.create_window();
    debug!("created window");
}

app! { main Terminal }
