mod types;

use crate::util::volatile::{Volatile, volatile_struct};
use crate::virtio::gpu::types::*;
use crate::virtio::queue::{QUEUE_SIZE, Queue};
use crate::virtio::registers::{STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK, features};
use crate::virtio::{NotifySlot, VirtioCommonConfig};
use alloc::vec;
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
    controlq: Queue<0>,
}

impl VirtioGpu {
    pub fn new(
        common: Volatile<VirtioCommonConfig>,
        notify: NotifySlot,
        _device: Volatile<Config>,
    ) -> VirtioGpu {
        common.device_status().write(0);
        common.device_status().write_bitor(STATUS_ACKNOWLEDGE as u8);
        common.device_status().write_bitor(STATUS_DRIVER as u8);
        let controlq = Queue::new(common, &notify, QUEUE_SIZE);
        common.device_status().write_bitor(STATUS_DRIVER_OK as u8);
        VirtioGpu { controlq }
    }

    pub fn demo(&mut self) {
        let req = CtrlHdr {
            type_: VIRTIO_GPU_CMD_GET_DISPLAY_INFO,
            ..Default::default()
        };
        let mut resp: ResponseDisplayInfo = Default::default();
        self.controlq.descriptor_readonly(0, &req, Some(1));
        self.controlq.descriptor_writeonly(1, &mut resp, None);
        self.controlq.send_and_recv(0);
        assert_eq!(resp.hdr.type_, VIRTIO_GPU_RESP_OK_DISPLAY_INFO);
        let pmode = &resp.pmodes[0];
        assert_eq!(pmode.enabled, 1);
        assert_eq!(pmode.r.x, 0);
        assert_eq!(pmode.r.y, 0);
        info!("detected a {}x{} display", pmode.r.width, pmode.r.height);

        let req = ResourceCreate2D {
            hdr: CtrlHdr {
                type_: VIRTIO_GPU_CMD_RESOURCE_CREATE_2D,
                ..Default::default()
            },
            resource_id: 1,
            format: VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM,
            width: pmode.r.width,
            height: pmode.r.height,
        };
        let mut resp: CtrlHdr = Default::default();
        self.controlq.descriptor_readonly(0, &req, Some(1));
        self.controlq.descriptor_writeonly(1, &mut resp, None);
        self.controlq.send_and_recv(0);
        assert_eq!(resp.type_, VIRTIO_GPU_RESP_OK_NODATA);

        let mut framebuffer = vec![0u8; pmode.r.width as usize * pmode.r.height as usize * 4];
        let req = ResourceAttachBacking {
            hdr: CtrlHdr {
                type_: VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING,
                ..Default::default()
            },
            resouce_id: 1,
            nr_entries: 1,
        };
        let mem_entry = MemEntry {
            addr: framebuffer.as_ptr() as u64,
            length: framebuffer.len() as u32,
            padding: 0,
        };
        let mut resp: CtrlHdr = Default::default();
        self.controlq.descriptor_readonly(0, &req, Some(1));
        self.controlq.descriptor_readonly(1, &mem_entry, Some(2));
        self.controlq.descriptor_writeonly(2, &mut resp, None);
        self.controlq.send_and_recv(0);
        assert_eq!(resp.type_, VIRTIO_GPU_RESP_OK_NODATA);

        let req = SetScanout {
            hdr: CtrlHdr {
                type_: VIRTIO_GPU_CMD_SET_SCANOUT,
                ..Default::default()
            },
            r: Rect {
                x: 0,
                y: 0,
                width: pmode.r.width,
                height: pmode.r.height,
            },
            scanout_id: 0,
            resource_id: 1,
        };
        let mut resp = CtrlHdr::default();
        self.controlq.descriptor_readonly(0, &req, Some(1));
        self.controlq.descriptor_writeonly(1, &mut resp, None);
        self.controlq.send_and_recv(0);
        assert_eq!(resp.type_, VIRTIO_GPU_RESP_OK_NODATA);

        for [b, g, r, a] in framebuffer.as_chunks_mut().0 {
            *b = 255;
            *g = 0;
            *r = 255;
            *a = 255;
        }

        let req = TransferToHost2D {
            hdr: CtrlHdr {
                type_: VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D,
                ..Default::default()
            },
            r: Rect {
                x: 0,
                y: 0,
                width: pmode.r.width,
                height: pmode.r.height,
            },
            offset: 0,
            resource_id: 1,
            padding: 0,
        };
        let mut resp = CtrlHdr::default();
        self.controlq.descriptor_readonly(0, &req, Some(1));
        self.controlq.descriptor_writeonly(1, &mut resp, None);
        self.controlq.send_and_recv(0);
        assert_eq!(resp.type_, VIRTIO_GPU_RESP_OK_NODATA);

        let req = ResourceFlush {
            hdr: CtrlHdr {
                type_: VIRTIO_GPU_CMD_RESOURCE_FLUSH,
                ..Default::default()
            },
            r: Rect {
                x: 0,
                y: 0,
                width: pmode.r.width,
                height: pmode.r.height,
            },
            resource_id: 1,
            padding: 0,
        };
        let mut resp = CtrlHdr::default();
        self.controlq.descriptor_readonly(0, &req, Some(1));
        self.controlq.descriptor_writeonly(1, &mut resp, None);
        self.controlq.send_and_recv(0);
        assert_eq!(resp.type_, VIRTIO_GPU_RESP_OK_NODATA);
    }
}
