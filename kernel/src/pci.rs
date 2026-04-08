pub mod capability;
pub mod config;

use crate::allocators::TrivialAllocator;
use crate::interrupt::register_interrupt;
use crate::pci::config::{Config, GeneralDeviceConfig};
use crate::uart::{Uart16550, Uart16550Mmio};
use crate::util::volatile::Volatile;
use crate::virtio;
use crate::virtio::blk::VirtioBlk;
use crate::virtio::gpu::VirtioGpu;
use fdt::Fdt;
use fdt::node::FdtNode;
use log::warn;

#[derive(Default)]
pub struct AllocatedRange {
    pub soc_offset: usize,
    #[allow(dead_code)]
    pub length: usize,
}

struct PciRange {
    soc_base: usize,
    pci_base: usize,
    length: usize,
}

struct PciRanges {
    io: PciRange,
    mem32: PciRange,
    mem64: PciRange,
}

pub fn initialize_all_pci(device_tree: &Fdt) -> (&'static VirtioBlk, &'static VirtioGpu) {
    let soc = device_tree.find_node("/soc").unwrap();
    let pci = device_tree.find_node("/soc/pci").unwrap();
    let pci_ranges = find_pci_ranges(&soc, &pci);
    let mut io = TrivialAllocator::new(pci_ranges.io.length);
    let mut mem32 = TrivialAllocator::new(pci_ranges.mem32.length);
    let mut mem64 = TrivialAllocator::new(pci_ranges.mem64.length);
    let region = pci.reg().unwrap().next().unwrap();
    let configs = region.starting_address as *mut Config;
    let configs = configs..configs.wrapping_byte_add(region.size.unwrap());
    let configs = unsafe { core::slice::from_mut_ptr_range(configs) };
    let mut virtio_blk_slot = None;
    let mut virtio_gpu_slot = None;
    for (config_index, config) in configs.iter_mut().enumerate() {
        if config.vendor_id == 0xFFFF {
            continue;
        }

        if config.vendor_id == 0x1B36 && config.device_id == 0x2 {
            let config = config.as_general_device().unwrap();
            let bars = allocate_all_bars(config, &pci_ranges, &mut io, &mut mem32, &mut mem64);
            config.command.write_bitor(0b111);
            let bar = unsafe { Volatile::new(bars[0].soc_offset as *mut Uart16550Mmio) };
            let mut uart = Uart16550::new(bar);
            uart.demo();
        } else if config.vendor_id == 0x1AF4 && config.device_id == 0x1041 {
            let config = config.as_general_device().unwrap();
            let bars = allocate_all_bars(config, &pci_ranges, &mut io, &mut mem32, &mut mem64);
            config.command.write_bitor(0b111);
            let virtio_net = virtio::initialize_net(config, &bars);
            let plic = pci_interrupt_to_plic(device_tree, config_index, config);
            register_interrupt(plic, virtio_net);
        } else if config.vendor_id == 0x1AF4 && config.device_id == 0x1042 {
            let config = config.as_general_device().unwrap();
            let bars = allocate_all_bars(config, &pci_ranges, &mut io, &mut mem32, &mut mem64);
            config.command.write_bitor(0b111);
            let virtio_blk = virtio::initialize_blk(config, &bars);
            let plic = pci_interrupt_to_plic(device_tree, config_index, config);
            register_interrupt(plic, virtio_blk);
            virtio_blk_slot = Some(virtio_blk);
        } else if config.vendor_id == 0x1AF4 && config.device_id == 0x1050 {
            let config = config.as_general_device().unwrap();
            let bars = allocate_all_bars(config, &pci_ranges, &mut io, &mut mem32, &mut mem64);
            config.command.write_bitor(0b111);
            let virtio_gpu = virtio::initialize_gpu(config, &bars);
            let plic = pci_interrupt_to_plic(device_tree, config_index, config);
            register_interrupt(plic, virtio_gpu);
            virtio_gpu_slot = Some(virtio_gpu);
        } else if config.vendor_id == 0x1AF4 && config.device_id == 0x1052 {
            let config = config.as_general_device().unwrap();
            let bars = allocate_all_bars(config, &pci_ranges, &mut io, &mut mem32, &mut mem64);
            config.command.write_bitor(0b111);
            let virtio_input = virtio::initialize_input(config, &bars);
            let plic = pci_interrupt_to_plic(device_tree, config_index, config);
            register_interrupt(plic, virtio_input);
        } else if config.vendor_id == 0x1B36 && config.device_id == 0x0008 {
            // TODO: Use this to scan the space more efficiently?
        } else {
            warn!(
                "unknown PCI device {:04x}:{:04x}",
                config.vendor_id, config.device_id
            );
        }
    }
    (virtio_blk_slot.unwrap(), virtio_gpu_slot.unwrap())
}

