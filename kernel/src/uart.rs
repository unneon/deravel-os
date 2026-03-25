use crate::util::volatile::{Volatile, volatile_struct};
use log::info;

volatile_struct! { pub Uart16550Mmio
    rbr_thr_dll: ReadWrite u8,
    ier_dlm: ReadWrite u8,
    iir_fcr: ReadWrite u8,
    lcr: ReadWrite u8,
    mcr: ReadWrite u8,
    lsr: ReadWrite u8,
    msr: ReadWrite u8,
    scr: ReadWrite u8,
}

pub struct Uart16550 {
    bar: Volatile<Uart16550Mmio>,
}

impl Uart16550 {
    pub fn new(bar: Volatile<Uart16550Mmio>) -> Uart16550 {
        info!("found UART 16550");
        bar.ier_dlm().write(0x00);
        bar.lcr().write(0x80);
        bar.rbr_thr_dll().write(0x01);
        bar.ier_dlm().write(0x00);
        bar.lcr().write(0x03);
        bar.iir_fcr().write(0xC7);
        bar.mcr().write(0x03);
        Uart16550 { bar }
    }

    pub fn demo(&mut self) {
        for c in "Hello, world!\n".bytes() {
            self.putc(c);
        }
    }

    fn putc(&mut self, c: u8) {
        while self.bar.lsr().read() & (1 << 5) == 0 {}
        self.bar.rbr_thr_dll().write(c);
    }
}
