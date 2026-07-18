use crate::util::address::Address;
use core::fmt::Formatter;
use core::ops::Range;

struct Memory {
    range: Range<usize>,
}

struct MemorySize {
    bytes: usize,
}

const KIBIBYTE: usize = 1024;
const MEBIBYTE: usize = 1024 * KIBIBYTE;
const GIBIBYTE: usize = 1024 * MEBIBYTE;
const TEBIBYTE: usize = 1024 * GIBIBYTE;
const PEBIBYTE: usize = 1024 * TEBIBYTE;

impl MemorySize {
    fn fmt_unit(
        &self,
        unit_size: usize,
        unit_name: &str,
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        if self.bytes.is_multiple_of(unit_size) {
            write!(f, "{} {unit_name}", self.bytes / unit_size)
        } else {
            write!(
                f,
                "{:.2} {unit_name}",
                (self.bytes as f32) / (unit_size as f32)
            )
        }
    }
}

impl core::fmt::Display for Memory {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:#x}..{:#x} ({})",
            self.range.start,
            self.range.end,
            fmt_memory_size(self.range.end - self.range.start)
        )
    }
}

impl core::fmt::Display for MemorySize {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.bytes < KIBIBYTE {
            write!(f, "{} bytes", self.bytes)
        } else if self.bytes < MEBIBYTE {
            self.fmt_unit(KIBIBYTE, "KiB", f)
        } else if self.bytes < GIBIBYTE {
            self.fmt_unit(MEBIBYTE, "MiB", f)
        } else if self.bytes < TEBIBYTE {
            self.fmt_unit(GIBIBYTE, "GiB", f)
        } else if self.bytes < PEBIBYTE {
            self.fmt_unit(TEBIBYTE, "TiB", f)
        } else {
            self.fmt_unit(PEBIBYTE, "PiB", f)
        }
    }
}

pub fn fmt_memory<T: Address<Raw = usize>>(range: &Range<T>) -> impl core::fmt::Display {
    Memory {
        range: range.raw_addr(),
    }
}

pub fn fmt_memory_size(bytes: usize) -> impl core::fmt::Display {
    MemorySize { bytes }
}
