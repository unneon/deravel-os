use crate::{alloc_shared, map_shared};
use deravel_types::{Capability, SharedMemory};

pub struct Framebuffer {
    ptr: &'static mut [u32],
    width: usize,
}

impl Framebuffer {
    pub fn alloc(width: usize, height: usize) -> (Framebuffer, Capability<SharedMemory>) {
        let (ptr, cap) = alloc_shared(4 * width * height);
        let ptr =
            unsafe { core::slice::from_raw_parts_mut(ptr.as_mut_ptr() as *mut u32, ptr.len() / 4) };
        (Framebuffer { ptr, width }, cap)
    }

    pub fn map(width: usize, height: usize, cap: Capability<SharedMemory>) -> Framebuffer {
        let ptr = map_shared(cap);
        assert_eq!(ptr.len(), 4 * width * height);
        let ptr =
            unsafe { core::slice::from_raw_parts_mut(ptr.as_mut_ptr() as *mut u32, ptr.len() / 4) };
        Framebuffer { ptr, width }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8, a: u8) {
        self.row(y)[x] = bgra(r, g, b, a);
    }

    pub fn fill(&mut self, r: u8, g: u8, b: u8, a: u8) {
        self.ptr.fill(bgra(r, g, b, a));
    }

    pub fn fill_rect(
        &mut self,
        x_start: usize,
        y_start: usize,
        x_end: usize,
        y_end: usize,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    ) {
        for row in self.rect(x_start, y_start, x_end, y_end) {
            row.fill(bgra(r, g, b, a));
        }
    }

    pub fn fill_rows(&mut self, y_start: usize, y_end: usize, r: u8, g: u8, b: u8, a: u8) {
        self.rows(y_start, y_end).fill(bgra(r, g, b, a))
    }

    pub fn shift_rows(&mut self, y_from: usize, y_to: usize, count: usize) {
        self.ptr.copy_within(
            y_from * self.width..(y_from + count) * self.width,
            y_to * self.width,
        )
    }

    pub fn rect(
        &mut self,
        x_start: usize,
        y_start: usize,
        x_end: usize,
        y_end: usize,
    ) -> impl Iterator<Item = &mut [u32]> {
        let width = self.width;
        self.rows(y_start, y_end)
            .chunks_mut(width)
            .map(move |row| &mut row[x_start..x_end])
    }

    pub fn rows(&mut self, y_start: usize, y_end: usize) -> &mut [u32] {
        &mut self.ptr[y_start * self.width..y_end * self.width]
    }

    pub fn row(&mut self, y: usize) -> &mut [u32] {
        &mut self.ptr[y * self.width..][..self.width]
    }
}

fn bgra(r: u8, g: u8, b: u8, a: u8) -> u32 {
    b as u32 | ((g as u32) << 8) | ((r as u32) << 16) | ((a as u32) << 24)
}
