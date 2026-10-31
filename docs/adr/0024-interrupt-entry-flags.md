# ADR-0024: Normalize live DF/AC before kernel Rust entry

- Status: Accepted for currently installed IDT gates
- Date: 2026-10-31
- Related: #40, #25; extends ADR-0023's entry-policy follow-up

## Context

Ring-3 code can arrive at a syscall or timer interrupt with DF/AC set. Kernel Rust code
requires a forward direction flag; SMAP access policy also depends on the live AC flag.
Sanitizing inside an ordinary Rust body is too late to establish its entry preconditions.
The hardware-saved RFLAGS must remain intact so a normal interrupt return preserves the
interrupted context.

## Decision

Before saving/calling into Rust, both raw timer and register-syscall entries use a temporary
PUSHFQ/AND/POPFQ sequence to clear live DF (bit 10) and AC (bit 18). It leaves every GPR,
the CPU-saved frame and current interrupt-enable state unchanged. This does not require
CLAC/SMAP instruction support and is tested on qemu64 as well as max.

Every currently installed compiler-generated `extern x86-interrupt` handler is reached through
an assembly gate using the same sequence. It tail-jumps to the original handler rather than
calling it: no return address, error-code adjustment or stack-layout change is introduced.
Double-fault IST configuration and Ring-3 DPL for software interrupts remain unchanged.
Future IDT entries must use this policy or document an equivalent safe entry implementation.

The saved frame is restored by IRETQ. User DF/AC can therefore be set again after returning
to Ring 3; this is intentional and distinct from the normalized live kernel flags. Supervisor
alias copies from ADR-0023 do not need to reopen AC access windows.

## Verification

The v3 UEFI suite executes an immutable assembly payload in Ring 3. It sets both flags,
waits across timer preemption, uses int 0x81 and int 0x80, checks expected invalid-buffer
errors, verifies user flag restoration after returning calls, prints its completion marker
and exits. Verification-only entry observers require clean live flags and recorded dirty
user flags on all three paths (register syscall, shared syscall, timer). Missing coverage,
an unexpected fault, wrong output or incomplete marker sequence fails the runner.

Negative buffers include null, noncanonical/out-of-user-region, arithmetic overflow,
supervisor-only, cross-unmapped-page and oversized ranges. The register frame's relevant
offsets and total size are compile-time asserted. These tests do not replace a full audit
of all exception/error paths, uninstalled vectors, NMI handling, legacy loaders or SMP/VM
concurrency and general fault recovery.

## References

- [Rust reference: x86 direction-flag rules](https://doc.rust-lang.org/reference/inline-assembly.html#asm-rules-x86-df).
- [Intel SDM documentation: CLAC and AC/SMAP behavior](https://software.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developers-manual.pdf).
