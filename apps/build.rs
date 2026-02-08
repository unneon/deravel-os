fn main() {
    println!("cargo::rerun-if-changed=../kernel-api/user.ld");
    println!("cargo::rustc-link-arg=-Tkernel-api/user.ld");
}
