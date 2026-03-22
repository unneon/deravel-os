#[derive(Debug, Default)]
#[repr(C)]
pub struct CtrlHdr {
    pub type_: u32,
    pub flags: u32,
    pub fence_id: u64,
    pub ctx_id: u32,
    pub ring_idx: u8,
    pub padding: [u8; 3],
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct DisplayOne {
    pub r: Rect,
    pub enabled: u32,
    pub flags: u32,
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct MemEntry {
    pub addr: u64,
    pub length: u32,
    pub padding: u32,
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct ResourceAttachBacking {
    pub hdr: CtrlHdr,
    pub resouce_id: u32,
    pub nr_entries: u32,
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct ResourceCreate2D {
    pub hdr: CtrlHdr,
    pub resource_id: u32,
    pub format: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct ResourceFlush {
    pub hdr: CtrlHdr,
    pub r: Rect,
    pub resource_id: u32,
    pub padding: u32,
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct ResponseDisplayInfo {
    pub hdr: CtrlHdr,
    pub pmodes: [DisplayOne; VIRTIO_GPU_MAX_SCANOUTS],
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct SetScanout {
    pub hdr: CtrlHdr,
    pub r: Rect,
    pub scanout_id: u32,
    pub resource_id: u32,
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct TransferToHost2D {
    pub hdr: CtrlHdr,
    pub r: Rect,
    pub offset: u64,
    pub resource_id: u32,
    pub padding: u32,
}

pub const VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM: u32 = 1;
pub const VIRTIO_GPU_MAX_SCANOUTS: usize = 16;

pub const VIRTIO_GPU_CMD_GET_DISPLAY_INFO: u32 = 0x0100;
pub const VIRTIO_GPU_CMD_RESOURCE_CREATE_2D: u32 = 0x0101;
pub const VIRTIO_GPU_CMD_SET_SCANOUT: u32 = 0x0103;
pub const VIRTIO_GPU_CMD_RESOURCE_FLUSH: u32 = 0x0104;
pub const VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D: u32 = 0x0105;
pub const VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING: u32 = 0x0106;
pub const VIRTIO_GPU_RESP_OK_NODATA: u32 = 0x1100;
pub const VIRTIO_GPU_RESP_OK_DISPLAY_INFO: u32 = 0x1101;
