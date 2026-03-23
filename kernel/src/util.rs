pub mod volatile;

pub macro forward_fmt($(impl $($trait:ident),* for $type:ty as $f:ident;)*) {
    $($(impl core::fmt::$trait for $type {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            core::fmt::$trait::fmt(<$type>::$f(self), f)
        }
    })*)*
}
