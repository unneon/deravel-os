use crate::PAGE_SIZE;
use core::ops::Range;

pub const USER_INPUTS: Range<usize> = 0x1000..0x2000;

pub const USER_STACK: Range<usize> = USER_STACK_GUARD.end..USER_STACK_REGION.end;

pub const USER_STACK_GUARD: Range<usize> =
    USER_STACK_REGION.start..USER_STACK_REGION.start + PAGE_SIZE;

const USER_STACK_REGION: Range<usize> = 0x2000..0x1_0000;
