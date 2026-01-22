use crate::sbi;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::BitOr;

trait Readable {}
trait Writeable {}

struct Read;
struct ReadWrite;
struct Write;

struct MmioDeviceRegisters {
    magic_value: MmioDeviceRegister<u32, 0x000, Read>,
    version: MmioDeviceRegister<u32, 0x004, Read>,
    device_id: MmioDeviceRegister<u32, 0x008, Read>,
    guest_page_size: MmioDeviceRegister<u32, 0x028, Write>,
    queue_sel: MmioDeviceRegister<u32, 0x030, Write>,
    queue_num: MmioDeviceRegister<u32, 0x038, Write>,
    queue_pfn: MmioDeviceRegister<u32, 0x040, ReadWrite>,
    queue_notify: MmioDeviceRegister<u32, 0x050, Write>,
    device_status: MmioDeviceRegister<u32, 0x070, ReadWrite>,
}

struct MmioDeviceRegister<T, const OFFSET: usize, D>(PhantomData<(T, D)>);

#[repr(C, packed)]
struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C, packed)]
struct VirtqAvail {
    flags: u16,
    index: u16,
    ring: [u16; VIRTQ_ENTRY_NUM],
}

#[repr(C, packed)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

#[repr(C, packed)]
struct VirtqUsed {
    flags: u16,
    index: u16,
    ring: [VirtqUsedElem; VIRTQ_ENTRY_NUM],
}

#[repr(C, packed)]
struct VirtioVirtq {
    descs: [VirtqDesc; VIRTQ_ENTRY_NUM],
    avail: VirtqAvail,
    _pad0: [u8; 4096 - size_of::<[VirtqAvail; VIRTQ_ENTRY_NUM]>() - size_of::<VirtqAvail>()],
    used: VirtqUsed,
    queue_index: i32,
    used_index: *const u16,
    last_used_index: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct VirtioBlkReq {
    type_: u32,
    reserved: u32,
    sector: u64,
    data: [u8; 512],
    status: u8,
}

#[repr(align(4096))]
struct PageAligned<T>(T);

const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;
const VIRTQ_ENTRY_NUM: usize = 16;
const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;
const STATUS_ACK: u32 = 1;
const STATUS_DRIVER: u32 = 2;
const STATUS_DRIVER_OK: u32 = 4;

static mut VIRTQ: MaybeUninit<PageAligned<VirtioVirtq>> = MaybeUninit::uninit();

impl<T, const OFFSET: usize, D: Readable> MmioDeviceRegister<T, OFFSET, D> {
    pub fn read(&self) -> T {
        unsafe { (((self as *const _ as usize) + OFFSET) as *const T).read_volatile() }
    }
}

impl<T, const OFFSET: usize, D: Writeable> MmioDeviceRegister<T, OFFSET, D> {
    pub fn write(&mut self, value: T) {
        unsafe { (((self as *mut _ as usize) + OFFSET) as *mut T).write_volatile(value) }
    }
}

impl<T: BitOr<Output = T>, const OFFSET: usize> MmioDeviceRegister<T, OFFSET, ReadWrite> {
    pub fn or_assign(&mut self, value: T) {
        self.write(self.read() | value);
    }
}

impl Readable for Read {}
impl Readable for ReadWrite {}
impl Writeable for ReadWrite {}
impl Writeable for Write {}

unsafe impl Send for VirtioVirtq {}
unsafe impl Sync for VirtioVirtq {}

pub fn virtio_blk_initialize() {
    let regs = mmio_device_registers();
    assert_eq!(regs.magic_value.read(), 0x74726976);
    assert_eq!(regs.version.read(), 1);
    assert_eq!(regs.device_id.read(), 2);

    sbi::console_writeln!("virtio: verified metadata");

    regs.device_status.write(0);
    regs.device_status.or_assign(STATUS_ACK);
    regs.device_status.or_assign(STATUS_DRIVER);
    regs.guest_page_size.write(4096);
    sbi::console_writeln!("virtio: pre-queue initialization complete");

    let queue = virtq_init(0, regs);
    regs.device_status.write(STATUS_DRIVER_OK);
    sbi::console_writeln!("virtio: queue initialization complete");

    read_write_disk(&mut [], 0, false, queue, regs);
}

fn mmio_device_registers() -> &'static mut MmioDeviceRegisters {
    unsafe { &mut *(0x1000_1000 as *mut MmioDeviceRegisters) }
}

fn virtq_init(index: usize, regs: &mut MmioDeviceRegisters) -> &'static mut VirtioVirtq {
    #[allow(clippy::deref_addrof)]
    let vq = &mut unsafe { (*&raw mut VIRTQ).assume_init_mut() }.0;
    vq.queue_index = index as i32;
    vq.used_index = &raw const vq.used.index;
    regs.queue_sel.write(index as u32);
    regs.queue_num.write(VIRTQ_ENTRY_NUM as u32);
    regs.queue_pfn
        .write(((vq as *mut _ as usize) / 4096) as u32);
    vq
}

fn virtq_kick(vq: &mut VirtioVirtq, desc_index: i32, regs: &mut MmioDeviceRegisters) {
    vq.avail.ring[vq.avail.index as usize % VIRTQ_ENTRY_NUM] = desc_index as u16;
    vq.avail.index += 1;
    // TODO: What is a __sync_synchronize equivalent? Is even that correct?
    riscv::asm::fence();
    regs.queue_notify.write(vq.queue_index as u32);
    vq.last_used_index += 1;
}

#[allow(dead_code)]
fn virtq_is_busy(vq: &mut VirtioVirtq) -> bool {
    vq.last_used_index != unsafe { *vq.used_index }
}

fn read_write_disk(
    buf: &mut [u8],
    sector: u32,
    is_write: bool,
    vq: &mut VirtioVirtq,
    regs: &mut MmioDeviceRegisters,
) {
    let mut blk_req = VirtioBlkReq {
        type_: 0,
        reserved: 0,
        sector: 0,
        data: [0; 512],
        status: 0,
    };
    blk_req.type_ = if is_write {
        VIRTIO_BLK_T_OUT
    } else {
        VIRTIO_BLK_T_IN
    };
    blk_req.sector = sector as u64;
    if is_write {
        blk_req.data[..buf.len()].copy_from_slice(buf);
    }

    vq.descs[0].addr = &blk_req as *const _ as u64;
    vq.descs[0].len = 16;
    vq.descs[0].flags = VIRTQ_DESC_F_NEXT;
    vq.descs[0].next = 1;

    vq.descs[1].addr = (&blk_req as *const _ as u64) + 16;
    vq.descs[1].len = 512;
    vq.descs[1].flags = VIRTQ_DESC_F_NEXT | if is_write { 0 } else { VIRTQ_DESC_F_WRITE };
    vq.descs[1].next = 2;

    vq.descs[2].addr = (&blk_req as *const _ as u64) + 16 + 512;
    vq.descs[2].len = 1;
    vq.descs[2].flags = VIRTQ_DESC_F_WRITE;

    sbi::console_writeln!("virtio: request written to memory");

    virtq_kick(vq, 0, regs);

    sbi::console_writeln!("virtio: kick executed");

    // while virtq_is_busy(vq) {}

    sbi::console_writeln!("virtio: finished waiting");

    if blk_req.status != 0 {
        sbi::console_writeln!(
            "virtio: warn: failed to read/write sector={sector} status={}",
            blk_req.status
        );
        return;
    }

    if !is_write {
        let s = str::from_utf8(&blk_req.data.as_slice()[..64]).unwrap();
        sbi::console_writeln!("virtio: successfully read {s:?}");
    }
}
