mod types;

use crate::interrupt::InterruptHandler;
use crate::util::volatile::{Readonly, Volatile, volatile_struct};
use crate::virtio::Capabilities;
use crate::virtio::gpu::types::*;
use crate::virtio::queue::Queue;
use crate::virtio::registers::{STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK, features};
use alloc::vec;
use log::{debug, info};

volatile_struct! { pub Config
    events_read: Readonly u32,
    events_clear: ReadWrite u32,
    num_scanouts: Readonly u32,
    num_capsets: Readonly u32,
}

features! { VirtioGpu Features 0
    has_virgl enable_virgl 0
    has_edid enable_edid 1
    has_resource_uuid enable_resource_uuid 2
    has_resource_blob enable_resource_blob 3
    has_context_init enable_context_init 4
}

pub struct VirtioGpu {
    isr: Volatile<u8, Readonly>,
    controlq: Queue<0>,
}

impl VirtioGpu {
    pub fn new(capabilities: Capabilities<Config>) -> VirtioGpu {
        let common = capabilities.common;
        common.device_status().write(0);
        common.device_status().write_bitor(STATUS_ACKNOWLEDGE as u8);
        common.device_status().write_bitor(STATUS_DRIVER as u8);
        let controlq = Queue::new(common, &capabilities.notify, 4);
        common.device_status().write_bitor(STATUS_DRIVER_OK as u8);
        VirtioGpu {
            isr: capabilities.isr,
            controlq,
        }
    }

    pub fn demo(&mut self) {
        let req = CtrlType::CmdGetDisplayInfo.header();
        self.controlq.descriptor_readonly(0, &req, Some(1));
        let resp: ResponseDisplayInfo = self.command(1).unwrap();
        let pmode = &resp.pmodes[0];
        assert_eq!(pmode.enabled, 1);
        assert_eq!(pmode.r.x, 0);
        assert_eq!(pmode.r.y, 0);
        let r = pmode.r;
        let width = r.width;
        let height = r.height;
        info!("detected a {width}x{height} display");

        let req = ResourceCreate2D {
            hdr: CtrlType::CmdResourceCreate2D.header(),
            resource_id: 1,
            format: Format::B8G8R8A8Unorm as u32,
            width,
            height,
        };
        self.controlq.descriptor_readonly(0, &req, Some(1));
        self.command::<ResponseNodata>(1).unwrap();

        let mut framebuffer = vec![0u8; width as usize * height as usize * 4];
        let req = ResourceAttachBacking {
            hdr: CtrlType::CmdResourceAttachBacking.header(),
            resouce_id: 1,
            nr_entries: 1,
        };
        let mem_entry = MemEntry {
            addr: framebuffer.as_ptr() as u64,
            length: framebuffer.len() as u32,
            padding: 0,
        };
        self.controlq.descriptor_readonly(0, &req, Some(1));
        self.controlq.descriptor_readonly(1, &mem_entry, Some(2));
        self.command::<ResponseNodata>(2).unwrap();

        let req = SetScanout {
            hdr: CtrlType::CmdSetScanout.header(),
            r,
            scanout_id: 0,
            resource_id: 1,
        };
        self.controlq.descriptor_readonly(0, &req, Some(1));
        self.command::<ResponseNodata>(1).unwrap();

        for [b, g, r, a] in framebuffer.as_chunks_mut().0 {
            *b = 255;
            *g = 0;
            *r = 255;
            *a = 255;
        }

        let req = TransferToHost2D {
            hdr: CtrlType::CmdTransferToHost2D.header(),
            r,
            offset: 0,
            resource_id: 1,
            padding: 0,
        };
        self.controlq.descriptor_readonly(0, &req, Some(1));
        self.command::<ResponseNodata>(1).unwrap();

        let req = ResourceFlush {
            hdr: CtrlType::CmdResourceFlush.header(),
            r,
            resource_id: 1,
            padding: 0,
        };
        self.controlq.descriptor_readonly(0, &req, Some(1));
        self.command::<ResponseNodata>(1).unwrap();
    }

    fn command<T: Response>(&mut self, input_descriptors: usize) -> Result<T, Error> {
        let mut response = T::default();
        self.controlq
            .descriptor_writeonly(input_descriptors as u16, &mut response, None);
        self.controlq.send_and_recv(0);
        if response.hdr().type_ & 0xFF00 == 0x1200 {
            Err(unsafe { core::mem::transmute::<u32, Error>(response.hdr().type_) })
        } else {
            assert_eq!(response.hdr().type_, T::TYPE);
            Ok(response)
        }
    }
}

impl InterruptHandler for VirtioGpu {
    fn handle(&self) {
        debug!("interrupt handler, isr {:#x}", self.isr.read());
    }
}
