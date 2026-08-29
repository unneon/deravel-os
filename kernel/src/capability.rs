use crate::heap::MutAllocator;
use crate::heap::bitmap::BitmapAllocator;
use crate::page::{PageTable, virt_to_phys};
use crate::process::PROCESS_COUNT;
use crate::sync::Mutex;
use crate::virtual_memory::VirtualMemoryRawMapping;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::marker::PhantomData;
use core::ops::Range;
use core::sync::atomic::Ordering;
use deravel_types::*;

pub trait Handler<T> {
    fn call_method(&'static self, method: usize, args: &[u8], sender: ProcessId) -> Vec<u8>;

    fn map_stream(&self, stream: usize) -> &'static UntypedRingBuffer;

    fn shared_memory_map(
        &self,
        virt: usize,
        page_table: &mut PageTable,
        vmms: &mut Vec<(Range<usize>, &'static (dyn VirtualMemoryRawMapping + Sync))>,
    );

    fn shared_memory_size(&self) -> usize;
}

pub trait RawHandler {
    fn call_method(&'static self, method: usize, args: &[u8], sender: ProcessId) -> Vec<u8>;

    fn map_stream(&self, stream: usize) -> &'static UntypedRingBuffer;

    fn shared_memory_map(
        &self,
        virt: usize,
        page_table: &mut PageTable,
        vmms: &mut Vec<(Range<usize>, &'static (dyn VirtualMemoryRawMapping + Sync))>,
    );

    fn shared_memory_size(&self) -> usize;
}

#[repr(transparent)]
struct TypedHandler<T, H: ?Sized>(PhantomData<T>, H);

const SLOT_LAYOUT: Layout = Layout::from_size_align(1, 1).ok().unwrap();

static CAPABILITY_PAGES: [CapabilityPage; PROCESS_COUNT + 1] =
    [const { CapabilityPage([const { CapabilityCertificate::new() }; _]) }; _];

static ALLOCATOR: Mutex<BitmapAllocator<[usize; CAPABILITIES_PER_PAGE / usize::BITS as usize]>> =
    Mutex::new(BitmapAllocator::new(0..CAPABILITIES_PER_PAGE, [0; _]));

static HANDLERS: [Mutex<Option<&'static (dyn RawHandler + Sync)>>;
    PAGE_SIZE / size_of::<CapabilityCertificateValue>()] = [const { Mutex::new(None) }; _];

impl<T, H: Handler<T> + ?Sized> RawHandler for TypedHandler<T, H> {
    fn call_method(&'static self, method: usize, args: &[u8], sender: ProcessId) -> Vec<u8> {
        self.1.call_method(method, args, sender)
    }

    fn map_stream(&self, stream: usize) -> &'static UntypedRingBuffer {
        self.1.map_stream(stream)
    }

    fn shared_memory_map(
        &self,
        virt: usize,
        page_table: &mut PageTable,
        vmms: &mut Vec<(Range<usize>, &'static (dyn VirtualMemoryRawMapping + Sync))>,
    ) {
        self.1.shared_memory_map(virt, page_table, vmms)
    }

    fn shared_memory_size(&self) -> usize {
        self.1.shared_memory_size()
    }
}

pub fn grant_kernel_capability<T: 'static + Sync>(
    grantee: ProcessId,
    handler: &'static (impl Handler<T> + Sync),
) -> Capability<T> {
    let cap = reserve_kernel_capability(handler);
    // TODO: Race condition, PID 0 can use the capability.
    kernel_capability_page().0[cap.local_index()].store(
        CapabilityCertificateValue::granted(grantee),
        Ordering::Relaxed,
    );
    cap
}

pub fn reserve_kernel_capability<T: 'static + Sync, H: Handler<T> + Sync>(
    handler: &'static H,
) -> Capability<T> {
    let local_index = ALLOCATOR.lock().alloc(SLOT_LAYOUT).unwrap();
    *HANDLERS[local_index].lock() =
        Some(unsafe { core::mem::transmute::<&'static H, &'static TypedHandler<T, H>>(handler) });
    unsafe { Capability::new(RawCapability::new(Actor::Kernel, local_index)) }
}

pub fn get_handler(local_index: usize) -> &'static (dyn RawHandler + Sync) {
    *HANDLERS[local_index].lock().as_ref().unwrap()
}

pub fn capability_certificate(cap: RawCapability) -> &'static CapabilityCertificate {
    match cap.certifier() {
        Actor::Userspace(pid) => &capability_page(pid).0[cap.local_index()],
        Actor::Kernel => &kernel_capability_page().0[cap.local_index()],
    }
}

fn capability_page(pid: ProcessId) -> &'static CapabilityPage {
    &CAPABILITY_PAGES[pid.as_u16() as usize]
}

fn kernel_capability_page() -> &'static CapabilityPage {
    &CAPABILITY_PAGES[0]
}

pub fn capability_pages_physical_address() -> usize {
    virt_to_phys(&CAPABILITY_PAGES as *const _ as usize)
}
