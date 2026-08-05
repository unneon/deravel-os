use crate::device_tree::timebase_frequency;
use crate::process::Process;
use crate::sbi;
use alloc::format;
use log::{Level, LevelFilter, Metadata, Record};

struct Logger {
    start_time: u64,
}

enum Time {
    Float(f64),
    Int(u64),
}

struct PrettyLogLevel(Level);

struct PrettyModulePath<'a>(Option<&'a str>);

static mut LOGGER: Logger = Logger { start_time: 0 };

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        !metadata.target().starts_with("smoltcp")
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let early_color = match record.level() {
                Level::Error => "\x1B[31m",
                Level::Warn => "\x1B[33m",
                _ if record.module_path().is_some() => "",
                _ => "\x1B[36m",
            };
            let time = riscv::register::time::read64() - self.start_time;
            let time = match timebase_frequency() {
                Some(timebase_frequency) => {
                    Time::Float(time as f64 / timebase_frequency.get() as f64)
                }
                None => Time::Int(time),
            };
            let level = PrettyLogLevel(record.level());
            let message = record.args();
            if record.module_path().is_some() {
                let module = PrettyModulePath(record.module_path());
                sbi::console_writeln!("{early_color}{time} {level} {module}{message}\x1B[0m");
            } else {
                let process_name = record.target();
                sbi::console_writeln!(
                    "{early_color}{time} {level}{early_color} \x1B[1m{process_name}:\x1B[0m{early_color} {message}\x1B[0m"
                );
            }
        }
    }

    fn flush(&self) {}
}

impl core::fmt::Display for Time {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Time::Float(time) => write!(f, "[{time:>13.7}]"),
            Time::Int(time) => write!(f, "[{time:>13}]"),
        }
    }
}

impl core::fmt::Display for PrettyLogLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self.0 {
            Level::Error => "\x1B[1mERRO\x1B[22m",
            Level::Warn => "\x1B[1mWARN\x1B[22m",
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
        write!(f, ":\x1B[22m ")
    }
}

pub fn initialize_log() {
    // Log is initialized before the heap, so this is kind of the best option. Later one, I might
    // add some typestate tokens that can ensure each subsystem gets initialized only once.
    let logger = unsafe { &mut *&raw mut LOGGER };
    logger.start_time = riscv::register::time::read64();
    log::set_logger(logger).unwrap();
    log::set_max_level(LevelFilter::Debug);
}

pub fn log_userspace(level: Level, proc: &Process, message: &str) {
    let args = format_args!("{message}");
    let name = proc.name;
    let pid = proc.id;
    let target = format!("{name}{pid:?}");
    let record = Record::builder()
        .args(args)
        .level(level)
        .target(&target)
        .build();
    log::logger().log(&record);
}
