use crate::DISK;
use crate::pci::{
    AllocatedRange, GeneralDeviceConfig, PciCapability, VendorPciCapability, walk_capabilities,
};
use crate::util::volatile::{Volatile, volatile_struct};
use crate::virtio::blk::VirtioBlk;
use crate::virtio::net::VirtioNet;

pub mod blk;
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

pub fn initialize_net(config: Volatile<GeneralDeviceConfig>, bars: &[AllocatedRange; 6]) {
    let configs = extract_configs(config, bars);
    let mut virtio_net = VirtioNet::new(configs.common, configs.notify, configs.device);
    virtio_net.demo();
}

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
        }
    }
    Configs {
        common: common.unwrap(),
        notify: notify.unwrap(),
        device: device.unwrap(),
    }
}
