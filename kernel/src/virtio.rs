use crate::DISK;
use crate::pci::{
    AllocatedRange, GeneralDeviceConfig, PciCapability, VendorPciCapability, walk_capabilities,
};
use crate::util::volatile::{Volatile, volatile_struct};
use crate::virtio::registers::Registers;
use crate::virtio::virtio_blk::{VirtioBlk, VirtioBlkConfig};
use crate::virtio::virtio_net::VirtioNet;
use fdt::Fdt;
use log::{debug, error, info};

pub mod queue;
pub mod registers;
pub mod virtio_blk;
pub mod virtio_net;

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
    queue_notif_config_data: Readonly u16,
    queue_reset: ReadWrite u16,

    admin_queue_index: Readonly u16,
    admin_queue_num: Readonly u16,
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

const VIRTIO_PCI_CAP_COMMON_CFG: u8 = 1;
const VIRTIO_PCI_CAP_DEVICE_CFG: u8 = 4;

unsafe impl VendorPciCapability for VirtioPciCapability {}

pub fn initialize_all_virtio_mmio(device_tree: &Fdt) {
    for mmio in device_tree.find_all_nodes("/soc/virtio_mmio") {
        let region = mmio.reg().unwrap().next().unwrap();
        let device = unsafe { Registers::new(region.starting_address as *mut Registers<()>) };
        if device.magic_value().read() != 0x74726976 {
            error!("{device} magic value is not 0x74726976");
            continue;
        }
        if device.version().read() != 0x1 {
            error!("{device} version is not 0x1");
            continue;
        }

        let device_id = device.device_id().read();
        if device_id == 0x0 {
            continue;
        }

        let vendor = device.vendor_id().read();
        if device_id == 0x1 {
            info!("found virtio-net device {device} from vendor {vendor:#x}");
            let device = unsafe { device.with_configuration() };
            let mut virtio_net = VirtioNet::new(device);
            virtio_net.demo();
        } else {
            debug!("ignoring {device} with unknown device id {device_id:#x}");
        }
    }
}

pub fn initialize_blk(config: Volatile<GeneralDeviceConfig>, bars: &[AllocatedRange; 6]) {
    let mut common = None;
    let mut device = None;
    for cap in walk_capabilities(config) {
        if let Some(cap) = unsafe { cap.get_vendor::<VirtioPciCapability>() } {
            let address = bars[cap.bar as usize].soc_offset + cap.offset as usize;
            if cap.cfg_type == VIRTIO_PCI_CAP_COMMON_CFG {
                common = Some(unsafe { Volatile::new(address as *mut VirtioCommonConfig) });
            } else if cap.cfg_type == VIRTIO_PCI_CAP_DEVICE_CFG {
                device = Some(unsafe { Volatile::new(address as *mut VirtioBlkConfig) });
            }
        }
    }
    let device = VirtioBlk::new(common.unwrap(), device.unwrap());
    unsafe {
        assert!(DISK.is_none());
        DISK = Some(device);
    }
}
