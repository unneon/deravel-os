use alloc::boxed::Box;
use core::alloc::Allocator;
use core::ops::Range;

pub struct BuddyAllocator<A: Allocator> {
    root: Node<A>,
    range: Range<usize>,
    alloc: A,
}

struct Node<A: Allocator> {
    max_available: usize,
    children: Option<Box<(Node<A>, Node<A>), A>>,
}

impl<A: Allocator + Copy> BuddyAllocator<A> {
    pub fn new(range: Range<usize>, alloc: A) -> BuddyAllocator<A> {
        BuddyAllocator {
            root: Node::new(range.end - range.start, alloc),
            range,
            alloc,
        }
    }

    pub fn alloc(&mut self, req_size: usize) -> Result<usize, ()> {
        Ok(self.range.start
            + self.root.alloc(
                req_size,
                (self.range.end - self.range.start).next_power_of_two(),
                self.alloc,
            )?)
    }

    pub fn dealloc(&mut self, ptr: usize, size: usize) {
        self.root.dealloc(
            ptr - self.range.start,
            size,
            (self.range.end - self.range.start).next_power_of_two(),
        );
    }
}

impl<A: Allocator + Copy> Node<A> {
    pub fn new(size: usize, alloc: A) -> Node<A> {
        if size.is_power_of_two() {
            Node {
                max_available: size,
                children: None,
            }
        } else {
            let left_size = size.isolate_highest_one();
            let right_size = size - left_size;
            Node {
                max_available: left_size,
                children: Some(Box::new_in(
                    (Node::new(left_size, alloc), Node::new(right_size, alloc)),
                    alloc,
                )),
            }
        }
    }

    pub fn alloc(&mut self, req_size: usize, node_size: usize, alloc: A) -> Result<usize, ()> {
        if req_size > self.max_available {
            return Err(());
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
