use alloc::string::String;

#[derive(Clone, Copy)]
#[repr(C)]
pub union DirectoryEntry {
    long: LongNameDirectoryEntry,
    short: ShortNameDirectoryEntry,
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
    fst_clus_hi: u16,
    pub wrt_time: u16,
    pub wrt_date: u16,
    fst_clus_lo: u16,
    pub file_size: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
struct LongNameDirectoryEntry {
    ord: u8,
    name1: [u16; 5],
    attr: u8,
    type_: u8,
    chksum: u8,
    name2: [u16; 6],
    fst_clus_lo: u16,
    name3: [u16; 2],
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

const MAX_LONG_RESULT_LENGTH: usize = 255;
const MAX_LONG_BUFFER_LENGTH: usize = MAX_LONG_ENTRY_LENGTH * MAX_LONG_ENTRIES;
const MAX_LONG_ENTRY_LENGTH: usize = 13;
const MAX_LONG_ENTRIES: usize = MAX_LONG_RESULT_LENGTH.div_ceil(MAX_LONG_ENTRY_LENGTH);

const UTF16_PERIOD: u16 = b'.' as u16;
const UTF16_SPACE: u16 = b' ' as u16;

impl ShortNameDirectoryEntry {
    pub fn fst_clus(&self) -> u32 {
        ((self.fst_clus_hi as u32) << 16) | self.fst_clus_lo as u32
    }
}

pub fn coalesce_long_names<'a>(
    mut entries: &'a [DirectoryEntry],
) -> impl Iterator<Item = (&'a ShortNameDirectoryEntry, Option<String>)> + 'a {
    core::iter::from_fn(move || {
        if let Some(long_count) = try_as_first_long(&entries[0]) {
            let longs = &entries[..long_count];
            let short = unsafe { &entries[long_count].short };
            entries = &entries[long_count..];

            let checksum = compute_checksum(&short.name);
            let mut name_buffer = [0; _];
            let name = copy_long(longs, checksum, &mut name_buffer);
            Some((short, Some(postprocess_long_name(name))))
        } else {
            let short = unsafe { &entries[0].short };
            entries = &entries[1..];
            Some((short, None))
        }
    })
}

fn try_as_first_long(entry: &DirectoryEntry) -> Option<usize> {
    let long = unsafe { &entry.long };
    if long.attr & ATTR_LONG_NAME_MASK != ATTR_LONG_NAME {
        return None;
    }
    assert_ne!(long.ord & LAST_LONG_ENTRY, 0);
    let long_count = (long.ord ^ LAST_LONG_ENTRY) as usize;
    assert!((1..=MAX_LONG_ENTRIES).contains(&long_count));
    Some(long_count)
}

fn compute_checksum(short_name: &[u8; 11]) -> u8 {
    let mut sum = 0;
    for &byte in short_name {
        sum = (if sum & 1 != 0 { 0x80u8 } else { 0 })
            .wrapping_add(sum >> 1)
            .wrapping_add(byte);
    }
    sum
}

fn ord(i: usize, long_count: usize) -> u8 {
    let mut ord = i as u8 + 1;
    if i == long_count - 1 {
        ord |= LAST_LONG_ENTRY;
    }
    ord
}

fn copy_long<'a>(
    longs: &[DirectoryEntry],
    checksum: u8,
    buffer: &'a mut [u16; MAX_LONG_BUFFER_LENGTH],
) -> &'a [u16] {
    let name = &mut buffer[..longs.len() * MAX_LONG_ENTRY_LENGTH];
    for (i, (long, chunk)) in longs.iter().rev().zip(name.as_chunks_mut().0).enumerate() {
        let long = unsafe { &long.long };
        assert_eq!(long.ord, ord(i, longs.len()));
        assert_eq!(long.chksum, checksum);
        copy_long_chunk(long, chunk);
    }
    name
}

fn copy_long_chunk(long: &LongNameDirectoryEntry, chunk: &mut [u16; MAX_LONG_ENTRY_LENGTH]) {
    for (i, cp) in long
        .name1
        .into_iter()
        .chain(long.name2)
        .chain(long.name3)
        .enumerate()
    {
        chunk[i] = cp;
    }
}

fn postprocess_long_name(mut name: &[u16]) -> String {
    while let Some(&UTF16_SPACE) = name.first() {
        name = &name[1..];
    }
    if let Some((null_i, _)) = name.iter().enumerate().find(|(_, cp)| **cp == 0) {
        assert!(name[null_i + 1..].iter().all(|cp| *cp == 0xFFFF));
        name = &name[..null_i];
    }
    while let Some(&UTF16_SPACE | &UTF16_PERIOD) = name.last() {
        name = &name[..name.len() - 1];
    }
    for cp in name {
        assert!(is_valid_long_char(*cp));
    }
    assert!(name.len() <= MAX_LONG_RESULT_LENGTH);
    String::from_utf16(name).unwrap()
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

fn is_valid_long_char(cp: u16) -> bool {
    if cp >= 128 {
        return true;
    }
    let byte = cp as u8;
    is_valid_short_char(byte)
        || matches!(byte, b'a'..=b'z' | b'.' | b',' | b';' | b'=' | b'[' | b']')
}

fn is_valid_short_char(byte: u8) -> bool {
    matches!(byte, b'A'..=b'Z' | b'0'..=b'9' | 128.. | b'$' | b'%' | b'\'' | b'-' | b'_' | b'@' | b'~' | b'`' | b'!' | b'(' | b')' | b'{' | b'}' | b'^' | b'#' | b'&' | b' ')
}
