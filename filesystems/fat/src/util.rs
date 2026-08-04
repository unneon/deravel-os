use core::fmt::Write;

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(transparent)]
pub struct ArrayCStr<const N: usize>(pub [u8; N]);

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Padding<const N: usize>([u8; N]);

impl<const N: usize> core::fmt::Debug for ArrayCStr<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_char('"')?;
        for chunk in self.0.utf8_chunks() {
            for ch in chunk.valid().chars() {
                write!(f, "{}", ch.escape_debug())?;
            }
            for byte in chunk.invalid() {
                write!(f, "\\x{byte:02X}")?;
            }
        }
        f.write_char('"')?;
        Ok(())
    }
}

impl<const N: usize> core::fmt::Debug for Padding<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_char('_')
    }
}
