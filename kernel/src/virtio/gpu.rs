mod types;

use crate::capability::grant_kernel_capability;
use crate::drvli::DisplayServer;
use crate::interrupt::InterruptHandler;
use crate::sync::Mutex;
use crate::util::volatile::{Readonly, Volatile, volatile_struct};
use crate::virtio::Capabilities;
use crate::virtio::gpu::types::*;
use crate::virtio::queue::Queue;
use crate::virtio::registers::{STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK, features};
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use deravel_types::{Capability, ProcessId, SharedMemory};
use log::info;

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
    width: u32,
    height: u32,
    framebuffer: Vec<u8>,
}

impl VirtioGpu {
    pub fn new(capabilities: Capabilities<Config>) -> VirtioGpu {
        let common = capabilities.common;
        common.device_status().write(0);
        common.device_status().write_bitor(STATUS_ACKNOWLEDGE as u8);
        common.device_status().write_bitor(STATUS_DRIVER as u8);
        let controlq = Queue::new(common, &capabilities.notify, 4);
        common.device_status().write_bitor(STATUS_DRIVER_OK as u8);

        let mut gpu = VirtioGpu {
            isr: capabilities.isr,
            controlq,
            width: 0,
            height: 0,
            framebuffer: Vec::new(),
        };

        let req = CtrlType::CmdGetDisplayInfo.header();
        gpu.controlq.descriptor_readonly(0, &req, Some(1));
        let resp: ResponseDisplayInfo = gpu.command(1).unwrap();
        let pmode = &resp.pmodes[0];
        assert_eq!(pmode.enabled, 1);
        let r = pmode.r;
        assert_eq!(r.x, 0);
        assert_eq!(r.y, 0);
        let width = r.width;
        let height = r.height;
        gpu.width = width;
        gpu.height = height;
        gpu.framebuffer = vec![0u8; width as usize * height as usize * 4];
        info!("detected a {width}x{height} display");

        let req = ResourceCreate2D {
            hdr: CtrlType::CmdResourceCreate2D.header(),
            resource_id: 1,
            format: Format::B8G8R8A8Unorm as u32,
            width,
            height,
        };
        gpu.controlq.descriptor_readonly(0, &req, Some(1));
        gpu.command::<ResponseNodata>(1).unwrap();

        let req = ResourceAttachBacking {
            hdr: CtrlType::CmdResourceAttachBacking.header(),
            resouce_id: 1,
            nr_entries: 1,
        };
        let mem_entry = MemEntry {
            addr: gpu.framebuffer.as_ptr() as u64,
            length: gpu.framebuffer.len() as u32,
            padding: 0,
        };
        gpu.controlq.descriptor_readonly(0, &req, Some(1));
        gpu.controlq.descriptor_readonly(1, &mem_entry, Some(2));
        gpu.command::<ResponseNodata>(2).unwrap();

        let req = SetScanout {
            hdr: CtrlType::CmdSetScanout.header(),
            r,
            scanout_id: 0,
            resource_id: 1,
        };
        gpu.controlq.descriptor_readonly(0, &req, Some(1));
        gpu.command::<ResponseNodata>(1).unwrap();

        gpu
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

impl InterruptHandler for Mutex<VirtioGpu> {
    fn handle(&self) {
        self.lock().isr.read();
    }
}

impl DisplayServer for Mutex<VirtioGpu> {
    fn width(&self, _: ProcessId) -> u32 {
        self.lock().width
    }

    fn height(&self, _: ProcessId) -> u32 {
        self.lock().height
    }

    fn framebuffer(&self, sender: ProcessId) -> Capability<SharedMemory> {
        let self_ = self.lock();
        grant_kernel_capability(
            sender,
            Box::leak(Box::new(crate::shared_memory::SharedMemory {
                physical_address: self_.framebuffer.as_ptr() as u64,
                length: (self_.width * self_.height * 4) as u64,
            })),
        )
    }

    fn draw(&self, _: ProcessId) {
        let mut self_ = self.lock();
        let r = Rect {
            x: 0,
            y: 0,
            width: self_.width,
            height: self_.height,
        };

        let req = TransferToHost2D {
            hdr: CtrlType::CmdTransferToHost2D.header(),
            r,
            offset: 0,
            resource_id: 1,
            padding: 0,
        };
        self_.controlq.descriptor_readonly(0, &req, Some(1));
        self_.command::<ResponseNodata>(1).unwrap();

        let req = ResourceFlush {
            hdr: CtrlType::CmdResourceFlush.header(),
            r,
            resource_id: 1,
            padding: 0,
        };
        self_.controlq.descriptor_readonly(0, &req, Some(1));
        self_.command::<ResponseNodata>(1).unwrap();
    }
}
