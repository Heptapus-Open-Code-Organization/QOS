//! Bounded boot and Ring-3 smoke checks. Persistence and full process tests remain open.

fn check(ok: bool, name: &str) {
    if !ok {
        crate::serial_println!("QOS_VERIFY FAIL {}", name);
        crate::qemu::exit(0x11);
    }
    crate::serial_println!("QOS_VERIFY PASS {}", name);
}

pub fn run() -> ! {
    crate::serial_println!("QOS_VERIFY BEGIN v4");
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
    check(crate::uaccess::verify_mappings(), "user-copy");
    check(
        verify_ring3(crate::user::spawn_ring3_regabi, 2),
        "ring3-write-exit",
    );
    crate::security::reset_entry_probe();
    let probe = verify_ring3(crate::user::spawn_ring3_entry_probe, 12);
    check(
        probe && crate::security::entry_probe_passed(),
        "entry-policy",
    );
    check(
        verify_ring3(crate::user::spawn_ring3_abi_probe, 10),
        "abi-contract",
    );
    crate::serial_println!("QOS_VERIFY COMPLETE v4");
    crate::qemu::exit(0x10);
}

fn verify_ring3(spawn: fn() -> crate::user::Ring3Handle, minimum_calls: u64) -> bool {
    use core::sync::atomic::Ordering;
    crate::kthread::reset();
    let user = spawn();
    crate::kthread::adopt_user(user.saved_rsp, user.cr3, user.rsp0_top);
    let before = crate::syscall::SYSCALL_COUNT.load(Ordering::Relaxed);
    crate::kthread::arm();
    let start = crate::interrupts::TICKS.load(Ordering::Relaxed);
    while !crate::kthread::all_finished()
        && crate::interrupts::TICKS
            .load(Ordering::Relaxed)
            .wrapping_sub(start)
            < 200
    {
        x86_64::instructions::interrupts::enable_and_hlt();
    }
    let passed = crate::kthread::all_finished()
        && crate::syscall::SYSCALL_COUNT.load(Ordering::Relaxed) >= before + minimum_calls;
    crate::kthread::disarm();
    crate::kthread::reset();
    crate::memory::switch_cr3(user.cr3);
    let root = crate::user::cleanup_user_addrspace_and_get_cr3();
    crate::memory::switch_to_kernel_cr3();
    crate::memory::with_ctx(|_, allocator| allocator.deallocate_frame(root));
    crate::user::clear_ring3_test_stacks();
    passed
}
