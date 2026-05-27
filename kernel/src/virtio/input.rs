mod config;
mod types;

use crate::drvli::InputDeviceServer;
use crate::interrupt::InterruptHandler;
use crate::ring_buffer::RingBuffer;
use crate::sync::Mutex;
use crate::util::volatile::{Readonly, Volatile};
use crate::virtio::Capabilities;
use crate::virtio::input::config::{Config, config_str};
use crate::virtio::input::types::ConfigSelect;
use crate::virtio::queue::Queue;
use crate::virtio::registers::{STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK};
use alloc::vec;
use alloc::vec::Vec;
use deravel_types::{InputEvent, RingBufferState};
use log::info;

struct State {
    eventq: Queue<0>,
    buffers: Vec<InputEvent>,
    ring: RingBuffer<InputEvent>,
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

        let mut buffers = vec![
            InputEvent {
                type_: 0,
                code: 0,
                value: 0,
            };
            QUEUE_SIZE
        ];
        let mut eventq = Queue::new(common, &caps.notify, QUEUE_SIZE);
        for (i, buffer) in buffers.iter_mut().enumerate() {
            eventq.descriptor_writeonly(i as u16, buffer, None);
            eventq.available.ring[i] = i as u16;
        }
        eventq.available.index = QUEUE_SIZE as u16;
        riscv::asm::fence();

        let name = config_str(caps.device, ConfigSelect::IdName, 0);
        info!("found {name}");

        let ring = RingBuffer::new();

        common.device_status().write_bitor(STATUS_DRIVER_OK as u8);
        VirtioInput {
            isr: caps.isr,
            state: Mutex::new(State {
                eventq,
                buffers,
                ring,
            }),
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
            state.ring.push(event);
        }
        riscv::asm::fence();
        unsafe {
            (&raw mut state.eventq.available.index).write_volatile(used_end + QUEUE_SIZE as u16)
        }
    }
}

impl InputDeviceServer for VirtioInput {
    fn events(&self) -> (*mut u8, usize, *mut RingBufferState) {
        self.state.lock().ring.expose()
    }
}
