use crate::sbi;
use log::{Level, LevelFilter, Metadata, Record};

struct Logger;

struct PrettyModulePath<'a>(&'a str);

static LOGGER: Logger = Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.target().starts_with(env!("CARGO_PKG_NAME"))
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level = match record.level() {
                Level::Error => "\x1B[1;31mERRO\x1B[0m",
                Level::Warn => "\x1B[1;33mWARN\x1B[0m",
                Level::Info => "\x1B[1;32mINFO\x1B[0m",
                Level::Debug => "\x1B[1;36mDEBG\x1B[0m",
                Level::Trace => "\x1B[1;34mTRCE\x1B[0m",
            };
            let module = PrettyModulePath(record.module_path().unwrap());
            sbi::console_writeln!("{level} {module}{}", record.args());
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

pub fn initialize_log() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(LevelFilter::Trace);
}
