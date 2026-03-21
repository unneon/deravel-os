use crate::allocators::TrivialAllocator;
use crate::virtio::registers::{Mmio, ReadWrite, mmio};
use core::marker::PhantomData;
use core::mem::transmute;
use core::ops::{Deref, Range};
use core::sync::atomic::{AtomicU16, AtomicU32, Ordering};
use fdt::Fdt;
use fdt::node::FdtNode;
use fdt::standard_nodes::MemoryRegion;
use log::{debug, info};
// TODO: Some stuff here is safe only for valid PCI configuration structures.

#[repr(C, align(4096))]
#[derive(Debug)]
struct CommonConfiguration {
    common: CommonRegisters,
    data: [u8; 4096 - size_of::<CommonRegisters>()],
}

#[repr(C)]
#[derive(Debug)]
struct CommonRegisters {
    vendor_id: u16,
    device_id: u16,
    command: AtomicU16,
    status: AtomicU16,
    revision_id: u8,
    prog_if: u8,
    subclass: u8,
    class_code: u8,
    cache_line_size: u8,
    latency_timer: u8,
    header_type: u8,
    bist: u8,
}

#[repr(C, align(4096))]
#[derive(Debug)]
struct GeneralDeviceConfiguration {
    registers: GeneralDeviceRegisters,
    data: [u8; 4096 - size_of::<GeneralDeviceRegisters>()],
}

#[repr(C)]
#[derive(Debug)]
struct GeneralDeviceRegisters {
    common: CommonRegisters,
    bars: [AtomicU32; 6],
    cardbus_cis_pointer: u32,
    subsystem_vendor_id: u16,
    subsystem_id: u16,
    expansion_rom_base_address: u32,
    capabilities_pointer: u8,
    _reserved0: [u8; 3],
    _reserved1: u32,
    interrupt_line: u8,
    interrupt_pin: u8,
    min_grant: u8,
    max_latency: u8,
}

mmio! { pub Uart16550
    0 rbr_thr_dll: ReadWrite u8,
    1 ier_dlm: ReadWrite u8,
    2 iir_fcr: ReadWrite u8,
    3 lcr: ReadWrite u8,
    4 mcr: ReadWrite u8,
    5 lsr: ReadWrite u8,
    6 msr: ReadWrite u8,
    7 scr: ReadWrite u8,
}

impl CommonConfiguration {
    fn as_general_device(&self) -> Option<&GeneralDeviceConfiguration> {
        if self.common.header_type != 0x0 {
            return None;
        }
        Some(unsafe { transmute::<&CommonConfiguration, &GeneralDeviceConfiguration>(self) })
    }
}

impl Deref for CommonConfiguration {
    type Target = CommonRegisters;

    fn deref(&self) -> &CommonRegisters {
        &self.common
    }
}

impl Deref for GeneralDeviceConfiguration {
    type Target = GeneralDeviceRegisters;

    fn deref(&self) -> &GeneralDeviceRegisters {
        &self.registers
    }
}

impl Deref for GeneralDeviceRegisters {
    type Target = CommonRegisters;

    fn deref(&self) -> &CommonRegisters {
        &self.common
    }
}

pub fn initialize_all_pci(device_tree: &Fdt) {
    let soc = device_tree.find_node("/soc").unwrap();
    let pci = device_tree.find_node("/soc/pci").unwrap();
    let pci_ranges = find_pci_ranges(&soc, &pci);
    let mut io = TrivialAllocator::new(pci_ranges.io.length);
    let mut mem32 = TrivialAllocator::new(pci_ranges.mem32.length);
    let mut mem64 = TrivialAllocator::new(pci_ranges.mem64.length);
    let region = pci.reg().unwrap().next().unwrap();
    let configs = unsafe { region_as_array::<CommonConfiguration>(region) };
    for config in configs {
        if config.vendor_id == 0xFFFF {
            continue;
        }

        if config.vendor_id == 0x1B36 && config.device_id == 0x2 {
            info!("found UART 16550 over PCI");
            let config = config.as_general_device().unwrap();

            let bars = allocate_all_bars(config, &pci_ranges, &mut io, &mut mem32, &mut mem64);
            config.command.fetch_or(0b111, Ordering::SeqCst);

            let bar = Mmio(bars[0].soc_offset as *mut Uart16550, PhantomData);

            bar.ier_dlm().write(0x00);

            bar.lcr().write(0x80);

            bar.rbr_thr_dll().write(0x01);
            bar.ier_dlm().write(0x00);

            bar.lcr().write(0x03);

            bar.iir_fcr().write(0xC7);

            bar.mcr().write(0x03);

            let uart_putc = |c: u8| {
                while bar.lsr().read() & (1 << 5) == 0 {}
                bar.rbr_thr_dll().write(c);
            };
            for c in "Hello, world!\n".bytes() {
                uart_putc(c);
            }
        } else if config.vendor_id == 0x1AF4 && config.device_id == 0x1042 {
            info!("found virtio-blk over PCI");
            let config = config.as_general_device().unwrap();

            let bars = allocate_all_bars(config, &pci_ranges, &mut io, &mut mem32, &mut mem64);
            config.command.fetch_or(0b111, Ordering::SeqCst);

            for cap in walk_capabilities(config) {
                if let Some(cap) = unsafe { cap.get_vendor::<VirtioCap>() } {
                    if cap.cfg_type == VIRTIO_PCI_CAP_COMMON_CFG {
                        debug!("cap = {cap:?}");
                        let bar = Mmio(
                            (bars[cap.bar as usize].soc_offset + cap.offset as usize)
                                as *mut VirtioPciCommonCfg,
                            PhantomData::<ReadWrite>,
                        );
                        bar.device_status().write(0);
                        bar.device_feature_select().write(0);
                        debug!("device features {:#b}", bar.device_feature().read());
                    }
                }
            }
        }
    }
}

