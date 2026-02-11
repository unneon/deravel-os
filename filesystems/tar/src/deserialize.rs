use crate::sectors::SequentialSectors;
use crate::{File, SECTOR_SIZE, TarHeaderBuf};
use alloc::borrow::ToOwned;
use alloc::vec;
use alloc::vec::Vec;
use core::ffi::CStr;

pub fn deserialize_archive() -> Vec<File> {
    let mut buf = TarHeaderBuf {
        bytes: [0; SECTOR_SIZE],
    };
    let mut files = Vec::new();
    let mut sectors = SequentialSectors::new();
    while !sectors.is_finished() {
        // Hopefully one day Rust will get safe transmute and something this simple will
        // become nice and safe. The alternative is error-prone indexing with prefix
        // sums or a macro.
        sectors.read(unsafe { &mut buf.bytes });
        let header = unsafe { &buf.header };

        // TODO: Shouldn't it be two consecutive zero-filled blocks?
        if header.name[0] == b'\0' {
            break;
        }

        assert_eq!(header.magic, *b"ustar\0");

        // TODO: Check it's a valid UNIX path with no weird components.
        let name = parse_string(&header.name).to_owned();
        let size = parse_octal(&header.size);

        let mut data = vec![0u8; size.next_multiple_of(SECTOR_SIZE)];
        for i in 0..size.div_ceil(SECTOR_SIZE) {
            let sector_data = &mut data[i * SECTOR_SIZE..][..SECTOR_SIZE];
            sectors.read(sector_data.try_into().unwrap());
        }

        files.push(File { name, data, size });
    }
    files
}

fn parse_string(raw: &[u8]) -> &str {
    CStr::from_bytes_until_nul(raw).unwrap().to_str().unwrap()
}

fn parse_octal(raw: &[u8]) -> usize {
    usize::from_str_radix(parse_string(raw), 8).unwrap()
}
