# Current x86-64 user ABI contracts

This describes the implemented interfaces, not a complete application SDK. Related: #40,
#15, #41. The two software-interrupt namespaces coexist; their operation numbers are not
interchangeable. Neither is a Linux/POSIX syscall ABI. Installed entries follow ADR-0024.

## Register interface: int 0x81, v1

The canonical constants are `qos_abi::reg`. RAX contains the call number and receives the
result. Arguments are assigned to RDI, RSI, RDX, R10, R8, R9 in that order. Returning calls
preserve the other GPRs and saved user RFLAGS. Callers must describe memory effects to their
compiler and must not mark the interrupt `nomem` or `readonly`.

| RAX | Call | Arguments and result |
|---|---|---|
| 0 | EXIT | Terminates the current preemptive user process; does not return. Exit-code/resource accounting is incomplete. |
| 1 | WRITE | RDI=pointer, RSI=length; writes serial text, returns length on success or `u64::MAX` on rejection. No file descriptor. |
| 2 | GETTICKS | Returns the kernel timer tick counter; not a wall-clock timestamp or calibrated duration API. |
| 3 | GET_ABI_VERSION | Returns register ABI version 1; additive discovery call. Older kernels return `u64::MAX`. |
| other | Unsupported | Returns `u64::MAX`; no extended file-call dispatch. |

WRITE is bounded to 4096 bytes. A zero length succeeds with zero regardless of pointer and
does not access it. Nonempty ranges must be resident 4 KiB user pages under the private P4[0]
region, with permissions at every paging level. The whole range is checked before output.
The current serial printer converts each input byte to a character; this is not yet a binary
stream/UTF-8 terminal contract. The v1 error result is a generic -1, not a detailed errno.

## Shared interface: int 0x80, v1

The canonical constants/layout are `qos_abi::shm`. The user maps a readable/writable call page
at `CALL_ADDR = 0x40010000` and fills the 48-byte little-endian frame below. No request pointer
is passed in registers. The kernel snapshots the frame through checked copies, decodes into
owned integers and writes only the response fields into that snapshot before copying it back.

| Offset | Size | Field | Direction |
|---|---|---|---|
| 0 | 4 | abi_version | input: 1 |
| 4 | 4 | op | input: operation number |
| 8 | 4 | status | output: 0=OK, 1=ERR |
| 12 | 4 | reserved | ignored/preserved by v1; new clients should initialize to zero |
| 16 | 8 | ret0 | output |
| 24 | 8 | ret1 | output |
| 32 | 8 | arg0 | input |
| 40 | 8 | arg1 | input |

Every dispatch initializes status=ERR and ret0=ret1=0 before version/operation handling.
Version mismatch and unknown operations therefore return a deterministic error with zero
results; unused success result fields are also zero. Existing operation-specific result/error
details take precedence after initialization. Request and reserved bytes are preserved.
An inaccessible/non-writable call frame cannot receive an error response: the kernel rejects
the call without accessing its payload. There is no register error return for this legacy ABI.

| op | Operation | Current behavior |
|---|---|---|
| 0 | GET_ABI_VERSION | ret0=1, ret1=0 |
| 1 | SUBMIT_BELL | Built-in legacy job submission, ret0=handle |
| 2 | GET_STATUS | arg0=handle, ret0=`qos_abi::JobState` discriminant |
| 3 | GET_RESULT | arg0=handle; legacy two-counter result in ret0/ret1; consumes completed slot |
| 4 | EXIT | arg0=exit code; preemptive path reaps current user process without returning |
| 5 | DISPATCH_NEXT | Legacy scheduling shim, ret0=current running handle |
| 6 | CANCEL | arg0=handle |
| 7 | SUBMIT_IR | arg0=header/payload pointer, arg1=total byte length; bounded by 4096 |
| 8 | VFS_IO | arg0=header/path/data pointer, arg1=total byte length; bounded by 4096 |

The submit/VFS headers and their version constants are in `qos_abi::shm`. This document does
not certify legacy quantum scheduling/results or job ownership; those remain tracked by #42
and #7. VFS result details remain operation-specific; generic errno unification is unfinished.

## Compatibility and remaining work

Existing v1 numbers/layout are retained. Register discovery is additive; callers targeting
older kernels must handle its generic error. A breaking request/result change requires an
explicit new version and migration path. The verifier transcript v4 is a separate test
protocol, not an ABI version change.

`syscall_ext.rs` is an internal experimental dispatcher with a conflicting Linux-like number
table and a global FD table. It is not wired to either interrupt and must not be exposed until
process-owned resources and an explicit migration contract exist. The disabled userdemo also
contains a duplicate frame definition that must migrate when its SDK/build path is restored.

ADR-0023 restricts copies to the single-BSP resident-page model. Legacy loaders/foreground
execution, general fault fixup, concurrent unmap/CoW/frame pinning, full errors, process-owned
jobs/FDs and repeated-process cleanup remain open. Modern preemptive shared EXIT uses the same
process-exit path in normal and verification builds; legacy foreground exit still reboots.

## Verification

The portable crate checks layout offsets and a golden little-endian frame, including
response-only encoding that cannot replace request bytes with modified local values.
UEFI transcript v4 retains prior checks and runs an independent literal-wire Ring-3 client:
register version discovery, preserved argument/GPR samples, unknown register call, empty
writes with null/noncanonical pointers, bad shared version, unknown shared operation/handle,
stale-result clearing, input preservation, successful shared version query and shared EXIT.
The kernel emits `QOS_VERIFY PASS abi-contract` only after regaining control and cleaning up
the test process. Timeout, missing markers and unexpected exceptions still fail.
