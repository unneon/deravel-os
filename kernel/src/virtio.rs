use crate::pci::AllocatedRange;
use crate::pci::capability::{PciCapability, VendorPciCapability};
use crate::pci::config::GeneralDeviceConfig;
use crate::util::volatile::{Readonly, Volatile, volatile_struct};
use crate::virtio::blk::VirtioBlk;
use crate::virtio::gpu::VirtioGpu;
use crate::virtio::input::VirtioInput;
use crate::virtio::net::VirtioNet;
use alloc::boxed::Box;

pub mod blk;
pub mod gpu;
pub mod input;
pub mod net;
pub mod queue;
pub mod registers;

pub struct Capabilities<T> {
    common: Volatile<VirtioCommonConfig>,
    notify: NotifySlot,
    isr: Volatile<u8, Readonly>,
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
const VIRTIO_PCI_CAP_ISR_CFG: u8 = 3;
const VIRTIO_PCI_CAP_DEVICE_CFG: u8 = 4;

impl NotifySlot {
    unsafe fn select(&self, common: Volatile<VirtioCommonConfig>) -> Volatile<u16> {
        let offset = common.queue_notify_off().read() as usize * self.off_multiplier as usize;
        unsafe { Volatile::new(self.base.byte_add(offset)) }
    }
}

unsafe impl VendorPciCapability for VirtioPciCapability {}

pub fn initialize_blk(
    config: Volatile<GeneralDeviceConfig>,
    bars: &[AllocatedRange; 6],
) -> &'static VirtioBlk {
    let caps = extract_capabilities(config, bars);
    let device = VirtioBlk::new(caps);
    // unsafe {
    //     assert!(DISK.is_none());
    //     DISK = Some(device);
    // }
    Box::leak(Box::new(device))
}

pub fn initialize_gpu(
    config: Volatile<GeneralDeviceConfig>,
    bars: &[AllocatedRange; 6],
) -> &'static VirtioGpu {
    let caps = extract_capabilities(config, bars);
    let mut virtio_gpu = VirtioGpu::new(caps);
    virtio_gpu.demo();
    Box::leak(Box::new(virtio_gpu))
}

pub fn initialize_input(
    config: Volatile<GeneralDeviceConfig>,
    bars: &[AllocatedRange; 6],
) -> &'static VirtioInput {
    let caps = extract_capabilities(config, bars);
    let mut virtio_input = VirtioInput::new(caps);
    virtio_input.demo();
    Box::leak(Box::new(virtio_input))
}

pub fn initialize_net(
    config: Volatile<GeneralDeviceConfig>,
    bars: &[AllocatedRange; 6],
) -> &'static VirtioNet {
    let caps = extract_capabilities(config, bars);
    let mut virtio_net = VirtioNet::new(caps);
    virtio_net.demo();
    Box::leak(Box::new(virtio_net))
}

fn extract_capabilities<T>(
    config: Volatile<GeneralDeviceConfig>,
    bars: &[AllocatedRange; 6],
) -> Capabilities<T> {
    let mut common = None;
    let mut notify = None;
    let mut isr = None;
    let mut device = None;
    for cap in config.walk_capabilities() {
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
            } else if cap.cfg_type == VIRTIO_PCI_CAP_ISR_CFG {
                isr = Some(unsafe { Volatile::<_, Readonly>::new(address as *mut u8) });
            } else if cap.cfg_type == VIRTIO_PCI_CAP_DEVICE_CFG {
                assert!(device.is_none());
                device = Some(unsafe { Volatile::new(address as *mut T) });
            }
        }
    }
    Capabilities {
        common: common.unwrap(),
        notify: notify.unwrap(),
        isr: isr.unwrap(),
        device: device.unwrap(),
    }
}
