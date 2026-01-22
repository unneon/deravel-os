mod ffi;

#[cfg(doc)]
use Error::*;

pub macro console_writeln($($arg:tt)*) {
    core::fmt::write(&mut crate::sbi::Console, format_args!("{}\n", format_args!($($arg)*))).unwrap()
}

#[doc(hidden)]
pub struct Console;

#[allow(dead_code)]
#[derive(Debug)]
#[repr(isize)]
pub enum Error {
    /// Failed.
    Failed = -1,
    /// Not supported.
    NotSupported = -2,
    /// Invalid parameter(s).
    InvalidParam = -3,
    /// Denied or not allowed.
    Denied = -4,
    /// Invalid address(s).
    InvalidAddress = -5,
    /// Already available.
    AlreadyAvailable = -6,
    /// Already started.
    AlreadyStarted = -7,
    /// Already stopped.
    AlreadyStopped = -8,
    /// Shared memory not available.
    NoShmem = -9,
    /// Invalid state.
    InvalidState = -10,
    /// Bad (or invalid) range.
    BadRange = -11,
    /// Failed due to timeout.
    Timeout = -12,
    /// Input/Output error.
    Io = -13,
    /// Denied or not allowed due to lock status.
    DeniedLocked = -14,
}

pub struct ImplId(usize);

#[allow(dead_code)]
#[repr(u32)]
pub enum ResetType {
    /// Power down of the entire system.
    Shutdown = 0,
    /// Power cycle of the entire system.
    ColdReboot = 1,
    /// Power cycle of the main processor and parts of the system, but not the entire system.
    ///
    /// For example, on a server class system with a BMC (board management controller), a warm reboot will not power cycle the BMC.
    WarmReboot = 2,
}

#[allow(dead_code)]
#[repr(u32)]
pub enum ResetReason {
    NoReason = 0,
    SystemFailure = 1,
}

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
            let written = debug_console_write(to_write).map_err(|_| core::fmt::Error)?;
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

/// Returns if the given SBI extension ID (EID) is available.
#[allow(dead_code)]
pub fn probe_extension(extension_id: usize) -> bool {
    ffi::sbi_probe_extension(extension_id) != 0
}

/// Write bytes to the debug console from input memory.
///
/// This is a non-blocking SBI call and it may do partial/no writes if the debug console is not able to accept
/// more bytes.
///
/// The number of bytes written is returned.
///
/// | Error code | Description |
/// | ---------- | ----------- |
/// | [`InvalidParam`] | The memory pointed to by the `bytes` parameter does not satisfy the requirements. |
/// | [`Denied`] | Writes to the debug console is not allowed. |
/// | [`Failed`] | Failed to write due to I/O errors. |
pub fn debug_console_write(bytes: &[u8]) -> Result<usize, Error> {
    ffi::sbi_debug_console_write(bytes.len(), bytes.as_ptr() as usize, 0)
}

/// Read bytes from the debug console into an output memory.
///
/// This is a non-blocking SBI call and it will not write anything into the output memory if there are no bytes
/// to be read in the debug console.
///
/// The number of bytes read is returned.
///
/// | Error code | Description |
/// | ---------- | ----------- |
/// | [`InvalidParam`] | The memory pointed to by the `bytes` parameter does not satisfy the requirements. |
/// | [`Denied`] | Reads from the debug console is not allowed. |
/// | [`Failed`] | Failed to read due to I/O errors. |
#[allow(dead_code)]
pub fn debug_console_read(bytes: &mut [u8]) -> Result<usize, Error> {
    ffi::sbi_debug_console_read(bytes.len(), bytes.as_ptr() as usize, 0)
}

/// Reset the system based on provided [`ResetType`] and [`ResetReason`].
///
/// This is a synchronous call.
///
/// When supervisor software is running natively, the SBI implementation is provided by machine mode
/// firmware. When supervisor software is running inside a virtual machine, the SBI implementation is provided by a
/// hypervisor. [Shutdown](ResetType::Shutdown), [cold reboot](ResetType::ColdReboot) and [warm reboot](ResetType::WarmReboot) will behave functionally the same as the native case,
/// but might not result in any physical power changes.
///
/// | Error code | Description |
/// | ---------- | ----------- |
/// | [`InvalidParam`] | At least one of `reset_type` or `reset_reason` is reserved or is platform-specific and unimplemented. |
/// | [`NotSupported`] | `reset_type` is not reserved and is implemented, but the platform does not support it due to one or more missing dependencies. |
/// | [`Failed`] | The reset request failed for unspecified or unknown other reasons. |
pub fn system_reset(type_: ResetType, reason: ResetReason) -> Result<!, Error> {
    ffi::sbi_system_reset(type_ as u32, reason as u32)
}
