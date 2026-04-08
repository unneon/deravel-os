mod config;
mod types;

use crate::interrupt::InterruptHandler;
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
struct InputEvent {
    type_: u16,
    code: u16,
    value: u32,
}

pub struct VirtioInput {
    isr: Volatile<u8, Readonly>,
    eventq: Queue<0>,
    buffers: Vec<InputEvent>,
}

const QUEUE_SIZE: usize = 64;

impl VirtioInput {
    pub fn new(caps: Capabilities<Config>) -> VirtioInput {
        let common = caps.common;
        common.device_status().write(0);
        common.device_status().write_bitor(STATUS_ACKNOWLEDGE as u8);
        common.device_status().write_bitor(STATUS_DRIVER as u8);

        let mut buffers = vec![InputEvent::default(); QUEUE_SIZE];
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
            eventq,
            buffers,
        }
    }

    pub fn demo(&mut self) {
        while let Some(event) = self.try_recv() {
            debug!("received {event:?}");
        }
    }

    fn try_recv(&mut self) -> Option<InputEvent> {
        let available_index = self.eventq.available.index;
        let used_index = unsafe { (&raw const self.eventq.used.index).read_volatile() };
        if available_index - used_index == QUEUE_SIZE as u16 {
            return None;
        }
        riscv::asm::fence();
        let event = self.buffers[available_index as usize % QUEUE_SIZE];
        riscv::asm::fence();
        self.eventq.available.index += 1;
        riscv::asm::fence();
        Some(event)
    }
}

impl InterruptHandler for VirtioInput {
    fn handle(&self) {
        self.isr.read();
    }
}
