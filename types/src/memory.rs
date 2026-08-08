use crate::{LEVEL_2_PAGE_SIZE, PAGE_SIZE, PAGE_TABLE_ENTRY_COUNT};
use core::ops::Range;

pub const USER_INPUTS: Range<usize> = 0x1000..0x2000;

pub const USER_STACK_GUARD: Range<usize> =
    USER_STACK_REGION.start..USER_STACK_REGION.start + PAGE_SIZE;

pub const USER_STACK: Range<usize> = USER_STACK_GUARD.end..USER_STACK_REGION.end;

const USER_STACK_REGION: Range<usize> = 0x2000..0x1_0000;

pub const USER_ELF: Range<usize> = 0x100_0000..0x200_0000;

pub const USER_CAPABILITIES: Range<usize> = 0x200_0000..0x400_0000;

pub const USER_HEAP: Range<usize> = 0x400_0000..0x8000_0000;

pub const PHYSICAL_ADDRESSES: Range<usize> = 0..VIRTUAL_ADDRESSES.end / 2;

pub const DIRECT_MAPPING: Range<usize> = VIRTUAL_ADDRESSES.end / 2..VIRTUAL_ADDRESSES.end;

pub const VIRTUAL_ADDRESSES: Range<usize> = 0..PAGE_TABLE_ENTRY_COUNT * LEVEL_2_PAGE_SIZE;
