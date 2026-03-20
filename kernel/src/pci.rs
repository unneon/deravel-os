use crate::virtio::registers::{Mmio, mmio};
use core::marker::PhantomData;
use core::mem::transmute;
use core::ops::Deref;
use core::sync::atomic::{AtomicU16, AtomicU32, Ordering};
use fdt::Fdt;
use fdt::node::FdtNode;
use fdt::standard_nodes::MemoryRegion;
use log::info;

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
    common: CommonRegisters,
    bar0: AtomicU32,
    bar1: AtomicU32,
    bar2: AtomicU32,
    bar3: AtomicU32,
    bar4: AtomicU32,
    bar5: AtomicU32,
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

pub fn initialize_all_pci(device_tree: &Fdt) {
    let soc = device_tree.find_node("/soc").unwrap();
    let pci = device_tree.find_node("/soc/pci").unwrap();
    let pci_io_base = find_pci_io_base(&soc, &pci);
    let region = pci.reg().unwrap().next().unwrap();
    let configs = unsafe { region_as_array::<CommonConfiguration>(region) };
    for config in configs {
        if config.vendor_id == 0xFFFF {
            continue;
        }

        if config.vendor_id == 0x1B36 && config.device_id == 0x2 {
            info!("found UART 16550 over PCI");
            let config = config.as_general_device().unwrap();

            let bar = if config.bar0.load(Ordering::SeqCst) & 1 == 1 {
                // TODO: Make an allocator for PCI IO space.
                config.bar0.store(0x1, Ordering::SeqCst);
                pci_io_base
            } else {
                unimplemented!()
            };

            config.common.command.fetch_or(0b111, Ordering::SeqCst);

            let bar = Mmio(bar as *mut Uart16550, PhantomData);

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
        }
    }
}

fn find_pci_io_base(soc: &FdtNode, pci: &FdtNode) -> usize {
    let triplet_cells = pci.cell_sizes().address_cells
        + soc.cell_sizes().address_cells
        + pci.cell_sizes().size_cells;

    let (cells, cells_leftover) = pci.property("ranges").unwrap().value.as_chunks::<4>();
    assert!(cells_leftover.is_empty());

    for triplet in cells.chunks(triplet_cells) {
        let (child_bus_address, rest) = triplet.split_at(pci.cell_sizes().address_cells);
        let (parent_bus_address, _length) = rest.split_at(soc.cell_sizes().address_cells);
        if child_bus_address[0][0] == 1 {
            assert_eq!(child_bus_address[1..], [[0; 4], [0; 4]]);
            assert_eq!(parent_bus_address.len(), 2);
            assert_eq!(parent_bus_address[0], [0; 4]);
            return u32::from_be_bytes(parent_bus_address[1]) as usize;
        }
    }
    unimplemented!()
}

unsafe fn region_as_array<T>(region: MemoryRegion) -> &'static [T] {
    let start = region.starting_address as *const T;
    let end = unsafe { start.byte_add(region.size.unwrap()) };
    unsafe { core::slice::from_ptr_range(start..end) }
}
