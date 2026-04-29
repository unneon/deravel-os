mod config;
mod types;

use crate::interrupt::InterruptHandler;
use crate::sync::Mutex;
use crate::util::volatile::{Readonly, Volatile};
use crate::virtio::Capabilities;
use crate::virtio::input::config::{Config, config_str};
use crate::virtio::input::types::ConfigSelect;
use crate::virtio::queue::Queue;
use crate::virtio::registers::{STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK};
use alloc::vec;
use alloc::vec::Vec;
use log::{debug, info};

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
struct Event {
    type_: u16,
    code: u16,
    value: u32,
}

struct State {
    eventq: Queue<0>,
    buffers: Vec<Event>,
}

pub struct VirtioInput {
    isr: Volatile<u8, Readonly>,
    state: Mutex<State>,
}

const QUEUE_SIZE: usize = 64;

impl VirtioInput {
    pub fn new(caps: Capabilities<Config>) -> VirtioInput {
        let common = caps.common;
        common.device_status().write(0);
        common.device_status().write_bitor(STATUS_ACKNOWLEDGE as u8);
        common.device_status().write_bitor(STATUS_DRIVER as u8);

        let mut buffers = vec![Event::default(); QUEUE_SIZE];
        let mut eventq = Queue::new(common, &caps.notify, QUEUE_SIZE);
        for (i, buffer) in buffers.iter_mut().enumerate() {
            eventq.descriptor_writeonly(i as u16, buffer, None);
            eventq.available.ring[i] = i as u16;
        }
        eventq.available.index = QUEUE_SIZE as u16;
        riscv::asm::fence();

        let name = config_str(caps.device, ConfigSelect::IdName, 0);
        info!("found {name}");

        common.device_status().write_bitor(STATUS_DRIVER_OK as u8);
        VirtioInput {
            isr: caps.isr,
            state: Mutex::new(State { eventq, buffers }),
        }
    }
}

impl InterruptHandler for VirtioInput {
    fn handle(&self) {
        let mut state = self.state.lock();
        self.isr.read();
        let used_start = state.eventq.available.index - QUEUE_SIZE as u16;
        let used_end = unsafe { (&raw const state.eventq.used.index).read_volatile() };
        riscv::asm::fence();
        for used_index in used_start..used_end {
            let event = state.buffers[used_index as usize % QUEUE_SIZE];
            if event.type_ == 0 {
                continue;
            }
            debug!("received {event:?}");
        }
        riscv::asm::fence();
        unsafe {
            (&raw mut state.eventq.available.index).write_volatile(used_end + QUEUE_SIZE as u16)
        }
    }
}
