use crate::sbi;
use fdt::Fdt;
use log::{Level, LevelFilter, Metadata, Record, info};

struct Logger {
    start_time: u64,
    timebase_frequency: usize,
}

struct PrettyLogLevel(Level);

struct PrettyModulePath<'a>(Option<&'a str>);

static mut LOGGER: Logger = Logger {
    start_time: 0,
    timebase_frequency: 0,
};

impl log::Log for Logger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let time = (riscv::register::time::read64() - self.start_time) as f64
                / self.timebase_frequency as f64;
            let level = PrettyLogLevel(record.level());
            let module = PrettyModulePath(record.module_path());
            sbi::console_writeln!("[{time:>13.7}] {level} {module}{}", record.args());
        }
    }

    fn flush(&self) {}
}

impl core::fmt::Display for PrettyLogLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self.0 {
            Level::Error => "\x1B[1;31mERRO\x1B[0m",
            Level::Warn => "\x1B[1;33mWARN\x1B[0m",
            Level::Info => "\x1B[1;32mINFO\x1B[0m",
            Level::Debug => "\x1B[1;36mDEBG\x1B[0m",
            Level::Trace => "\x1B[1;34mTRCE\x1B[0m",
        })
    }
}

impl core::fmt::Display for PrettyModulePath<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let Some(path) = self.0 else {
            return Ok(());
        };
        if path == "deravel_kernel" {
            return Ok(());
        }
        let last = path.split("::").last().unwrap();
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
    log::set_max_level(LevelFilter::Debug);
    info!("timebase frequency is {timebase_frequency}");
}

pub fn log_userspace(level: Level, process_name: &str, message: &str) {
    let time = (riscv::register::time::read64() - unsafe { LOGGER.start_time }) as f64
        / unsafe { LOGGER.timebase_frequency } as f64;
    let level = PrettyLogLevel(level);
    sbi::console_writeln!(
        "[\x1B[36m{time:>13.7}] {level}\x1B[36m \x1B[1m{process_name}:\x1B[0;36m {}\x1B[0m",
        message
    );
}

fn find_timebase_frequency(device_tree: &Fdt) -> Option<usize> {
    device_tree
        .find_node("/cpus")?
        .property("timebase-frequency")?
        .as_usize()
}
