use crate::sync::Mutex;
use fdt::Fdt;

static TIMEBASE_FREQUENCY: Mutex<f64> = Mutex::new(f64::NAN);

pub fn timebase_frequency() -> f64 {
    *TIMEBASE_FREQUENCY.lock()
}

pub fn initialize_timebase_frequency(device_tree: &Fdt) {
    *TIMEBASE_FREQUENCY.lock() = find_timebase_frequency(device_tree).unwrap() as f64;
}

fn find_timebase_frequency(device_tree: &Fdt) -> Option<usize> {
    device_tree
        .find_node("/cpus")?
        .property("timebase-frequency")?
        .as_usize()
}
