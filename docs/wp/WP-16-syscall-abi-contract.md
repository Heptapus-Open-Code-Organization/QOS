# WP-16: Current syscall ABI contract

- Status: partial; current v1 contract/discovery and response semantics implemented
- Issues: #40, #25; coordinates with #15 and #41
- ADR: [ADR-0025](../adr/0025-current-syscall-abi-contract.md)
- Branch: `feat/syscall-abi-contract`
- Interface reference: [SYSCALL-ABI](../SYSCALL-ABI.md)

## Delivered scope

- Canonical register v1 numbers, generic error, write bound and additive version query in
  `qos-abi::reg`; shared frame address/size also belong to `qos-abi`.
- Safe little-endian decoding of checked 48-byte call snapshots and response-only encoding,
  with compile-time checks for all shared field offsets and golden-wire host tests.
- Shared dispatch initializes error status and zero results before version/op handling;
  operation-specific responses then override those defaults. Unused success fields are zero.
- The normal preemptive shared EXIT path is exercised in verification builds. It reaps the
  current process, not QEMU. The outer verifier alone reports suite success.
- The unused conflicting legacy enum was removed. The internal extended file dispatcher
  stays unwired pending process ownership and ABI migration.

## Current verification contract v4

All previous memory/entry checks remain required. BEGIN/COMPLETE advance to v4, and after
`QOS_VERIFY PASS entry-policy` the runner additionally requires, in order:

```text
ring3 ABI contract passed
user exit: OP_EXIT (code=0) -> process reaped; kernel survives
QOS_VERIFY PASS abi-contract
QOS_VERIFY COMPLETE v4
```

An independent Ring-3 client uses literal v1 numbers/layout. It tests version discovery,
unknown register call, representative GPR/argument preservation, zero-length writes with
null/noncanonical pointers, bad shared version (including EXIT), unknown shared op/handle,
cleared stale results, preserved request/reserved bytes, successful query and actual shared
EXIT. Returning from EXIT triggers UD2; any unexpected exception fails verification.

Local evidence (2026-10-31): **83 host unit tests and one doctest passed**, with the same
ignored subprocess fixture used by the timeout test. `cargo check -p qos-abi --target
x86_64-unknown-none` and the normal `cargo image` build passed. Both UEFI profiles completed
v4 with exit 33; max logged SMEP/SMAP on:

- max: `target/qos-verify/run-39036-1793465038754349300`
- qemu64: `target/qos-verify/run-33764-1793465125708168700`

```powershell
cargo test -p qos-abi -p qos-gfx -p qos-ui -p qos-driver -p qos-acpi -p qos-core -p qos-verify -p qos-uaccess --features qos-core/std
cargo check -p qos-abi --target x86_64-unknown-none
cargo image
cargo os-verify --cpu max
cargo os-verify --skip-build --cpu qemu64
```

## Remaining acceptance

- [ ] Review/publication and Linux CI evidence for this increment.
- [ ] Detailed error/version migration and a minimal external application SDK.
- [ ] Process-owned resources, jobs/FDs, blocking/cancellation and repeated-process cleanup.
- [ ] Legacy loader/access audit, foreground execution/exit migration and general fault fixup.
- [ ] Address-space locking/frame pinning before concurrent unmap/CoW/SMP; uninstalled
  exception/NMI policy and broader adversarial tests.

These are the existing #40/#7/#15/#41 boundaries, not a change in execution priority.
