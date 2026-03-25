use crate::DISK;
use crate::allocators::TrivialAllocator;
use crate::pci::capability::{PciCapability, VendorPciCapability};
use crate::pci::{AllocatedRange, GeneralDeviceConfig, walk_capabilities};
use crate::util::volatile::{ReadWrite, Volatile, volatile_struct};
use crate::virtio::blk::VirtioBlk;
use crate::virtio::gpu::VirtioGpu;
use crate::virtio::input::VirtioInput;
use crate::virtio::net::VirtioNet;
use log::{debug, warn};

pub mod blk;
pub mod gpu;
pub mod input;
pub mod net;
pub mod queue;
pub mod registers;

struct Configs<T> {
    common: Volatile<VirtioCommonConfig>,
    notify: NotifySlot,
    device: Volatile<T>,
}

pub struct NotifySlot {
    base: *mut u16,
    off_multiplier: u32,
}

volatile_struct! { pub VirtioCommonConfig
    device_feature_select: ReadWrite u32,
    device_feature: Readonly u32,
    driver_feature_select: ReadWrite u32,
    driver_feature: ReadWrite u32,
    config_msix_vector: ReadWrite u16,
    num_queues: ReadWrite u16,
    device_status: ReadWrite u8,
    config_generation: ReadWrite u8,

    queue_select: ReadWrite u16,
    queue_size: ReadWrite u16,
    queue_msix_vector: ReadWrite u16,
    queue_enable: ReadWrite u16,
    queue_notify_off: Readonly u16,
    queue_desc: ReadWrite u64,
    queue_driver: ReadWrite u64,
    queue_device: ReadWrite u64,
}

#[repr(C)]
#[derive(Debug)]
struct VirtioPciCapability {
    cap: PciCapability,
    cap_len: u8,
    cfg_type: u8,
    bar: u8,
    id: u8,
    padding: [u8; 2],
    offset: u32,
    length: u32,
}

#[repr(C)]
struct VirtioPciNotifyCapability {
    cap: VirtioPciCapability,
    notify_off_multiplier: u32,
}

const VIRTIO_PCI_CAP_COMMON_CFG: u8 = 1;
const VIRTIO_PCI_CAP_NOTIFY_CFG: u8 = 2;
const VIRTIO_PCI_CAP_DEVICE_CFG: u8 = 4;

impl NotifySlot {
    unsafe fn select(&self, common: Volatile<VirtioCommonConfig>) -> Volatile<u16> {
        let offset = common.queue_notify_off().read() as usize * self.off_multiplier as usize;
        unsafe { Volatile::new(self.base.byte_add(offset)) }
    }
}

unsafe impl VendorPciCapability for VirtioPciCapability {}

pub fn initialize_blk(config: Volatile<GeneralDeviceConfig>, bars: &[AllocatedRange; 6]) {
    let configs = extract_configs(config, bars);
    let device = VirtioBlk::new(configs.common, configs.notify, configs.device);
    unsafe {
        assert!(DISK.is_none());
        DISK = Some(device);
    }
}

pub fn initialize_gpu(config: Volatile<GeneralDeviceConfig>, bars: &[AllocatedRange; 6]) {
    let configs = extract_configs(config, bars);
    let mut virtio_gpu = VirtioGpu::new(configs.common, configs.notify, configs.device);
    virtio_gpu.demo();
}

pub fn initialize_input(config: Volatile<GeneralDeviceConfig>, bars: &[AllocatedRange; 6]) {
    let configs = extract_configs(config, bars);
    let mut virtio_input = VirtioInput::new(configs.common, configs.notify, configs.device);
    virtio_input.demo();
}

pub fn initialize_net(config: Volatile<GeneralDeviceConfig>, bars: &[AllocatedRange; 6]) {
    let configs = extract_configs(config, bars);
    let mut virtio_net = VirtioNet::new(configs.common, configs.notify, configs.device);
    virtio_net.demo();
}

volatile_struct! { MsiXEntry
    message_address_low: ReadWrite u32,
    message_address_high: ReadWrite u32,
    message_data: ReadWrite u32,
    vector_control: ReadWrite u32,
}

static mut TEST_MSIX: [u32; 16] = [0; 16];
static mut TEST_MSIX_ALLOCATOR: TrivialAllocator = TrivialAllocator::new(16);

fn extract_configs<T>(
    config: Volatile<GeneralDeviceConfig>,
    bars: &[AllocatedRange; 6],
) -> Configs<T> {
    let mut common = None;
    let mut notify = None;
    let mut device = None;
    for cap in walk_capabilities(config) {
        if let Some(cap) = unsafe { cap.get_vendor::<VirtioPciCapability>() } {
            let address = bars[cap.bar as usize].soc_offset + cap.offset as usize;
            if cap.cfg_type == VIRTIO_PCI_CAP_COMMON_CFG {
                assert!(common.is_none());
                common = Some(unsafe { Volatile::new(address as *mut VirtioCommonConfig) });
            } else if cap.cfg_type == VIRTIO_PCI_CAP_NOTIFY_CFG {
                assert!(notify.is_none());
                let cap = unsafe {
                    &*(cap as *const VirtioPciCapability as *const VirtioPciNotifyCapability)
                };
                notify = Some(NotifySlot {
                    base: address as *mut u16,
                    off_multiplier: cap.notify_off_multiplier,
                });
            } else if cap.cfg_type == VIRTIO_PCI_CAP_DEVICE_CFG {
                assert!(device.is_none());
                device = Some(unsafe { Volatile::new(address as *mut T) });
            }
        } else if let Some(msi_x) = cap.get_msi_x() {
            debug!("msix: {:x?}", unsafe { TEST_MSIX });
            let message_control = msi_x.message_control().read();
            let table = msi_x.table().read();
            let message_table =
                (bars[table.bir() as usize].soc_offset + table.offset() as usize) as *mut MsiXEntry;
            for i in 0..message_control.table_size() as usize {
                let entry: Volatile<_, ReadWrite> =
                    unsafe { Volatile::new(message_table.wrapping_add(i)) };
                debug!(
                    "[{i}] before, ma {:#x}:{:x}, md {:#x}, vc {:#x}",
                    entry.message_address_high().read(),
                    entry.message_address_low().read(),
                    entry.message_data().read(),
                    entry.vector_control().read()
                );
                let address =
                    unsafe { &raw const TEST_MSIX[TEST_MSIX_ALLOCATOR.allocate(1, 1)] } as usize;
                entry.message_address_low().write(address as u32);
                entry.message_address_high().write((address >> 32) as u32);
                entry.message_data().write(0x666);
                entry.vector_control().write(0);
                debug!(
                    "[{i}] after,  ma {:#x}:{:x}, md {:#x}, vc {:#x}",
                    entry.message_address_high().read(),
                    entry.message_address_low().read(),
                    entry.message_data().read(),
                    entry.vector_control().read()
                );
            }
            debug!("before {message_control:?}, {table:?}");
            msi_x
                .message_control()
                .write(message_control.with_enable(true));
            let message_control = msi_x.message_control().read();
            let table = msi_x.table().read();
            debug!("after  {message_control:?}, {table:?}");
            debug!("msix: {:x?}", unsafe { TEST_MSIX });
        } else {
            warn!("unknown capability {:?}", cap);
        }
    }
    Configs {
        common: common.unwrap(),
        notify: notify.unwrap(),
        device: device.unwrap(),
    }
}
