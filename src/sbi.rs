pub mod ffi;

pub macro console_writeln($($arg:tt)*) {
    core::fmt::write(&mut crate::sbi::Console, format_args!("{}\n", format_args!($($arg)*))).unwrap()
}

#[doc(hidden)]
pub struct Console;

pub struct ImplId(usize);

pub struct SpecVersion(u32);

impl ImplId {
    pub fn number(&self) -> usize {
        self.0
    }

    pub fn name(&self) -> Option<&'static str> {
        const KNOWN_NAMES: [&str; 12] = [
            "Berkeley Boot Loader (BBL)",
            "OpenSBI",
            "Xvisor",
            "KVM",
            "RustSBI",
            "Diosix",
            "Coffer",
            "Xen Project",
            "PolarFire Hart Software Services",
            "coreboot",
            "oreboot",
            "bhyve",
        ];
        KNOWN_NAMES.get(self.0).copied()
    }
}

impl SpecVersion {
    pub fn minor(&self) -> u32 {
        self.0 & ((1 << 24) - 1)
    }

    pub fn major(&self) -> u32 {
        (self.0 >> 24) & ((1 << 7) - 1)
    }
}

impl core::fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut to_write = s.as_bytes();
        while !to_write.is_empty() {
            let written = ffi::sbi_debug_console_write(to_write.len(), s.as_ptr() as usize, 0)
                .map_err(|_| core::fmt::Error)?;
            to_write = &to_write[written..];
        }
        Ok(())
    }
}

impl core::fmt::Display for ImplId {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self.name() {
            Some(name) => write!(f, "{name}"),
            None => write!(f, "<unknown id={}>", self.number()),
        }
    }
}

impl core::fmt::Display for SpecVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}.{}", self.major(), self.minor())
    }
}

/// Returns the current SBI specification version.
pub fn get_spec_version() -> SpecVersion {
    SpecVersion(ffi::sbi_get_spec_version() as u32)
}

/// Returns the current SBI implementation ID.
///
/// It is intended that this implementation ID allows software to probe for SBI implementation quirks.
pub fn get_impl_id() -> ImplId {
    ImplId(ffi::sbi_get_impl_id())
}

/// Returns the current SBI implementation version.
///
/// The encoding of this version number is specific to the SBI implementation.
pub fn get_impl_version() -> usize {
    ffi::sbi_get_impl_version()
}
