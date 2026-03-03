use core::arch::asm;
use core::hint::unreachable_unchecked;
use core::mem::transmute_copy;
use deravel_types::ProcessId;
use deravel_types::capability::Capability;

macro syscalls($(#[no = $no:literal] pub fn $name:ident($($a0name:ident: $a0type:ty$(, $a1name:ident: $a1type:ty$(, $a2name:ident: $a2type:ty)?)?)?) $(-> $return_type:ty)?;)*) {
    $(pub fn $name($($a0name: $a0type$(, $a1name: $a1type$(, $a2name: $a2type)?)?)?) $(-> $return_type)? {
        let _a0: usize;
        let _a1: usize;
        unsafe {
            asm!(
                "ecall",
                $(in("a0") <$a0type as To1A>::to_1a($a0name),
                $(in("a1") <$a1type as To1A>::to_1a($a1name),
                $(in("a2") <$a2type as To1A>::to_1a($a2name),
                )?)?)?
                in("a3") $no,
                lateout("a0") _a0,
                lateout("a1") _a1,
            )
        }
        $(<$return_type as FromA0A1>::from_a0a1(_a0, _a1))?
    })*
}

trait FromA0A1 {
    fn from_a0a1(a0: usize, a1: usize) -> Self;
}

trait To1A {
    fn to_1a(self) -> usize;
}

impl FromA0A1 for u8 {
    fn from_a0a1(a0: usize, _: usize) -> u8 {
        debug_assert!(a0 <= u8::MAX as usize);
        unsafe { transmute_copy::<usize, u8>(&a0) }
    }
}

impl FromA0A1 for usize {
    fn from_a0a1(a0: usize, _: usize) -> usize {
        a0
    }
}

impl FromA0A1 for *mut u8 {
    fn from_a0a1(a0: usize, _: usize) -> Self {
        a0 as *mut u8
    }
}

impl FromA0A1 for ProcessId {
    fn from_a0a1(a0: usize, _: usize) -> ProcessId {
        ProcessId(a0)
    }
}

impl FromA0A1 for (usize, ProcessId) {
    fn from_a0a1(a0: usize, a1: usize) -> Self {
        (a0, ProcessId(a1))
    }
}

impl FromA0A1 for (Capability, ProcessId) {
    fn from_a0a1(a0: usize, a1: usize) -> Self {
        (Capability(a0 as *const _), ProcessId(a1))
    }
}

impl FromA0A1 for ! {
    fn from_a0a1(_: usize, _: usize) -> ! {
        unsafe { unreachable_unchecked() }
    }
}

impl To1A for u8 {
    fn to_1a(self) -> usize {
        self as usize
    }
}

impl To1A for usize {
    fn to_1a(self) -> usize {
        self
    }
}

impl<T> To1A for *const T {
    fn to_1a(self) -> usize {
        self as usize
    }
}

impl<T> To1A for *mut T {
    fn to_1a(self) -> usize {
        self as usize
    }
}

impl<T> To1A for &T {
    fn to_1a(self) -> usize {
        self as *const T as usize
    }
}

impl<T> To1A for &mut T {
    fn to_1a(self) -> usize {
        self as *mut T as usize
    }
}

impl To1A for Capability {
    fn to_1a(self) -> usize {
        self.0 as usize
    }
}

syscalls! {
    #[no = 1]
    pub fn exit() -> !;

    #[no = 2]
    pub fn putchar(ch: u8);

    #[no = 3]
    pub fn getchar() -> u8;

    #[no = 4]
    pub fn yield_();

    #[no = 5]
    pub fn pid_by_name(name: *const u8, name_len: usize) -> ProcessId;

    #[no = 6]
    pub fn ipc_send(data: *const u8, data_len: usize, dest: usize);

    #[no = 7]
    pub fn ipc_recv(buf: *mut u8, buf_max_len: usize) -> (usize, ProcessId);

    #[no = 8]
    pub fn log(text: *const u8, text_len: usize, level: usize);

    #[no = 9]
    pub fn disk_read(sector: usize, buf: &mut [u8; 512]);

    #[no = 10]
    pub fn disk_write(sector: usize, buf: &[u8; 512]);

    #[no = 11]
    pub fn disk_capacity() -> usize;

    #[no = 12]
    pub fn allocate_pages(count: usize) -> *mut u8;
}
