use crate::{alloc_shared, map_shared};
use deravel_types::{Capability, PAGE_SIZE, SharedMemory};

pub struct Framebuffer {
    ptr: &'static mut [u32],
    width: usize,
    height: usize,
}

impl Framebuffer {
    pub fn alloc(width: usize, height: usize) -> (Framebuffer, Capability<SharedMemory>) {
        let (ptr, cap) = alloc_shared(4 * width * height);
        let ptr =
            unsafe { core::slice::from_raw_parts_mut(ptr.as_mut_ptr() as *mut u32, ptr.len() / 4) };
        (Framebuffer { ptr, width, height }, cap)
    }

    pub fn map(width: usize, height: usize, cap: Capability<SharedMemory>) -> Framebuffer {
        let ptr = map_shared(cap);
        assert_eq!(ptr.len(), (4 * width * height).next_multiple_of(PAGE_SIZE));
        let ptr =
            unsafe { core::slice::from_raw_parts_mut(ptr.as_mut_ptr() as *mut u32, ptr.len() / 4) };
        Framebuffer { ptr, width, height }
    }

    #[track_caller]
    pub fn set_pixel(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8, a: u8) {
        assert!(x < self.width);
        assert!(y < self.height);
        self.row_mut(y)[x] = bgra(r, g, b, a);
    }

    pub fn fill(&mut self, r: u8, g: u8, b: u8, a: u8) {
        self.ptr.fill(bgra(r, g, b, a));
    }

    #[track_caller]
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
        assert!(x_start <= x_end);
        assert!(x_end <= self.width);
        assert!(y_start <= y_end);
        assert!(y_end <= self.height);
        for row in self.rect(x_start, y_start, x_end, y_end) {
            row.fill(bgra(r, g, b, a));
        }
    }

    #[track_caller]
    pub fn fill_rows(&mut self, y_start: usize, y_end: usize, r: u8, g: u8, b: u8, a: u8) {
        assert!(y_start <= y_end);
        assert!(y_end <= self.height);
        self.rows(y_start, y_end).fill(bgra(r, g, b, a))
    }

    #[track_caller]
    pub fn copy_rect(&mut self, offset_x: usize, offset_y: usize, rect: &Framebuffer) {
        assert!(offset_x <= self.width);
        assert!(offset_y <= self.height);
        assert!(offset_x + rect.width <= self.width);
        assert!(offset_y + rect.height <= self.height);
        for rect_y in 0..rect.height {
            let y = offset_y + rect_y;
            self.row_mut(y)[offset_x..][..rect.width].copy_from_slice(rect.row(rect_y))
        }
    }

    #[track_caller]
    pub fn shift_rows(&mut self, y_from: usize, y_to: usize, count: usize) {
        assert!(y_from + count <= self.height);
        assert!(y_to + count <= self.height);
        self.ptr.copy_within(
            y_from * self.width..(y_from + count) * self.width,
            y_to * self.width,
        )
    }

    #[track_caller]
    pub fn rect(
        &mut self,
        x_start: usize,
        y_start: usize,
        x_end: usize,
        y_end: usize,
    ) -> impl Iterator<Item = &mut [u32]> {
        assert!(x_start <= x_end);
        assert!(x_end <= self.width);
        assert!(y_start <= y_end);
        assert!(y_end <= self.height);
        let width = self.width;
        self.rows(y_start, y_end)
            .chunks_mut(width)
            .map(move |row| &mut row[x_start..x_end])
    }

    #[track_caller]
    pub fn rows(&mut self, y_start: usize, y_end: usize) -> &mut [u32] {
        assert!(y_start <= y_end);
        assert!(y_end <= self.height);
        &mut self.ptr[y_start * self.width..y_end * self.width]
    }

    #[track_caller]
    pub fn row(&self, y: usize) -> &[u32] {
        assert!(y < self.height);
        &self.ptr[y * self.width..][..self.width]
    }

    #[track_caller]
    pub fn row_mut(&mut self, y: usize) -> &mut [u32] {
        assert!(y < self.height);
        &mut self.ptr[y * self.width..][..self.width]
    }
}

fn bgra(r: u8, g: u8, b: u8, a: u8) -> u32 {
    b as u32 | ((g as u32) << 8) | ((r as u32) << 16) | ((a as u32) << 24)
}
