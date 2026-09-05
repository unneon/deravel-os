mod types;

use crate::capability::grant_kernel_capability;
use crate::drvli::DisplayServer;
use crate::heap::granularity::{PageGranular, page_granular_vec};
use crate::interrupt::InterruptHandler;
use crate::page::virt_to_phys;
use crate::sync::Mutex;
use crate::util::untyped_box::UntypedBox;
use crate::util::volatile::{Volatile, volatile_struct};
use crate::virtio::gpu::types::*;
use crate::virtio::queue::Queue;
use crate::virtio::registers::{STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK, features};
use crate::virtio::{Capabilities, Isr};
use alloc::sync::Arc;
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
    isr: Isr,
    state: Mutex<State>,
}

struct State {
    config: Volatile<'static, Config>,
    controlq: Queue<0>,
    cursorq: Queue<1>,
    width: u32,
    height: u32,
    framebuffer: Option<Arc<UntypedBox<PageGranular>>>,
    cursor_image: Arc<UntypedBox<PageGranular>>,
    cursor_updated: bool,
}

impl VirtioGpu {
    pub fn new(capabilities: Capabilities<Config>) -> VirtioGpu {
        let mut common = capabilities.common;
        common.device_status().write(0);
        common.device_status().write_bitor(STATUS_ACKNOWLEDGE as u8);
        common.device_status().write_bitor(STATUS_DRIVER as u8);
        let controlq = Queue::new(&mut common, &capabilities.notify, 4);
        let cursorq = Queue::new(&mut common, &capabilities.notify, 4);
        common.device_status().write_bitor(STATUS_DRIVER_OK as u8);

        let mut gpu = VirtioGpu {
            isr: capabilities.isr,
            state: Mutex::new(State {
                config: capabilities.device,
                controlq,
                cursorq,
                width: 0,
                height: 0,
                framebuffer: None,
                cursor_image: Arc::new(UntypedBox::new(
                    page_granular_vec![0u8; 64 * 64 * 4].into_boxed_slice(),
                )),
                cursor_updated: true,
            }),
        };

        let (width, height) = gpu.get_resolution();
        let mut state = gpu.state.lock();
        state.width = width;
        state.height = height;
        state.framebuffer = Some(Arc::new(UntypedBox::new(
            page_granular_vec![0u8; width as usize * height as usize * 4].into_boxed_slice(),
        )));
        info!("detected a {width}x{height} display");

        let req = ResourceCreate2D {
            hdr: CtrlType::CmdResourceCreate2D.header(),
            resource_id: 1,
            format: Format::B8G8R8A8Unorm as u32,
            width,
            height,
        };
        state.controlq.descriptor_readonly(0, &req, Some(1));
        command::<ResponseNodata, _>(&mut state.controlq, 1).unwrap();

        let req = ResourceAttachBacking {
            hdr: CtrlType::CmdResourceAttachBacking.header(),
            resouce_id: 1,
            nr_entries: 1,
        };
        // TODO: Include this in reference count?
        let mem_entry = MemEntry {
            addr: virt_to_phys(state.framebuffer.as_ref().unwrap().as_untyped_ptr()) as u64,
            length: state.framebuffer.as_ref().unwrap().byte_size() as u32,
            padding: 0,
        };
        state.controlq.descriptor_readonly(0, &req, Some(1));
        state.controlq.descriptor_readonly(1, &mem_entry, Some(2));
        command::<ResponseNodata, _>(&mut state.controlq, 2).unwrap();

        let req = SetScanout {
            hdr: CtrlType::CmdSetScanout.header(),
            r: Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
            scanout_id: 0,
            resource_id: 1,
        };
        state.controlq.descriptor_readonly(0, &req, Some(1));
        command::<ResponseNodata, _>(&mut state.controlq, 1).unwrap();
        drop(state);

        gpu.initialize_cursor_memory();

        gpu
    }

    fn get_resolution(&self) -> (u32, u32) {
        let mut state = self.state.lock();
        let req = CtrlType::CmdGetDisplayInfo.header();
        state.controlq.descriptor_readonly(0, &req, Some(1));
        let resp: ResponseDisplayInfo = command(&mut state.controlq, 1).unwrap();
        let pmode = &resp.pmodes[0];
        assert_eq!(pmode.enabled, 1);
        let r = pmode.r;
        assert_eq!(r.x, 0);
        assert_eq!(r.y, 0);
        (r.width, r.height)
    }

