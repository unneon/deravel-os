pub macro features($driver:ident $struct:ident $base:literal $($has_name:ident $enable_name:ident $bit:literal)*) {
    #[derive(Default)]
    pub struct $struct(u32);

    impl $struct {
        $(pub fn $has_name(&self) -> bool {
            self.0 & (1 << $bit) != 0
        }

        pub fn $enable_name(&mut self) {
            self.0 |= 1 << $bit;
        })*
    }

    impl From<$struct> for u32 {
        fn from(features: $struct) -> u32 {
            features.0
        }
    }
}

pub const STATUS_ACKNOWLEDGE: u32 = 1;
pub const STATUS_DRIVER: u32 = 2;
pub const STATUS_DRIVER_OK: u32 = 4;
