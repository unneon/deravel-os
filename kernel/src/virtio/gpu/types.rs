pub unsafe trait Response: Default {
    const TYPE: CtrlType;
    fn hdr(&self) -> &CtrlHdr;
}

#[derive(Default)]
#[repr(C)]
pub struct CtrlHdr {
    pub type_: u32,
    pub flags: u32,
    pub fence_id: u64,
    pub ctx_id: u32,
    pub ring_idx: u8,
    pub padding: [u8; 3],
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum CtrlType {
    CmdGetDisplayInfo = 0x0100,
    CmdResourceCreate2D,
    CmdResourceUnref,
    CmdSetScanout,
    CmdResourceFlush,
    CmdTransferToHost2D,
    CmdResourceAttachBacking,
    CmdResourceDetachBacking,
    CmdGetCapsetInfo,
    CmdGetCapset,
    CmdGetEdid,
    CmdResourceAssignUuid,
    CmdResourceCreateBlob,
    CmdSetScanoutBlob,

    CmdCtxCreate = 0x0200,
    CmdCtxDestroy,
    CmdCtxAttachResource,
    CmdCtxDetachResource,
    CmdResourceCreate3D,
    CmdTransferToHost3D,
    CmdTransferFromHost3D,
    CmdSubmit3D,
    CmdResourceMapBlob,
    CmdResourceUnmapBlob,

    CmdUpdateCursor = 0x0300,
    CmdMoveCursor,

    RespOkNodata = 0x1100,
    RespOkDisplayInfo,
    RespOkCapsetInfo,
    RespOkCapset,
    RespOkEdid,
    RespOkResourceUuid,
    RespOkMapInfo,

    RespErrUnspec = 0x1200,
    RespErrOutOfMemory,
    RespErrInvalidScanoutId,
    RespErrInvalidResourceId,
    RespErrInvalidContextId,
    RespErrInvalidParameter,
}

#[derive(Default)]
#[repr(C)]
pub struct DisplayOne {
    pub r: Rect,
    pub enabled: u32,
    pub flags: u32,
}

#[allow(dead_code)]
#[derive(Debug)]
#[repr(u32)]
pub enum Error {
    Unspec = 0x1200,
    OutOfMemory,
    InvalidScanoutId,
    InvalidResourceId,
    InvalidContextId,
    InvalidParameter,
}

#[allow(dead_code, clippy::enum_variant_names)]
#[repr(u32)]
pub enum Format {
    B8G8R8A8Unorm = 1,
    B8G8R8X8Unorm = 2,
    A8R8G8B8Unorm = 3,
    X8R8G8B8Unorm = 4,
    R8G8B8A8Unorm = 67,
    X8B8G8R8Unorm = 68,
    A8B8G8R8Unorm = 121,
    R8G8B8X8Unorm = 134,
}

#[repr(C)]
pub struct MemEntry {
    pub addr: u64,
    pub length: u32,
    pub padding: u32,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
pub struct ResourceAttachBacking {
    pub hdr: CtrlHdr,
    pub resouce_id: u32,
    pub nr_entries: u32,
}

#[repr(C)]
pub struct ResourceCreate2D {
    pub hdr: CtrlHdr,
    pub resource_id: u32,
    pub format: u32,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
pub struct ResourceFlush {
    pub hdr: CtrlHdr,
    pub r: Rect,
    pub resource_id: u32,
    pub padding: u32,
}

#[derive(Default)]
#[repr(C)]
pub struct ResponseDisplayInfo {
    pub hdr: CtrlHdr,
    pub pmodes: [DisplayOne; MAX_SCANOUTS],
}

#[derive(Default)]
#[repr(C)]
pub struct ResponseNodata {
    pub hdr: CtrlHdr,
}

#[repr(C)]
pub struct SetScanout {
    pub hdr: CtrlHdr,
    pub r: Rect,
    pub scanout_id: u32,
    pub resource_id: u32,
}

#[repr(C)]
pub struct TransferToHost2D {
    pub hdr: CtrlHdr,
    pub r: Rect,
    pub offset: u64,
    pub resource_id: u32,
    pub padding: u32,
}

pub const MAX_SCANOUTS: usize = 16;

impl CtrlType {
    pub fn header(self) -> CtrlHdr {
        CtrlHdr {
            type_: self as u32,
            flags: 0,
            fence_id: 0,
            ctx_id: 0,
            ring_idx: 0,
            padding: [0; _],
        }
    }
}

unsafe impl Response for ResponseDisplayInfo {
    const TYPE: CtrlType = CtrlType::RespOkDisplayInfo;

    fn hdr(&self) -> &CtrlHdr {
        &self.hdr
    }
}

unsafe impl Response for ResponseNodata {
    const TYPE: CtrlType = CtrlType::RespOkNodata;

    fn hdr(&self) -> &CtrlHdr {
        &self.hdr
    }
}

impl PartialEq<CtrlType> for u32 {
    fn eq(&self, other: &CtrlType) -> bool {
        *self == *other as u32
    }
}
