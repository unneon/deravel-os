use core::num::NonZeroUsize;
use core::sync::atomic::{AtomicUsize, Ordering};
use fdt::Fdt;
use log::warn;

static TIMEBASE_FREQUENCY: AtomicUsize = AtomicUsize::new(0);

pub fn timebase_frequency() -> Option<NonZeroUsize> {
    NonZeroUsize::new(TIMEBASE_FREQUENCY.load(Ordering::Relaxed))
}

pub fn initialize_timebase_frequency(device_tree: &Fdt) {
    let Some(timebase_frequency) = find_timebase_frequency(device_tree) else {
        return warn!("CPU timebase frequency missing");
    };
    TIMEBASE_FREQUENCY.store(timebase_frequency, Ordering::Relaxed);
}

fn find_timebase_frequency(device_tree: &Fdt) -> Option<usize> {
    device_tree
        .find_node("/cpus")?
        .property("timebase-frequency")?
        .as_usize()
}
