use crate::capability::{Handler, capability_certificate};
use crate::elf::Elf;
use crate::page::PageTable;
use crate::process::{Process, reserve_process};
use crate::virtual_memory::VirtualMemoryRawMapping;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;
use core::sync::atomic::Ordering;
use deravel_types::{Actor, CapabilityCertificateValue, ProcessArgs, ProcessId, ProcessTag};

impl<T: ProcessTag, U: AsRef<[u8]>> Handler<T::Spawner> for &'static Elf<T, U> {
    fn call_method(&self, _: usize, args: &[u8], sender: ProcessId) -> Vec<u8> {
        let reserve = reserve_process(self);
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

    fn map_stream(&self, _: usize, _: &mut Process) -> (*const (), usize) {
        unreachable!()
    }

    fn shared_memory_map(
        &self,
        _: usize,
        _: &mut PageTable,
        _: &mut Vec<(Range<usize>, &'static (dyn VirtualMemoryRawMapping + Sync))>,
    ) {
        unreachable!()
    }

    fn shared_memory_size(&self) -> usize {
        unreachable!()
    }
}
