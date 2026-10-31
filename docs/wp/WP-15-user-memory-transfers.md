# WP-15: Checked user-memory transfer, first #40 slice

- Status: partial — bounded resident-copy slice implemented; broader #40 remains open
- Issue: [#40](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/40)
- ADR: [ADR-0023](../adr/0023-resident-user-memory-transfers.md)
- Branch: `feat/syscall-user-memory`

## Delivered scope

- Portable `no_std` range/four-level permission policy, limited to 4 KiB resident transfers
  under the current private P4[0] user region.
- Kernel `copy_from_user`/`copy_to_user` through validated supervisor aliases, bounded interrupt
  exclusion and full-range preflight; immutable physical-offset access without a spin lock.
- Register write, shared call-frame/submit/VFS buffers and extended file dispatcher use owned
  bounce buffers rather than unchecked user slices. Shared output requires writable mappings.
- The small preemptive Ring-3 payload loader uses the same checked copy path.
- Verification v2 tests real mappings/permission failures, shared ABI/VFS output, and a Ring-3
  register write/exit. It does not advertise full application resource cleanup or all syscalls.
- The subsequent entry-policy slice clears live DF/AC before Rust on all installed IDT gates,
  preserving hardware-saved flags. See [ADR-0024](../adr/0024-interrupt-entry-flags.md).

## Verification contract v2 additions

This section records the first increment. The current **v3** contract also requires the
entry-policy markers below; v2-only transcripts no longer satisfy the runner.

The v2 runner required BEGIN/COMPLETE **v2**, the existing boot/frame/W^X/Bell checks, and then:

```text
QOS_VERIFY PASS user-copy
hello from ring3 via register syscall ABI (int 0x81)
QOS_VERIFY PASS ring3-write-exit
QOS_VERIFY COMPLETE v2
```

After the ordinary Ring-3 write/exit check, v3 runs a separate Ring-3 payload with DF/AC set.
It exercises timer preemption, both software-interrupt ABIs, invalid-buffer rejection and
restoration of the user flags. The host requires:

```text
ring3 negative buffers and entry flags passed
QOS_VERIFY PASS entry-policy
QOS_VERIFY COMPLETE v3
```

BEGIN also changes to v3. Observers require all three entry paths to be exercised with dirty
saved user flags and clean live kernel flags. No exception/panic allowance is added.

`cargo os-verify` now defaults to `--cpu max`; `--cpu qemu64` exercises the baseline profile.
CI runs both independently. Artifacts and strict error/timeout behavior are unchanged from
WP-14. The max profile's serial CPU-hardening line records SMEP/SMAP support; do not infer that
every physical CPU supports them.

```powershell
cargo test -p qos-uaccess -p qos-verify
cargo check -p qos-uaccess --target x86_64-unknown-none
cargo os-verify --cpu max
cargo os-verify --skip-build --cpu qemu64
```

## Remaining acceptance

Local evidence (2026-10-31): 81 host tests plus one doctest passed; `qos-uaccess` built for
`x86_64-unknown-none`; the interactive UEFI image built. Full max and qemu64 UEFI runs both
completed v2 with exit 33. The max run logged NX/WP/SMEP/SMAP on. Mapping checks also exercised
read-only shared-call-frame rejection and the extended dispatcher's invalid-buffer errors.
Artifact examples: `target/qos-verify/run-39568-1793457553944312700` (max) and
`target/qos-verify/run-7852-1793457562676109700` (qemu64).

- [ ] Address-space locks/frame pinning and concurrent unmap/CoW/demand-paging behavior before
  corresponding #7/#20 SMP/VM features are enabled.
- [x] Normalize live DF/AC at installed IDT entries; prove timer/shared/register paths and
  user flag restoration on max and qemu64 (v3, ADR-0024).
- [ ] Remaining legacy loader/user-access audit and uninstalled exception/NMI entry coverage.
- [ ] Unified ABI/error/version contract and reachable extended syscall ownership semantics.
- [ ] Full adversarial Ring-3 buffer corpus, repeated process resource accounting, blocking and
  cancellation integration as #41 matures.
- [ ] PR/CI evidence and review before integrating this branch into main.

This first slice rejects invalid resident ranges before accessing them; it does not implement
a general kernel page-fault recovery table or establish production security.

Local v3 evidence: both CPU profiles completed all markers with exit 33; max artifact
`target/qos-verify/run-28712-1793464414102986000`, qemu64 artifact
`target/qos-verify/run-6812-1793464420426080500`.
