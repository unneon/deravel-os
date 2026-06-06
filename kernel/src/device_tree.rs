use fdt::Fdt;

pub fn find_timebase_frequency(device_tree: &Fdt) -> Option<usize> {
    device_tree
        .find_node("/cpus")?
        .property("timebase-frequency")?
        .as_usize()
}
