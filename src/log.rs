use crate::sbi;
use fdt::Fdt;
use log::{Level, LevelFilter, Metadata, Record, info};

struct Logger {
    start_time: u64,
    timebase_frequency: usize,
}

struct PrettyModulePath<'a>(&'a str);

static mut LOGGER: Logger = Logger {
    start_time: 0,
    timebase_frequency: 0,
};

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.target().starts_with(env!("CARGO_PKG_NAME"))
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let time = (riscv::register::time::read64() - self.start_time) as f64
                / self.timebase_frequency as f64;
            let level = match record.level() {
                Level::Error => "\x1B[1;31mERRO\x1B[0m",
                Level::Warn => "\x1B[1;33mWARN\x1B[0m",
                Level::Info => "\x1B[1;32mINFO\x1B[0m",
                Level::Debug => "\x1B[1;36mDEBG\x1B[0m",
                Level::Trace => "\x1B[1;34mTRCE\x1B[0m",
            };
            let module = PrettyModulePath(record.module_path().unwrap());
            sbi::console_writeln!("[{time:>13.7}] {level} {module}{}", record.args());
        }
    }

    fn flush(&self) {}
}

impl core::fmt::Display for PrettyModulePath<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0 == "deravel" {
            return Ok(());
        }
        let last = self.0.split("::").last().unwrap();
        write!(f, "\x1B[1m{last}: \x1B[0m")
    }
}

pub fn initialize_log(device_tree: &Fdt) {
    let timebase_frequency = find_timebase_frequency(device_tree).unwrap();
    unsafe {
        LOGGER.start_time = riscv::register::time::read64();
        LOGGER.timebase_frequency = timebase_frequency;
        log::set_logger(&LOGGER).unwrap();
    };
    log::set_max_level(LevelFilter::Trace);
    info!("timebase frequency is {timebase_frequency}");
}

fn find_timebase_frequency(device_tree: &Fdt) -> Option<usize> {
    device_tree
        .find_node("/cpus")?
        .property("timebase-frequency")?
        .as_usize()
}
