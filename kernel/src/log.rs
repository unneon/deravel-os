use crate::sbi;
use alloc::boxed::Box;
use fdt::Fdt;
use log::{Level, LevelFilter, Metadata, Record, info};

struct Logger {
    start_time: u64,
    timebase_frequency: usize,
}

struct PrettyLogLevel(Level);

struct PrettyModulePath<'a>(Option<&'a str>);

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        !metadata.target().starts_with("smoltcp")
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let time = (riscv::register::time::read64() - self.start_time) as f64
                / self.timebase_frequency as f64;
            let level = PrettyLogLevel(record.level());
            let message = record.args();
            if record.module_path().is_some() {
                let module = PrettyModulePath(record.module_path());
                sbi::console_writeln!("[{time:>13.7}] {level} {module}{message}");
            } else {
                let process_name = record.target();
                sbi::console_writeln!(
                    "[\x1B[36m{time:>13.7}] {level}\x1B[36m \x1B[1m{process_name}:\x1B[0;36m {message}\x1B[0m"
                );
            }
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
        write!(f, "\x1B[1m")?;
        for segment in path
            .split("::")
            .filter(|&seg| seg != "deravel_kernel")
            .intersperse(".")
        {
            write!(f, "{segment}")?;
        }
        write!(f, ":\x1B[0m ")
    }
}

pub fn initialize_log(device_tree: &Fdt) {
    let timebase_frequency = find_timebase_frequency(device_tree).unwrap();
    let logger = Logger {
        start_time: riscv::register::time::read64(),
        timebase_frequency,
    };
    log::set_logger(Box::leak(Box::new(logger))).unwrap();
    log::set_max_level(LevelFilter::Trace);
    info!("timebase frequency is {timebase_frequency}");
}

pub fn log_userspace(level: Level, process_name: &str, message: &str) {
    let args = format_args!("{message}");
    let record = Record::builder()
        .args(args)
        .level(level)
        .target(process_name)
        .build();
    log::logger().log(&record);
}

fn find_timebase_frequency(device_tree: &Fdt) -> Option<usize> {
    device_tree
        .find_node("/cpus")?
        .property("timebase-frequency")?
        .as_usize()
}
