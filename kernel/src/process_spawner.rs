use crate::capability::{Handler, capability_page};
use crate::process::reserve_process;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::sync::atomic::Ordering;
use deravel_types::{
    Actor, CapabilityCertificateUnpacked, CapabilityCertificateValue, ProcessArgs, ProcessId,
    ProcessTag, UntypedRingBuffer,
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
        capability_page(reserve.id).0[0].store(
            CapabilityCertificateValue::granted(sender),
            Ordering::Relaxed,
        );
        let args: <T as ProcessTag>::Args = serde_json::from_slice(args).unwrap();
        args.for_all(|cap| {
            assert_eq!(cap.certifier(), Actor::from(sender));
            let slot = &capability_page(sender).0[cap.local_index()];
            let preforward = slot.load(Ordering::Relaxed).unpack();
            match preforward {
                CapabilityCertificateUnpacked::Granted {
                    grantee: Actor::Kernel,
                } => slot.store(
                    CapabilityCertificateValue::granted(reserve.id),
                    Ordering::Relaxed,
                ),
                CapabilityCertificateUnpacked::Forwarded {
                    forwardee: Actor::Kernel,
                    inner,
                } => slot.store(
                    CapabilityCertificateValue::forwarded(reserve.id.into(), inner),
                    Ordering::Relaxed,
                ),
                _ => unreachable!("{preforward:?}"),
            }
        });
        reserve.spawn_with_ready_caps(args);
        serde_json::to_vec(&export).unwrap()
    }

    fn map_stream(&self, _: usize) -> &'static UntypedRingBuffer {
        unreachable!()
    }

    fn shared_memory(&self) -> (usize, usize) {
        unreachable!()
    }
}
