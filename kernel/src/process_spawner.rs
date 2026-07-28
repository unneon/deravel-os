use crate::capability::{Handler, capability_certificate};
use crate::process::{get_process, reserve_process};
use alloc::vec;
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
        capability_certificate(*export).store(
            CapabilityCertificateValue::granted(sender),
            Ordering::Relaxed,
        );
        let args: <T as ProcessTag>::Args = postcard::from_bytes(args).unwrap();
        args.for_all(|cap| {
            let slot = capability_certificate(cap);
            let preforward = slot.load(Ordering::Relaxed).unpack();
            match preforward {
                CapabilityCertificateUnpacked::Granted {
                    grantee: Actor::Kernel,
                } => {
                    assert_eq!(
                        cap.certifier(),
                        Actor::Userspace(sender),
                        "{}{sender:?} tried to pass {cap:?} granted by {}{:?}",
                        get_process(sender).lock_if_some().unwrap().name,
                        get_process(cap.certifier().unwrap_user())
                            .lock_if_some()
                            .unwrap()
                            .name,
                        cap.certifier(),
                    );
                    slot.store(
                        CapabilityCertificateValue::granted(reserve.id),
                        Ordering::Relaxed,
                    )
                }
                CapabilityCertificateUnpacked::Forwarded {
                    forwardee: Actor::Kernel,
                    inner,
                } => {
                    if let Err(err) = inner.validate(sender) {
                        panic!(
                            "{}{sender:?} tried to pass {cap:?}, {err}",
                            get_process(sender).lock_if_some().unwrap().name
                        );
                    }
                    slot.store(
                        CapabilityCertificateValue::forwarded(reserve.id.into(), inner),
                        Ordering::Relaxed,
                    )
                }
                _ => unreachable!("{preforward:?}"),
            }
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

    fn shared_memory(&self) -> (usize, usize) {
        unreachable!()
    }
}
