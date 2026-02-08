#![no_std]
#![no_main]

use deravel_kernel_api::{Capability, CapabilityExport, app, pid, pid_by_name, println, yield_};

fn main() {
    println!("A launched with pid {}", pid());
    let cap = Capability::create(pid_by_name("ipc-b"));
    println!("A created capability {cap:?} for B");
    println!("A sent {cap:?} to B (todo)");
    println!("A goes to sleep");
    yield_();
    println!("A wakes up");
    let req = Capability::guess(0x2001000);
    println!("A is queried by C with {req:?} (todo)");
    println!("A verifies the capability is within 0x2______ (todo)");
    let exp1 = req.read().unpack();
    println!("A reads the capability export: {exp1:?}");
    let CapabilityExport::Redirect { dst_pid, inner } = exp1 else {
        unreachable!()
    };
    println!("A sees it's a redirect, so it checks the pid matches with C");
    assert_eq!(dst_pid, pid_by_name("ipc-c"));
    println!("A verified the redirect was allowed, so it proceeds to the next link");
    let exp2 = inner.read().unpack();
    println!("A reads the second capability export: {exp2:?}");
    let CapabilityExport::Internal { dst_pid } = exp2 else {
        unreachable!()
    };
    println!("A sees it's an internal, so it checks the pid matches last src_pid (B)");
    assert_eq!(dst_pid, req.src_pid());
    println!("A has verified the chain, so now it checks the cap is its own");
    assert_eq!(inner.src_pid(), pid());
    println!("A completes C's request for capability {inner:?} (traversed from {req:?})");
}

app! { main }
