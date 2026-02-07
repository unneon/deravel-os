fn main() {
    println!("cargo::rerun-if-changed=kernel.ld");
    println!("cargo::rustc-link-arg=-Tkernel/kernel.ld");
}