fn allocate_all_bars(
    config: &mut GeneralDeviceConfig,
    pci_ranges: &PciRanges,
    io: &mut TrivialAllocator,
    mem32: &mut TrivialAllocator,
    mem64: &mut TrivialAllocator,
) -> [AllocatedRange; 6] {
    let mut i = 0;
    let mut allocated: [AllocatedRange; 6] = Default::default();
    while i < 6 {
        let bar = &mut config.bars[i];
        let flags = bar.read();

        bar.write(0xFFFF_FFFF);
        let readback = bar.read();

        if readback == 0 {
            i += 1;
            continue;
        }

        // TODO: Figure out the correct alignment here.
        if flags & 1 == 1 {
            let length = (!(readback & !1) + 1) as usize;
            let offset = io.allocate(length, 4096);
            let pci_offset = pci_ranges.io.pci_base + offset;
            let soc_offset = pci_ranges.io.soc_base + offset;
            bar.write(pci_offset as u32 | 0x1);
            allocated[i] = AllocatedRange { soc_offset, length };
            i += 1;
        } else if flags & 0b110 == 0b000 {
            let length = (!(readback & !0b1111) + 1) as usize;
            let offset = mem32.allocate(length, 4096);
            let pci_offset = pci_ranges.mem32.pci_base + offset;
            let soc_offset = pci_ranges.mem32.soc_base + offset;
            bar.write(pci_offset as u32 | (flags & 0b1111));
            allocated[i] = AllocatedRange { soc_offset, length };
            i += 1;
        } else if flags & 0b110 == 0b100 {
            let [lo_bar, hi_bar] = config.bars.get_disjoint_mut([i, i + 1]).unwrap();
            let lo_readback = readback;

            hi_bar.write(0xFFFF_FFFF);
            let hi_readback = hi_bar.read();
            let readback = ((hi_readback as u64) << 32) | lo_readback as u64;

            let length = (!(readback & !0b1111) + 1) as usize;
            let offset = mem64.allocate(length, 4096);
            let pci_offset = pci_ranges.mem64.pci_base + offset;
            let soc_offset = pci_ranges.mem64.soc_base + offset;
            lo_bar.write(pci_offset as u32 | (flags & 0b1111));
            hi_bar.write((pci_offset >> 32) as u32);
            allocated[i] = AllocatedRange { soc_offset, length };
            i += 2;
        } else {
            panic!("unrecognized PCI BAR flags {flags:#b}")
        }
    }
    allocated
}

fn find_pci_ranges(soc: &FdtNode, pci: &FdtNode) -> PciRanges {
    // TODO: Read the DT spec and implement generic behavior.
    assert_eq!(pci.cell_sizes().address_cells, 3);
    assert_eq!(soc.cell_sizes().address_cells, 2);
    assert_eq!(pci.cell_sizes().size_cells, 2);

    let (cells, cells_leftover) = pci.property("ranges").unwrap().value.as_chunks::<4>();
    assert!(cells_leftover.is_empty());

    let triplet_cells = pci.cell_sizes().address_cells
        + soc.cell_sizes().address_cells
        + pci.cell_sizes().size_cells;
    assert!(cells.len().is_multiple_of(triplet_cells));

    let mut io = None;
    let mut mem32 = None;
    let mut mem64 = None;

    for triplet in cells.chunks(triplet_cells) {
        let (child_bus_address, rest) = triplet.split_at(pci.cell_sizes().address_cells);
        let (parent_bus_address, length) = rest.split_at(soc.cell_sizes().address_cells);
        let pci_base = cells_to_usize(&child_bus_address[1..]);
        let soc_base = cells_to_usize(parent_bus_address);
        let length = cells_to_usize(length);
        let range = PciRange {
            pci_base,
            soc_base,
            length,
        };
        if child_bus_address[0] == [1, 0, 0, 0] {
            assert!(io.is_none());
            io = Some(range);
        } else if child_bus_address[0] == [2, 0, 0, 0] {
            assert!(mem32.is_none());
            mem32 = Some(range);
        } else if child_bus_address[0] == [3, 0, 0, 0] {
            assert!(mem64.is_none());
            mem64 = Some(range);
        }
    }
    PciRanges {
        io: io.unwrap(),
        mem32: mem32.unwrap(),
        mem64: mem64.unwrap(),
    }
}

fn pci_interrupt_to_plic(
    device_tree: &Fdt,
    config_index: usize,
    config: &GeneralDeviceConfig,
) -> u32 {
    let pci = device_tree.find_node("/soc/pci").unwrap();
    let plic = device_tree.find_node("/soc/plic").unwrap();

    assert_eq!(pci.cell_sizes().address_cells, 3);
    assert_eq!(pci.interrupt_cells().unwrap(), 1);
    assert_eq!(plic.cell_sizes().address_cells, 0);
    assert_eq!(plic.interrupt_cells().unwrap(), 1);

    let address_mask = u32::from_be_bytes(
        *pci.property("interrupt-map-mask")
            .unwrap()
            .value
            .first_chunk()
            .unwrap(),
    );

    let function = config_index % 8;
    let device = config_index / 8 % 32;
    let bus = config_index / 256;
    let interrupt_address = ((bus << 16) | (device << 11) | (function << 8)) as u32 & address_mask;

    let interrupt_map = pci
        .property("interrupt-map")
        .unwrap()
        .value
        .as_chunks::<4>()
        .0
        .as_chunks::<6>()
        .0;

    for &interrupt in interrupt_map {
        let [pci_a0, _, _, pci_int, _, plic_int] = interrupt.map(u32::from_be_bytes);
        if pci_a0 == interrupt_address && pci_int == config.interrupt_pin as u32 {
            return plic_int;
        }
    }
    unreachable!()
}

fn cells_to_usize(cells: &[[u8; 4]]) -> usize {
    assert_eq!(cells.len(), 2);
    ((u32::from_be_bytes(cells[0]) as usize) << 32) + u32::from_be_bytes(cells[1]) as usize
}
