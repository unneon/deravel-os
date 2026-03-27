use crate::sectors::SequentialSectors;
use crate::{File, SECTOR_SIZE, TarHeaderBuf};
use deravel_kernel_api::{Capability, Drive};

pub fn serialize_archive(files: &[File], drive: Capability<Drive>) {
    let mut sectors = SequentialSectors::new(drive);
    for file in files {
        let mut buf = TarHeaderBuf {
            bytes: [0; SECTOR_SIZE],
        };
        let header = unsafe { &mut buf.header };

        assert!(file.name.len() < header.name.len());
        header.name[..file.name.len()].copy_from_slice(file.name.as_bytes());
        header.mode = *b"000644\0\0";
        header.magic = *b"ustar\0";
        header.version = *b"00";
        header.type_ = b'0';
        header.size = to_octal(file.size);

        let checksum = calculate_checksum(unsafe { &buf.bytes });
        buf.header.checksum = to_octal(checksum);

        sectors.write(unsafe { &buf.bytes });
        for i in 0..file.data.len() / SECTOR_SIZE {
            let sector_data = &file.data[i * SECTOR_SIZE..][..SECTOR_SIZE];
            sectors.write(sector_data.try_into().unwrap());
        }
    }
}

fn calculate_checksum(bytes: &[u8]) -> usize {
    let mut checksum = b' ' as usize * 8;
    for &byte in bytes {
        checksum += byte as usize;
    }
    checksum
}

fn to_octal<const N: usize>(mut number: usize) -> [u8; N] {
    let mut buf = [0; N];
    for i in 0..buf.len() {
        buf[buf.len() - i - 1] = b'0' + (number % 8) as u8;
        number /= 8;
    }
    assert_eq!(number, 0);
    buf
}
