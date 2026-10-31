# ADR-0023: Bounded resident user-memory transfers

- Status: Accepted for the current single-BSP, resident-page model
- Date: 2026-10-31
- Related: #40, #7, #20, #41; ADR-0020 CPU hardening

## Context

Syscall inputs must be checked for user ownership and access permissions, not just address
translation. Creating slices/references directly from untrusted virtual pointers also conflicts
with SMAP and makes validation/ownership difficult to audit. The kernel currently uses
four-level page tables, private user mappings under P4[0], 4 KiB resident user pages and one BSP.

## Decision

Add the allocation-free, `no_std` `qos-uaccess` policy crate. Nonempty copies reject null,
overflowed, noncanonical/out-of-user-region and >4096-byte ranges before constructing a virtual
address. The permitted region is P4[0] (`0 < address < 2^39`), matching current page-table
creation/teardown. Empty transfers are no-ops. A pure page walker requires PRESENT and USER
at all four levels and WRITABLE at all levels for output. Huge user mappings are rejected;
the leaf PAT bit is not mistaken for a huge-page flag.

Kernel `uaccess` adapters read the active CR3 and trusted, kernel-owned page tables. They
validate the whole transfer before copying, then use the existing supervisor physical-memory
mapping in page-sized chunks. The kernel buffer must be resident kernel-owned memory, distinct
from user-owned frames. No Rust reference/slice into a user's virtual range is constructed,
and no STAC/CLAC window or disabling of SMAP is needed. The immutable physical-offset lookup
uses `spin::Once`, avoiding a lock held by an interrupted task.

Interrupts are excluded for each bounded validation/copy operation. On the current single BSP
this prevents CR3 switches or concurrent unmap while the transfer is in progress. Preflight
prevents partial output for ordinary invalid/permission-denied ranges. This is **not** a
general page-fault fixup mechanism: corrupt kernel page tables, broken supervisor mappings,
DMA interference or hardware faults are outside this transfer contract.

The register SYS_WRITE uses a kernel bounce buffer. The shared ABI snapshots its call frame,
validates writable output before side effects, operates on an owned integer-only frame and
encodes return fields explicitly; it avoids response writes after a legacy context switch.
Submit buffers and VFS input/output use checked copies; submit headers use unaligned reads.
The extended file dispatcher also uses bounded buffers, but its file ownership/offset/resource
semantics and broader routing remain separate work.

Existing error contracts remain: register write uses its existing -1 rejection, the shared
ABI uses STATUS_ERR, and extended file transfer reports EFAULT (oversized transfer: EINVAL).
There is no ABI version bump or claim of complete error-number unification in this slice.

## Consequences and gates

- Before SMP, demand paging, shared/unmapped VMAs or CoW can race with copies, add address-space
  locking/frame pinning and define fault resolution/cancellation. Do not treat interrupt
  exclusion as an SMP lock. Further kernel user-memory access must use the same discipline.
- Other legacy ELF/demo loading paths still contain direct memory access and need a separate
  SMAP audit. The small Ring-3 verification payload loader is migrated here.
- The syscall entry AC/DF policy, fault fixup if user-VA access is introduced, a stable unified
  ABI, full process-owned resources and adversarial concurrent-process coverage remain open.
- Kernel-owned page tables and trusted direct-map coverage are preconditions; user processes
  cannot supply arbitrary physical frame mappings through this API.

## Verification

Host tests cover bounds/overflow, page coverage, permission bits at every level, read-only
ancestors, absent entries, huge mappings and invalid roots. UEFI verification contract v2 adds
cross-page transfers, denied output with no partial write, inaccessible pages, production
register/shared-ABI dispatch and VFS response checks, plus a real Ring-3 int 0x81 write/exit.
Run qemu64 and max profiles; the latter exercises SMEP/SMAP where exposed. Serial fault markers,
wrong exits and incomplete transcripts fail the host runner.
