use crate::capability::grant_kernel_capability;
use crate::drvli::SyscallHandler;
use crate::heap::granularity::page_granular_vec;
use crate::log::log_userspace;
use crate::page::{PageFlags, virt_to_phys};
use crate::process::{Message, ProcessState, get_process, kill};
use crate::stack::UserCtx;
use crate::syscall::SyscallAction::Yield;
use crate::user::{UserPtr, UserSyscallError};
use crate::util::untyped_box::UntypedBox;
use crate::{capability, shared_memory};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::ops::DerefMut;
use deravel_types::{
    Actor, CACHE_LINE_SIZE, Capability, PAGE_SIZE, ProcessId, RawCapability, SharedMemory,
};
use log::Level;

pub type Result<T> = core::result::Result<T, SyscallAction>;

pub enum SyscallAction {
    UserErr(UserSyscallError),
    Yield,
}

impl SyscallHandler for () {
    fn exit(user: &mut UserCtx) -> Result<!> {
        user.process().state = ProcessState::Finished;
        Err(Yield)
    }

    fn ipc_call(
        user: &mut UserCtx,
        cap: RawCapability,
        method: usize,
        args_buffer: UserPtr<[u8]>,
        mut result_buffer: UserPtr<[u8]>,
    ) -> Result<usize> {
        let mut proc = user.process();
        let cap = match cap.validate(proc.id) {
            Ok(cap) => cap,
            Err(err) => kill!(user, proc, "{err}"),
        };

        match cap.certifier() {
            Actor::Userspace(dest) => {
                let Some(mut dest) = get_process(dest).lock_if_some() else {
                    // This can't actually happen because capability validation will catch this
                    // earlier, but let's check in case the design changes later.
                    kill!(user, proc, "ipc send to nonexistent process");
                };

                proc.state = ProcessState::WaitingForReply {
                    from: cap.certifier(),
                    result_buffer,
                };

                dest.messages.push_back(Message {
                    cap,
                    method,
                    args: args_buffer.copy_to_kernel(),
                    sender: user.pid(),
                });

                Err(Yield)
            }
            Actor::Kernel => {
                drop(proc);
                let handler = capability::get_handler(cap.local_index());
                let result = handler.call_method(method, &args_buffer.copy_to_kernel(), user.pid());
                if let Err(err) = result_buffer.write_to_user(&result) {
                    kill!(user, "{err}")
                };
                Ok(result.len())
            }
        }
    }

    fn ipc_receive(
        user: &mut UserCtx,
        mut args: UserPtr<[u8]>,
    ) -> Result<(Option<RawCapability>, usize, usize, Option<ProcessId>)> {
        let mut proc = user.process();
        if proc.currently_serving.is_some() {
            kill!(user, proc, "ipc receive without replying to previous one")
        }
        let Some(message) = proc.messages.pop_front() else {
            return Ok((None, 0, 0, None));
        };
        if let Err(err) = args.write_to_user(&message.args) {
            kill!(user, proc, "{err}")
        };
        proc.currently_serving = Some(message.sender);
        Ok((
            Some(message.cap),
            message.method,
            message.args.len(),
            Some(message.sender),
        ))
    }

    fn ipc_reply(user: &mut UserCtx, result: UserPtr<[u8]>) -> Result<()> {
        let mut proc = user.process();
        let Some(caller) = proc.currently_serving.take() else {
            kill!(user, proc, "ipc_reply called without matching ipc_serve")
        };
        let mut caller = get_process(caller).lock_if_some().unwrap();
        if let ProcessState::WaitingForReply {
            from,
            result_buffer,
        } = &caller.state
        {
            if *from != Actor::Userspace(proc.id) {
                kill!(user, proc, "replied to process waiting for someone else");
            }
            caller.state = ProcessState::ReadyReply {
                reply: result.copy_to_kernel(),
                result_buffer: result_buffer.clone(),
            };
            Ok(())
        } else if let ProcessState::WaitingForStreamMap { from } = caller.state {
            if from != Actor::Userspace(proc.id) {
                kill!(user, proc, "replied to process waiting for someone else");
            }
            let Ok((stream, declared_size)) =
                postcard::from_bytes::<(RawCapability, usize)>(&result.copy_to_kernel())
            else {
                kill!(user, proc, "invalid stream map reply")
            };
            let Ok(ring) = stream.validate(caller.id) else {
                kill!(user, proc, "ring {stream:?} not valid for sender");
            };
            if ring.certifier() != Actor::Kernel {
                kill!(user, proc, "shared memory must be granted by the kernel");
            }
            let handler = capability::get_handler(ring.local_index());
            let length = handler.shared_memory_size();
            if !length.is_multiple_of(PAGE_SIZE) {
                kill!(user, proc, "stream size must be a multiple of page size")
            }
            if length < 2 * CACHE_LINE_SIZE + declared_size {
                kill!(user, proc, "stream length does not match memory size")
            }
            caller.state = ProcessState::ReadyStreamMap {
                ring,
                declared_size,
            };
            Ok(())
        } else {
            unimplemented!()
        }
    }