    fn initialize_cursor_memory(&mut self) {
        let mut state = self.state.lock();
        let req = ResourceCreate2D {
            hdr: CtrlType::CmdResourceCreate2D.header(),
            resource_id: 2,
            // TODO: This seems to be ignored in favor of R8G8B8A8Unorm.
            format: Format::B8G8R8A8Unorm as u32,
            width: 64,
            height: 64,
        };
        state.controlq.descriptor_readonly(0, &req, Some(1));
        command::<ResponseNodata, _>(&mut state.controlq, 1).unwrap();

        let req = ResourceAttachBacking {
            hdr: CtrlType::CmdResourceAttachBacking.header(),
            resouce_id: 2,
            nr_entries: 1,
        };
        // TODO: Include this in reference count?
        let mem_entry = MemEntry {
            addr: virt_to_phys(state.cursor_image.as_untyped_ptr()) as u64,
            length: 64 * 64 * 4,
            padding: 0,
        };
        state.controlq.descriptor_readonly(0, &req, Some(1));
        state.controlq.descriptor_readonly(1, &mem_entry, Some(2));
        command::<ResponseNodata, _>(&mut state.controlq, 2).unwrap();
    }
}

impl InterruptHandler for VirtioGpu {
    fn handle(&self) {
        let mut state = self.state.lock();
        let isr = self.isr.clear();
        if isr.has_device_configuration_interrupt() {
            let events_read = state.config.events_read().read();
            if events_read & EVENT_DISPLAY != 0 {
                state.config.events_clear().write(EVENT_DISPLAY);
                drop(state);
                let (width, height) = self.get_resolution();
                info!("display configuration changed to {width}x{height}");
            }
        }
    }
}

impl DisplayServer for VirtioGpu {
    fn width(&self, _: ProcessId) -> u32 {
        self.state.lock().width
    }

    fn height(&self, _: ProcessId) -> u32 {
        self.state.lock().height
    }

    fn framebuffer(&self, sender: ProcessId) -> Capability<SharedMemory> {
        let state = self.state.lock();
        grant_kernel_capability(sender, state.framebuffer.as_ref().unwrap().clone())
    }

    fn draw(&self, _: ProcessId) {
        let mut state = self.state.lock();
        let r = Rect {
            x: 0,
            y: 0,
            width: state.width,
            height: state.height,
        };

        let req = TransferToHost2D {
            hdr: CtrlType::CmdTransferToHost2D.header(),
            r,
            offset: 0,
            resource_id: 1,
            padding: 0,
        };
        state.controlq.descriptor_readonly(0, &req, Some(1));
        command::<ResponseNodata, _>(&mut state.controlq, 1).unwrap();

        let req = ResourceFlush {
            hdr: CtrlType::CmdResourceFlush.header(),
            r,
            resource_id: 1,
            padding: 0,
        };
        state.controlq.descriptor_readonly(0, &req, Some(1));
        command::<ResponseNodata, _>(&mut state.controlq, 1).unwrap();
    }

    fn cursor_image_buffer(&self, sender: ProcessId) -> Capability<SharedMemory> {
        let state = self.state.lock();
        grant_kernel_capability(sender, state.cursor_image.clone())
    }

    fn cursor_image_modified(&self, _: ProcessId) {
        let mut state = self.state.lock();
        let req = TransferToHost2D {
            hdr: CtrlHdr {
                flags: FLAG_FENCE,
                ..CtrlType::CmdTransferToHost2D.header()
            },
            r: Rect {
                x: 0,
                y: 0,
                width: 64,
                height: 64,
            },
            offset: 0,
            resource_id: 2,
            padding: 0,
        };
        state.controlq.descriptor_readonly(0, &req, Some(1));
        command::<ResponseNodata, _>(&mut state.controlq, 1).unwrap();

        state.cursor_updated = true;
    }

    fn update_cursor(&self, _: ProcessId, x: u32, y: u32) {
        let mut state = self.state.lock();
        let req = UpdateCursor {
            hdr: if state.cursor_updated {
                state.cursor_updated = false;
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
        state.cursorq.descriptor_readonly(0, &req, None);
        state.cursorq.send_and_recv(0);
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
