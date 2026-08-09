use crate::{alloc_shared, map_shared};
use deravel_types::{Capability, PageAligned, SharedMemory};

pub struct Framebuffer {
    ptr: &'static mut [u32],
    width: usize,
    height: usize,
}

impl Framebuffer {
    pub fn alloc(width: usize, height: usize) -> (Framebuffer, Capability<SharedMemory>) {
        let (ptr, cap) = alloc_shared(4 * width * height);
        let ptr = unsafe { &mut *PageAligned::cast_mut(ptr) };
        (Framebuffer { ptr, width, height }, cap)
    }

    pub fn map(width: usize, height: usize, cap: Capability<SharedMemory>) -> Framebuffer {
        let ptr = unsafe { &mut *PageAligned::cast_mut(map_shared(cap)) };
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

    pub fn copy_from_rect(&mut self, offset_x: isize, offset_y: isize, rect: &Framebuffer) {
        let min_rect_x = (-offset_x).max(0);
        let min_rect_y = (-offset_y).max(0);
        let max_rect_x = (self.width as isize - offset_x).min(rect.width as isize);
        let max_rect_y = (self.height as isize - offset_y).min(rect.height as isize);
        for rect_y in min_rect_y..max_rect_y {
            self.row_mut((offset_y + rect_y) as usize)
                [(offset_x + min_rect_x) as usize..(offset_x + max_rect_x) as usize]
                .copy_from_slice(
                    &rect.row(rect_y as usize)[min_rect_x as usize..max_rect_x as usize],
                );
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
