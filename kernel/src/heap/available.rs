use crate::page::phys_to_virt;
use alloc::vec::Vec;
use core::iter::once;
use core::ops::Range;
use fdt::Fdt;

pub fn collect_available(dt: &Fdt) -> Vec<Range<*mut u8>> {
    dt.memory()
        .regions()
        .map(|reg| {
            let start = phys_to_virt(reg.starting_address as *mut u8);
            let end = start.wrapping_byte_add(reg.size.unwrap());
            start..end
        })
        .collect()
}

pub fn collect_reserved(dt: &Fdt, dt_ptr: *const u8) -> impl Iterator<Item = Range<*const u8>> {
    reserved_ranges_from_dt(dt)
        .chain(once(reserved_kernel_range()))
        .chain(once(reserved_dt_memory(dt, dt_ptr)))
}

fn reserved_ranges_from_dt(dt: &Fdt) -> impl Iterator<Item = Range<*const u8>> {
    dt.find_node("/reserved-memory")
        .unwrap()
        .children()
        .flat_map(|reserved| {
            reserved.reg().into_iter().flatten().map(|reg| {
                let start = phys_to_virt(reg.starting_address);
                let end = start.wrapping_byte_add(reg.size.unwrap());
                start..end
            })
        })
}

fn reserved_kernel_range() -> Range<*const u8> {
    unsafe extern "C" {
        static image_start: u8;
        static image_end: u8;
    }
    &raw const image_start..&raw const image_end
}

fn reserved_dt_memory(dt: &Fdt, dt_ptr: *const u8) -> Range<*const u8> {
    dt_ptr..dt_ptr.wrapping_byte_add(dt.total_size())
}
