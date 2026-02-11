use crate::DISK;
use crate::virtio::registers::Registers;
use crate::virtio::virtio_blk::VirtioBlk;
use crate::virtio::virtio_net::VirtioNet;
use fdt::Fdt;
use log::{debug, error, info};

pub mod queue;
pub mod registers;
pub mod virtio_blk;
pub mod virtio_net;

pub fn initialize_all_virtio_mmio(device_tree: &Fdt) {
    for mmio in device_tree.find_all_nodes("/soc/virtio_mmio") {
        let region = mmio.property("reg").unwrap().value;
        let base_address =
            usize::from_be_bytes(region.iter().copied().array_chunks().next().unwrap());
        let device = unsafe { Registers::new(base_address as *mut Registers<()>) };
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
        } else if device_id == 0x2 {
            info!("found virtio-blk device {device} from vendor {vendor:#x}");
            let device = unsafe { device.with_configuration() };
            let device = VirtioBlk::new(device);
            unsafe {
                assert!(DISK.is_none());
                DISK = Some(device);
            }
        } else {
            debug!("ignoring {device} with unknown device id {device_id:#x}");
        }
    }
}
