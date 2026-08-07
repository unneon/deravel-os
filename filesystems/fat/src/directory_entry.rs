use alloc::string::String;
use core::mem::MaybeUninit;

#[derive(Clone, Copy)]
#[repr(C)]
pub union DirectoryEntry {
    pub long: LongNameDirectoryEntry,
    pub short: ShortNameDirectoryEntry,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct ShortNameDirectoryEntry {
    pub name: [u8; 11],
    pub attr: u8,
    nt_res: [u8; 1],
    pub crt_time_tenth: u8,
    pub crt_time: u16,
    pub crt_date: u16,
    pub lst_acc_date: u16,
    pub fst_clus_hi: u16,
    pub wrt_time: u16,
    pub wrt_date: u16,
    pub fst_clus_lo: u16,
    pub file_size: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct LongNameDirectoryEntry {
    pub ord: u8,
    pub name1: [u16; 5],
    pub attr: u8,
    pub type_: u8,
    pub chksum: u8,
    pub name2: [u16; 6],
    pub fst_clus_lo: u16,
    pub name3: [u16; 2],
}

const _: () = assert!(size_of::<DirectoryEntry>() == 32);
const _: () = assert!(size_of::<ShortNameDirectoryEntry>() == 32);
const _: () = assert!(size_of::<LongNameDirectoryEntry>() == 32);

const ATTR_READ_ONLY: u8 = 0x01;
const ATTR_HIDDEN: u8 = 0x02;
const ATTR_SYSTEM: u8 = 0x04;
const ATTR_VOLUME_ID: u8 = 0x08;
const ATTR_DIRECTORY: u8 = 0x10;
const ATTR_ARCHIVE: u8 = 0x20;

const ATTR_LONG_NAME: u8 = ATTR_READ_ONLY | ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME_ID;
const ATTR_LONG_NAME_MASK: u8 =
    ATTR_READ_ONLY | ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME_ID | ATTR_DIRECTORY | ATTR_ARCHIVE;

const LAST_LONG_ENTRY: u8 = 0x40;

const MAX_LONG_NAME_RESULT_LENGTH: usize = 255;
const MAX_LONG_NAME_BUFFER_LENGTH: usize = MAX_LONG_NAME_ENTRY_LENGTH * MAX_LONG_NAME_ENTRIES;
const MAX_LONG_NAME_ENTRY_LENGTH: usize = 13;
const MAX_LONG_NAME_ENTRIES: usize =
    MAX_LONG_NAME_RESULT_LENGTH.div_ceil(MAX_LONG_NAME_ENTRY_LENGTH);

const UTF16_PERIOD: u16 = b'.' as u16;
const UTF16_SPACE: u16 = b' ' as u16;

pub fn coalesce_long_names<'a>(
    mut entries: impl Iterator<Item = DirectoryEntry> + 'a,
) -> impl Iterator<Item = (ShortNameDirectoryEntry, Option<String>)> + 'a {
    core::iter::from_fn(move || {
        let mut entry = entries.next()?;
        let long_name = if unsafe { entry.short.attr } & ATTR_LONG_NAME_MASK == ATTR_LONG_NAME {
            let last = unsafe { &entry.long };
            assert_ne!(last.ord & LAST_LONG_ENTRY, 0);
            let long_entry_count = last.ord ^ LAST_LONG_ENTRY;
            assert!((1..=MAX_LONG_NAME_ENTRIES).contains(&(long_entry_count as usize)));

            let checksum = last.chksum;

            let mut long_name_buf = [MaybeUninit::uninit(); MAX_LONG_NAME_BUFFER_LENGTH];
            let long_name = long_name_buf[..long_entry_count as usize * MAX_LONG_NAME_ENTRY_LENGTH]
                .write_filled(0);
            for (i, chunk) in long_name.as_chunks_mut().0.iter_mut().enumerate().rev() {
                let long = unsafe { &entry.long };
                if (i as u8) < long_entry_count - 1 {
                    assert_eq!(long.ord, i as u8 + 1);
                }
                assert_eq!(long.chksum, checksum);
                copy_long_cps(long, chunk);
                entry = entries.next().unwrap();
            }

            assert_eq!(checksum, compute_checksum(unsafe { &entry.short.name }));

            let mut long_name = long_name as &[u16];
            trim_long_name(&mut long_name);
            for cp in long_name {
                assert!(is_valid_long_char(*cp));
            }
            assert!(long_name.len() <= MAX_LONG_NAME_RESULT_LENGTH);
            Some(String::from_utf16(long_name).unwrap())
        } else {
            None
        };
        Some((unsafe { entry.short }, long_name))
    })
}

fn copy_long_cps(long: &LongNameDirectoryEntry, out: &mut [u16; MAX_LONG_NAME_ENTRY_LENGTH]) {
    for (i, cp) in long
        .name1
        .into_iter()
        .chain(long.name2)
        .chain(long.name3)
        .enumerate()
    {
        out[i] = cp;
    }
}

fn compute_checksum(short_name: &[u8; 11]) -> u8 {
    let mut sum = 0;
    for byte in short_name {
        sum = if sum & 1 != 0 { 0x80 } else { 0 } + (sum >> 1) + byte;
    }
    sum
}

fn trim_long_name(long_name: &mut &[u16]) {
    while let Some(&UTF16_SPACE) = long_name.first() {
        *long_name = &long_name[1..];
    }
    if let Some((null_i, _)) = long_name.iter().enumerate().find(|(_, cp)| **cp == 0) {
        assert!(long_name[null_i + 1..].iter().all(|cp| *cp == 0xFFFF));
        *long_name = &long_name[..null_i];
    }
    while let Some(&UTF16_SPACE | &UTF16_PERIOD) = long_name.last() {
        *long_name = &long_name[..long_name.len() - 1];
    }
}

pub fn to_short_name(s: &str) -> Option<[u8; 11]> {
    let (main_part, extension) = s.split_once('.').unwrap_or((s, ""));
    if main_part.len() > 8 {
        return None;
    }
    if extension.len() > 3 {
        return None;
    }

    let mut name = [b' '; _];
    for (i, byte) in main_part.bytes().enumerate() {
        let byte = byte.to_ascii_uppercase();
        if !is_valid_short_char(byte) {
            return None;
        }
        name[i] = byte;
    }
    for (i, byte) in extension.bytes().enumerate() {
        let byte = byte.to_ascii_uppercase();
        if !is_valid_short_char(byte) {
            return None;
        }
        name[8 + i] = byte;
    }
    Some(name)
}

fn is_valid_short_char(byte: u8) -> bool {
    matches!(byte, b'A'..=b'Z' | b'0'..=b'9' | 128.. | b'$' | b'%' | b'\'' | b'-' | b'_' | b'@' | b'~' | b'`' | b'!' | b'(' | b')' | b'{' | b'}' | b'^' | b'#' | b'&' | b' ')
}

fn is_valid_long_char(cp: u16) -> bool {
    if cp >= 128 {
        return true;
    }
    let byte = cp as u8;
    is_valid_short_char(byte)
        || matches!(byte, b'a'..=b'z' | b'.' | b',' | b';' | b'=' | b'[' | b']')
}