mmio! { pub VirtioPciCommonCfg
    0 device_feature_select: ReadWrite u32,
    4 device_feature: Readonly u32,
    8 driver_feature_select: ReadWrite u32,
    12 driver_feature: ReadWrite u32,
    16 config_msix_vector: ReadWrite u16,
    18 num_queues: ReadWrite u16,
    20 device_status: ReadWrite u8,
    21 config_generation: ReadWrite u8,
}

const VIRTIO_PCI_CAP_COMMON_CFG: u8 = 1;
const VIRTIO_PCI_CAP_NOTIFY_CFG: u8 = 2;
const VIRTIO_PCI_CAP_ISR_CFG: u8 = 3;
const VIRTIO_PCI_CAP_DEVICE_CFG: u8 = 4;
const VIRTIO_PCI_CAP_PCI_CFG: u8 = 5;

unsafe trait VendorPciCapability {}

#[repr(C)]
#[derive(Debug)]
struct PciCapability {
    vndr: u8,
    next: u8,
    len: u8,
}

#[repr(C)]
#[derive(Debug)]
struct VirtioCap {
    cap: PciCapability,
    cfg_type: u8,
    bar: u8,
    id: u8,
    padding: [u8; 2],
    offset: u32,
    length: u32,
}

unsafe impl VendorPciCapability for VirtioCap {}

impl PciCapability {
    unsafe fn get_vendor<T: VendorPciCapability>(&self) -> Option<&T> {
        if self.vndr == 0x09 {
            Some(unsafe { &*(self as *const Self as *const T) })
        } else {
            None
        }
    }
}

#[derive(Default)]
struct AllocatedRange {
    soc_offset: usize,
    length: usize,
}

fn allocate_all_bars(
    config: &GeneralDeviceConfiguration,
    pci_ranges: &PciRanges,
    io: &mut TrivialAllocator,
    mem32: &mut TrivialAllocator,
    mem64: &mut TrivialAllocator,
) -> [AllocatedRange; 6] {
    let mut i = 0;
    let mut allocated: [AllocatedRange; 6] = Default::default();
    while i < 6 {
        let bar = &config.bars[i];
        let flags = bar.load(Ordering::SeqCst);

        bar.store(0xFFFF_FFFF, Ordering::SeqCst);
        let readback = bar.load(Ordering::SeqCst);

        // TODO: Figure out the correct alignment here.
        if flags & 1 == 1 {
            let length = (!(readback & !1) + 1) as usize;
            let offset = io.allocate(length, 4096);
            let pci_offset = pci_ranges.io.pci_base + offset;
            let soc_offset = pci_ranges.io.soc_base + offset;
            bar.store(pci_offset as u32 | 0x1, Ordering::SeqCst);
            allocated[i] = AllocatedRange { soc_offset, length };
            i += 1;
        } else if flags & 0b110 == 0b000 {
            let length = (!(readback & !0b1111) + 1) as usize;
            let offset = mem32.allocate(length, 4096);
            let pci_offset = pci_ranges.mem32.pci_base + offset;
            let soc_offset = pci_ranges.mem32.soc_base + offset;
            bar.store(pci_offset as u32 | (flags & 0b1111), Ordering::SeqCst);
            allocated[i] = AllocatedRange { soc_offset, length };
            i += 1;
        } else if flags & 0b110 == 0b100 {
            let lo_bar = bar;
            let hi_bar = &config.bars[i + 1];
            let lo_readback = readback;

            hi_bar.store(0xFFFF_FFFF, Ordering::SeqCst);
            let hi_readback = hi_bar.load(Ordering::SeqCst);
            let readback = ((hi_readback as u64) << 32) | lo_readback as u64;

            let length = (!(readback & !0b1111) + 1) as usize;
            let offset = mem64.allocate(length, 4096);
            let pci_offset = pci_ranges.mem64.pci_base + offset;
            let soc_offset = pci_ranges.mem64.soc_base + offset;
            lo_bar.store(pci_offset as u32 | (flags & 0b1111), Ordering::SeqCst);
            hi_bar.store((pci_offset >> 32) as u32, Ordering::SeqCst);
            allocated[i] = AllocatedRange { soc_offset, length };
            i += 2;
        } else {
            panic!("unrecognized PCI BAR flags {flags:#b}")
        }
    }
    allocated
}

fn walk_capabilities(config: &GeneralDeviceConfiguration) -> impl Iterator<Item = &PciCapability> {
    assert_ne!(config.status.load(Ordering::SeqCst) & (1 << 4), 0);
    let config_space = config as *const GeneralDeviceConfiguration;
    let mut pointer = config.registers.capabilities_pointer & !0x3;
    core::iter::from_fn(move || {
        if pointer == 0 {
            return None;
        }
        let cap = unsafe { &*(config_space.byte_add(pointer as usize) as *const PciCapability) };
        pointer = cap.next;
        Some(cap)
    })
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

fn cells_to_usize(cells: &[[u8; 4]]) -> usize {
    assert_eq!(cells.len(), 2);
    ((u32::from_be_bytes(cells[0]) as usize) << 32) + u32::from_be_bytes(cells[1]) as usize
}

unsafe fn region_as_array<T>(region: MemoryRegion) -> &'static [T] {
    let start = region.starting_address as *const T;
    let end = unsafe { start.byte_add(region.size.unwrap()) };
    unsafe { core::slice::from_ptr_range(start..end) }
}