    fn ipc_stream(
        user: &mut UserCtx,
        cap: RawCapability,
        stream: usize,
    ) -> Result<(*mut (), usize)> {
        let mut proc = user.process();
        let cap = match cap.validate(proc.id) {
            Ok(cap) => cap,
            Err(err) => kill!(user, proc, "{err}"),
        };
        match cap.certifier() {
            Actor::Userspace(original_pid) => {
                proc.state = ProcessState::WaitingForStreamMap {
                    from: original_pid.into(),
                };
                let mut dest = get_process(original_pid).lock_if_some().unwrap();
                dest.messages.push_back(Message {
                    cap,
                    method: 1000 + stream,
                    args: Vec::new(),
                    sender: user.pid(),
                });

                Err(Yield)
            }
            Actor::Kernel => {
                let handler = capability::get_handler(cap.local_index());
                let ring_buffer = handler.map_stream(stream);
                let ring_buffer_size = size_of_val(ring_buffer);
                let ring_buffer_layout =
                    Layout::from_size_align(ring_buffer_size, PAGE_SIZE).unwrap();

                let virt = proc.heap.alloc(ring_buffer_layout).unwrap();
                proc.page_table.map(
                    virt,
                    virt_to_phys(ring_buffer as *const _ as *const u8) as usize,
                    PAGE_SIZE,
                    PageFlags::read_write().user(),
                );
                riscv::asm::sfence_vma_all();
                Ok((virt as *mut (), ring_buffer.0.data.0.len()))
            }
        }
    }

    fn alloc(user: &mut UserCtx, size: usize) -> Result<*mut u8> {
        let size = size.next_multiple_of(PAGE_SIZE);
        let pages = Arc::new(UntypedBox::new(
            page_granular_vec![0u8; size].into_boxed_slice(),
        ));
        let virt = user.process().alloc(pages, PageFlags::read_write().user());
        riscv::asm::sfence_vma_all();
        Ok(virt)
    }

    fn alloc_shared(
        user: &mut UserCtx,
        size: usize,
    ) -> Result<(*mut u8, Capability<SharedMemory>)> {
        let size = size.next_multiple_of(PAGE_SIZE);
        let pages = Arc::new(UntypedBox::new(
            page_granular_vec![0u8; size].into_boxed_slice(),
        ));
        let virt = user
            .process()
            .alloc(pages.clone(), PageFlags::read_write().user());
        riscv::asm::sfence_vma_all();
        let cap = grant_kernel_capability(
            user.pid(),
            Box::leak(Box::new(shared_memory::SharedMemory { backing: pages })),
        );
        Ok((virt, cap))
    }

    fn map_shared(user: &mut UserCtx, cap: Capability<SharedMemory>) -> Result<(*mut u8, usize)> {
        let mut proc = user.process();
        let cap = match cap.validate(proc.id) {
            Ok(cap) => cap,
            Err(err) => kill!(user, proc, "{err}"),
        };
        if cap.certifier() != Actor::Kernel {
            kill!(user, proc, "non-kernel shared memory capability")
        }

        let handler = capability::get_handler(cap.local_index());
        let length = handler.shared_memory_size();
        let padded_length = length.next_multiple_of(PAGE_SIZE);
        let layout = Layout::from_size_align(padded_length, PAGE_SIZE).unwrap();

        let virt = proc.heap.alloc(layout).unwrap();
        let proc = proc.deref_mut();
        handler.shared_memory_map(
            virt,
            &mut proc.page_table,
            &mut proc.virtual_memory_mappings,
        );
        riscv::asm::sfence_vma_all();

        Ok((virt as *mut u8, length))
    }

    fn free(user: &mut UserCtx, ptr: UserPtr<u8>) -> Result<()> {
        let mut proc = user.process();
        if proc.dealloc(ptr.as_ptr()).is_err() {
            kill!(user, proc, "free of unallocated pointer")
        }
        Ok(())
    }

    fn yield_(_: &mut UserCtx) -> Result<()> {
        Err(Yield)
    }

    fn log(user: &mut UserCtx, message: UserPtr<[u8]>, level: u64) -> Result<()> {
        let Ok(text) = String::from_utf8(message.copy_to_kernel()) else {
            kill!(user, "invalid utf-8")
        };
        let level = match level {
            0 => Level::Error,
            1 => Level::Warn,
            2 => Level::Info,
            3 => Level::Debug,
            4 => Level::Trace,
            _ => kill!(user, "invalid log level"),
        };
        log_userspace(level, &user.process(), &text);
        Ok(())
    }
}

impl<T: Into<UserSyscallError>> From<T> for SyscallAction {
    fn from(err: T) -> SyscallAction {
        SyscallAction::UserErr(err.into())
    }
}
