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
use log::*;

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
    cursorq: Queue<1>,
    width: u32,
    height: u32,
    framebuffer: Vec<u8>,
    cursor_image: Vec<u8>,
    cursor_updated: bool,
}

impl VirtioGpu {
    pub fn new(capabilities: Capabilities<Config>) -> VirtioGpu {
        let common = capabilities.common;
        common.device_status().write(0);
        common.device_status().write_bitor(STATUS_ACKNOWLEDGE as u8);
        common.device_status().write_bitor(STATUS_DRIVER as u8);
        let controlq = Queue::new(common, &capabilities.notify, 4);
        let cursorq = Queue::new(common, &capabilities.notify, 4);
        common.device_status().write_bitor(STATUS_DRIVER_OK as u8);

        let mut gpu = VirtioGpu {
            isr: capabilities.isr,
            controlq,
            cursorq,
            width: 0,
            height: 0,
            framebuffer: Vec::new(),
            cursor_image: vec![0; 64 * 64 * 4],
            cursor_updated: true,
        };

        let req = CtrlType::CmdGetDisplayInfo.header();
        gpu.controlq.descriptor_readonly(0, &req, Some(1));
        let resp: ResponseDisplayInfo = command(&mut gpu.controlq, 1).unwrap();
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
        command::<ResponseNodata, _>(&mut gpu.controlq, 1).unwrap();

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
        command::<ResponseNodata, _>(&mut gpu.controlq, 2).unwrap();

        let req = SetScanout {
            hdr: CtrlType::CmdSetScanout.header(),
            r,
            scanout_id: 0,
            resource_id: 1,
        };
        gpu.controlq.descriptor_readonly(0, &req, Some(1));
        command::<ResponseNodata, _>(&mut gpu.controlq, 1).unwrap();

        gpu.initialize_cursor_memory();

        gpu
    }

    fn initialize_cursor_memory(&mut self) {
        let req = ResourceCreate2D {
            hdr: CtrlType::CmdResourceCreate2D.header(),
            resource_id: 2,
            // TODO: This seems to be ignored in favor of R8G8B8A8Unorm.
            format: Format::B8G8R8A8Unorm as u32,
            width: 64,
            height: 64,
        };
        self.controlq.descriptor_readonly(0, &req, Some(1));
        command::<ResponseNodata, _>(&mut self.controlq, 1).unwrap();

        let req = ResourceAttachBacking {
            hdr: CtrlType::CmdResourceAttachBacking.header(),
            resouce_id: 2,
            nr_entries: 1,
        };
        let mem_entry = MemEntry {
            addr: self.cursor_image.as_ptr() as u64,
            length: 64 * 64 * 4,
            padding: 0,
        };
        self.controlq.descriptor_readonly(0, &req, Some(1));
        self.controlq.descriptor_readonly(1, &mem_entry, Some(2));
        command::<ResponseNodata, _>(&mut self.controlq, 2).unwrap();
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
                physical_address: self_.framebuffer.as_ptr() as usize,
                size: (self_.width * self_.height * 4) as usize,
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
        command::<ResponseNodata, _>(&mut self_.controlq, 1).unwrap();

        let req = ResourceFlush {
            hdr: CtrlType::CmdResourceFlush.header(),
            r,
            resource_id: 1,
            padding: 0,
        };
        self_.controlq.descriptor_readonly(0, &req, Some(1));
        command::<ResponseNodata, _>(&mut self_.controlq, 1).unwrap();
    }

    fn set_cursor_image(&self, _: ProcessId, image: &[u8]) {
        assert_eq!(image.len(), 64 * 64 * 4);
        let mut self_ = self.lock();
        self_.cursor_image.copy_from_slice(image);

        let r = Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
        };

        // TODO: There should be a GPU fence here once I do GL acceleration.
        let req = TransferToHost2D {
            hdr: CtrlHdr {
                flags: FLAG_FENCE,
                ..CtrlType::CmdTransferToHost2D.header()
            },
            r,
            offset: 0,
            resource_id: 2,
            padding: 0,
        };
        self_.controlq.descriptor_readonly(0, &req, Some(1));
        command::<ResponseNodata, _>(&mut self_.controlq, 1).unwrap();

        self_.cursor_updated = true;
    }

    fn update_cursor(&self, _: ProcessId, x: u32, y: u32) {
        let mut self_ = self.lock();
        let req = UpdateCursor {
            hdr: if self_.cursor_updated {
                self_.cursor_updated = false;
                CtrlType::CmdUpdateCursor
            } else {
                CtrlType::CmdMoveCursor
            }
            .header(),
            pos: CursorPos {
                scanout_id: 0,
                x,
                y,
                padding: 0,
            },
            resource_id: 2,
            hot_x: 0,
            hot_y: 0,
            padding: 0,
        };
        self_.cursorq.descriptor_readonly(0, &req, None);
        self_.cursorq.send_and_recv(0);
    }
}

fn command<T: Response, const INDEX: u16>(
    queue: &mut Queue<INDEX>,
    input_descriptors: usize,
) -> Result<T, Error> {
    let mut response = T::default();
    queue.descriptor_writeonly(input_descriptors as u16, &mut response, None);
    queue.send_and_recv(0);
    if response.hdr().type_ & 0xFF00 == 0x1200 {
        Err(unsafe { core::mem::transmute::<u32, Error>(response.hdr().type_) })
    } else {
        assert_eq!(response.hdr().type_, T::TYPE, "{response:?}");
        Ok(response)
    }
}
