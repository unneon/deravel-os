use crate::sync::Mutex;
use alloc::boxed::Box;
use core::alloc::{AllocError, Allocator, Layout};
use core::ops::Range;
use core::ptr::NonNull;

pub struct BuddyAllocator<A: Allocator> {
    root: Node<A>,
    range: Range<usize>,
    alloc: A,
}

pub struct BuddyMemoryAllocator<A: Allocator>(BuddyAllocator<A>);

struct Node<A: Allocator> {
    max_available: usize,
    children: Option<Box<(Node<A>, Node<A>), A>>,
}

impl<A: Allocator + Copy> BuddyAllocator<A> {
    pub fn new(range: Range<usize>, alloc: A) -> BuddyAllocator<A> {
        let size = range.end - range.start;
        let node_size = size.next_power_of_two();
        BuddyAllocator {
            root: Node::new(size, node_size, alloc),
            range,
            alloc,
        }
    }

    pub fn alloc(&mut self, req_size: usize) -> Result<usize, AllocError> {
        let unoffset_ptr = self.root.alloc(
            req_size,
            (self.range.end - self.range.start).next_power_of_two(),
            self.alloc,
        )?;
        let ptr = self.range.start + unoffset_ptr;
        assert!(ptr >= self.range.start);
        assert!(ptr + req_size <= self.range.end);
        Ok(ptr)
    }

    pub fn dealloc(&mut self, ptr: usize, size: usize) {
        self.root.dealloc(
            ptr - self.range.start,
            size,
            (self.range.end - self.range.start).next_power_of_two(),
        );
    }
}

impl<A: Allocator + Copy> BuddyMemoryAllocator<A> {
    pub unsafe fn new<T>(range: *mut [T], alloc: A) -> BuddyMemoryAllocator<A> {
        let start = range.as_mut_ptr() as usize;
        let size = range.len() * size_of::<T>();
        BuddyMemoryAllocator(BuddyAllocator::new(start..start + size, alloc))
    }
}

impl<A: Allocator + Copy> Node<A> {
    pub fn new(size: usize, node_size: usize, alloc: A) -> Node<A> {
        if size == 0 {
            Node {
                max_available: 0,
                children: None,
            }
        } else if size == node_size {
            Node {
                max_available: node_size,
                children: None,
            }
        } else {
            debug_assert!(size < node_size);
            let left_size = node_size / 2;
            let right_size = size.saturating_sub(left_size);
            Node {
                max_available: left_size,
                children: Some(Box::new_in(
                    (
                        Node::new(left_size, node_size / 2, alloc),
                        Node::new(right_size, node_size / 2, alloc),
                    ),
                    alloc,
                )),
            }
        }
    }

    pub fn alloc(
        &mut self,
        req_size: usize,
        node_size: usize,
        alloc: A,
    ) -> Result<usize, AllocError> {
        if req_size > self.max_available {
            return Err(AllocError);
        }
        if req_size > node_size / 2 {
            debug_assert_eq!(self.max_available, node_size);
            self.max_available = 0;
            return Ok(0);
        }
        let (left, right) = match &mut self.children {
            Some(children) => &mut **children,
            None => self.children.insert(Box::new_in(
                (
                    Node {
                        max_available: node_size / 2,
                        children: None,
                    },
                    Node {
                        max_available: node_size / 2,
                        children: None,
                    },
                ),
                alloc,
            )),
        };
        let ptr = if (left.max_available >= req_size && left.max_available <= right.max_available)
            || right.max_available < req_size
        {
            left.alloc(req_size, node_size / 2, alloc)?
        } else {
            right.alloc(req_size, node_size / 2, alloc)? + node_size / 2
        };
        self.max_available = left.max_available.max(right.max_available);
        Ok(ptr)
    }

    pub fn dealloc(&mut self, ptr: usize, req_size: usize, node_size: usize) {
        if ptr == 0 && req_size == node_size {
            debug_assert_eq!(self.max_available, 0);
            self.max_available = node_size;
            return;
        }
        let (left, right) = &mut **self.children.as_mut().unwrap();
        if ptr < node_size / 2 {
            left.dealloc(ptr, req_size, node_size / 2);
        } else {
            right.dealloc(ptr - node_size / 2, req_size, node_size / 2);
        }
        self.max_available = left.max_available.max(right.max_available);
    }
}

unsafe impl<A: Allocator + Copy> Allocator for Mutex<Option<BuddyMemoryAllocator<A>>> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let mut self_ = self.lock();
        let buddy = &mut self_.as_mut().unwrap().0;
        let address = buddy.alloc(layout.size().max(layout.align()))?;
        let ptr = core::ptr::slice_from_raw_parts_mut(address as *mut u8, layout.size());
        Ok(NonNull::new(ptr).unwrap())
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let mut self_ = self.lock();
        let buddy = &mut self_.as_mut().unwrap().0;
        buddy.dealloc(ptr.as_ptr() as usize, layout.size().max(layout.align()));
    }
}
