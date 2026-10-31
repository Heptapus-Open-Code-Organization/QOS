# ADR-0025: Preserve v1 syscall namespaces and define deterministic shared responses

- Status: Accepted for the current preemptive user ABI
- Date: 2026-10-31
- Related: #40, #15, #41; follows ADR-0023 and ADR-0024

## Context

Register int 0x81 and shared-frame int 0x80 already have callers and incompatible number
tables. An unwired extended file dispatcher has a third table/global FD state. Treating it
as the current register ABI would silently change EXIT/GETTICKS semantics. Shared response
fields were previously left with caller-supplied/stale values when handlers did not set them.
The verification-only shared EXIT also ended QEMU rather than exercising normal process exit.

## Decision

Preserve existing v1 numbers and shared wire layout. Publish register constants in `qos-abi`
and add call 3 for register version discovery; older kernels report their existing generic
unsupported-call result. Retain the v1 generic register error and shared OK/ERR conventions.
Document current behavior and limitations in `docs/SYSCALL-ABI.md` before further migration.

Decode checked shared snapshots explicitly as little-endian integers. Initialize status=ERR
and both results to zero on every dispatch before validating version or operating on the
request. Encode only response fields into the original snapshot, preserving request bytes.
Operation-specific results may override the initialized values. Existing reserved bytes
remain ignored/preserved for compatibility. Compile-time checks fix every call-frame offset.

Use the production preemptive shared EXIT implementation in verification builds. Only the
outer verifier may signal successful QEMU completion, after observing process completion.
Leave the internal extended dispatcher unwired until resource ownership and migration are
defined. No POSIX compatibility, complete SDK, quantum job correctness or production security
is established by this increment.

## Verification and consequences

Golden-byte host tests cover decoding and response-only encoding. The v4 UEFI suite runs an
independent Ring-3 wire client through register discovery/errors, empty transfers, shared
version/operation/handle errors, stale outputs, input preservation, successful query and
nonreturning shared EXIT. Prior DF/AC and memory-permission tests remain required.

Detailed errno migration, legacy foreground exit/loaders, resource ownership, SMP/VM pinning
and uninstalled exception handling remain separate work. Any future breaking change requires
a versioned contract and migration; transcript v4 does not change the user ABI version.
