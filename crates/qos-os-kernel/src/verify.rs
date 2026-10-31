//! Bounded boot smoke checks. This does not stand in for Ring-3 or persistence tests.

fn check(ok: bool, name: &str) {
    if !ok {
        crate::serial_println!("QOS_VERIFY FAIL {}", name);
        crate::qemu::exit(0x11);
    }
    crate::serial_println!("QOS_VERIFY PASS {}", name);
}

pub fn run() -> ! {
    crate::serial_println!("QOS_VERIFY BEGIN v1");
    check(crate::framebuffer::active(), "boot-framebuffer");
    let (wx, mapped) = crate::security::wx_audit();
    check(wx == 0 && mapped > 0, "wx");
    check(crate::compositor::verify_frame(), "desktop-frame");

    // Use the real engine used by the modern desktop, not the legacy verify-mode ABI mock.
    crate::quantum::sim::seed_rng(0x853c49e6748fea9b);
    let result = crate::quantum::sim::run_qasm2(
        b"OPENQASM 2.0;\ninclude \"qelib1.inc\";\nqreg q[2];\ncreg c[2];\nh q[0];\ncx q[0],q[1];\nmeasure q[0] -> c[0];\nmeasure q[1] -> c[1];\n",
        256,
    );
    let ok = result
        .map(|r| {
            r.shots == 256
                && r.n_qubits == 2
                && r.counts.len() == 2
                && r.counts.get("00").copied().unwrap_or(0) > 0
                && r.counts.get("11").copied().unwrap_or(0) > 0
                && r.counts.values().sum::<u64>() == 256
        })
        .unwrap_or(false);
    check(ok, "quantum-bell");
    crate::serial_println!("QOS_VERIFY COMPLETE v1");
    crate::qemu::exit(0x10);
}
