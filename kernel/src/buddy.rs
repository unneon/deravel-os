use crate::sync::Mutex;
use alloc::alloc::Global;
use alloc::boxed::Box;
use core::alloc::{AllocError, Allocator, Layout};
use core::ops::{DerefMut, Range};
use core::ptr::NonNull;

pub struct BuddyAllocator<A: Allocator = Global> {
    root: Node<A>,
    root_node_size: usize,
    range: Range<usize>,
    alloc: A,
}

pub struct BuddyMemoryAllocator<A: Allocator>(BuddyAllocator<A>);

struct Node<A: Allocator> {
    max_available: usize,
    children: Option<Box<(Node<A>, Node<A>), A>>,
}

impl BuddyAllocator {
    pub fn new(range: Range<usize>) -> BuddyAllocator {
        BuddyAllocator::new_in(range, Global)
    }
}

impl<A: Allocator + Copy> BuddyAllocator<A> {
    pub fn new_in(range: Range<usize>, alloc: A) -> BuddyAllocator<A> {
        let size = range.end - range.start;
        let node_size = size.next_power_of_two();
        BuddyAllocator {
            root: Node::new(size, node_size, alloc),
            root_node_size: node_size,
            range,
            alloc,
        }
    }

    pub fn alloc(&mut self, layout: Layout) -> Result<usize, AllocError> {
        // TODO: Handle alignment with respect to containing range.
        let req_size = layout.size().max(layout.align());
        let unoffset_ptr = self.root.alloc(req_size, self.root_node_size, self.alloc)?;
        let ptr = self.range.start + unoffset_ptr;
        assert!(ptr >= self.range.start);
        assert!(ptr + req_size <= self.range.end);
        Ok(ptr)
    }

    pub fn dealloc(&mut self, ptr: usize, layout: Layout) {
        let req_size = layout.size().max(layout.align());
        self.root
            .dealloc(ptr - self.range.start, req_size, self.root_node_size);
    }

    pub fn reserve_range(&mut self, range: Range<usize>) {
        debug_assert!(range.start >= self.range.start);
        debug_assert!(range.end <= self.range.end);
        let range = range.start - self.range.start..range.end - self.range.start;
        self.root
            .reserve_range(range, self.root_node_size, self.alloc);
    }
}

impl<A: Allocator + Copy> BuddyMemoryAllocator<A> {
    pub unsafe fn new(range: Range<*mut u8>, alloc: A) -> BuddyMemoryAllocator<A> {
        let range = range.start as usize..range.end as usize;
        BuddyMemoryAllocator(BuddyAllocator::new_in(range, alloc))
    }

    pub fn reserve_range(&mut self, range: Range<*const u8>) {
        let range = range.start as usize..range.end as usize;
        self.0.reserve_range(range.clone());
    }

    pub fn allocate_mut(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let address = self.0.alloc(layout)?;
        let ptr = core::ptr::slice_from_raw_parts_mut(address as *mut u8, layout.size());
        Ok(NonNull::new(ptr).unwrap())
    }

    pub unsafe fn deallocate_mut(&mut self, ptr: NonNull<u8>, layout: Layout) {
        self.0.dealloc(ptr.as_ptr() as usize, layout);
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
            assert!(size < node_size);
            let left_size = (node_size / 2).min(size);
            let right_size = size.saturating_sub(node_size / 2);
            let mut node = Node {
                max_available: 0,
                children: Some(Box::new_in(
                    (
                        Node::new(left_size, node_size / 2, alloc),
                        Node::new(right_size, node_size / 2, alloc),
                    ),
                    alloc,
                )),
            };
            node.update(node_size);
            node
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
            assert_eq!(self.max_available, node_size);
            self.max_available = 0;
            return Ok(0);
        }
        let (left, right) = get_children(&mut self.children, node_size, alloc);
        let ptr = if (left.max_available >= req_size && left.max_available <= right.max_available)
            || right.max_available < req_size
        {
            assert!(left.max_available >= req_size);
            left.alloc(req_size, node_size / 2, alloc)?
        } else {
            assert!(right.max_available >= req_size);
            right.alloc(req_size, node_size / 2, alloc)? + node_size / 2
        };
        self.update(node_size);
        Ok(ptr)
    }

    pub fn dealloc(&mut self, ptr: usize, req_size: usize, node_size: usize) {
        if ptr == 0 && req_size > node_size / 2 {
            assert_eq!(self.max_available, 0);
            self.max_available = node_size;
            return;
        }
        let (left, right) = self.children.as_deref_mut().unwrap();
        if ptr < node_size / 2 {
            left.dealloc(ptr, req_size, node_size / 2);
        } else {
            right.dealloc(ptr - node_size / 2, req_size, node_size / 2);
        }
        self.update(node_size);
    }

    pub fn reserve_range(&mut self, range: Range<usize>, node_size: usize, alloc: A) {
        if range == (0..node_size) {
            assert_eq!(self.max_available, node_size);
            self.max_available = 0;
        } else {
            let (left, right) = get_children(&mut self.children, node_size, alloc);
            if range.start < node_size / 2 {
                left.reserve_range(
                    range.start..range.end.min(node_size / 2),
                    node_size / 2,
                    alloc,
                );
            }
            if range.end > node_size / 2 {
                right.reserve_range(
                    range.start.max(node_size / 2) - node_size / 2..range.end - node_size / 2,
                    node_size / 2,
                    alloc,
                );
            }
            self.update(node_size);
        }
    }

    fn update(&mut self, node_size: usize) {
        if let Some((left, right)) = self.children.as_deref_mut() {
            if left.max_available == node_size / 2 && right.max_available == node_size / 2 {
                self.max_available = node_size;
            } else {
                self.max_available = left.max_available.max(right.max_available);
            }
        }
    }
}

unsafe impl<A: Allocator + Copy> Allocator for Mutex<Option<BuddyMemoryAllocator<A>>> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.lock().as_mut().unwrap().allocate_mut(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        unsafe { self.lock().as_mut().unwrap().deallocate_mut(ptr, layout) }
    }
}

fn get_children<A: Allocator>(
    children: &mut Option<Box<(Node<A>, Node<A>), A>>,
    node_size: usize,
    alloc: A,
) -> &mut (Node<A>, Node<A>) {
    match children {
        Some(children) => children.deref_mut(),
        None => children.insert(Box::new_in(
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
    }
}
