use crate::capability::{Handler, capability_certificate};
use crate::page::TopPageTable;
use crate::process::reserve_process;
use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::Range;
use core::sync::atomic::Ordering;
use deravel_types::{
    Actor, CapabilityCertificateValue, ProcessArgs, ProcessId, ProcessTag, SharedMemory,
    UntypedRingBuffer,
};

pub struct ProcessSpawnerService<T> {
    elf: &'static [u8],
    _phantom: PhantomData<T>,
}

impl<T: ProcessTag> ProcessSpawnerService<T> {
    pub fn new(elf: &'static [u8]) -> ProcessSpawnerService<T> {
        ProcessSpawnerService {
            elf,
            _phantom: PhantomData,
        }
    }
}

impl<T: ProcessTag> Handler<T::Spawner> for ProcessSpawnerService<T> {
    fn call_method(&self, _: usize, args: &[u8], sender: ProcessId) -> Vec<u8> {
        let reserve = reserve_process::<T>(self.elf);
        let export = reserve.export;
        capability_certificate(*export).store(
            CapabilityCertificateValue::granted(sender),
            Ordering::Relaxed,
        );
        let args: <T as ProcessTag>::Args = postcard::from_bytes(args).unwrap();
        args.for_all(|cap| {
            assert_eq!(cap.certifier(), Actor::Userspace(sender));
            let slot = capability_certificate(cap);
            let before = slot.load(Ordering::Relaxed);
            let after = before.replace_recipient(reserve.id);
            slot.store(after, Ordering::Relaxed);
        });
        reserve.spawn_with_ready_caps(args);
        let mut buf = vec![0; 4096];
        let buf_len = postcard::to_slice(&export, &mut buf).unwrap().len();
        buf.resize(buf_len, 0);
        buf
    }

    fn map_stream(&self, _: usize) -> &'static UntypedRingBuffer {
        unreachable!()
    }

    fn shared_memory_map(
        &self,
        _: usize,
        _: &mut TopPageTable,
        _: &mut Vec<(Range<usize>, &'static (dyn Handler<SharedMemory> + Sync))>,
    ) {
        unreachable!()
    }

    fn shared_memory_size(&self) -> usize {
        unreachable!()
    }

    fn virtual_memory_load(&self, _: usize, _: usize, _: &mut TopPageTable) {
        unreachable!()
    }
}
