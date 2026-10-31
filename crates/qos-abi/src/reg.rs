//! Register ABI v1, software interrupt 0x81. This is separate from the shared ABI
//! operation namespace and the kernel-only, experimental extended file dispatcher.
//!
//! RAX carries the call number/result; arguments use RDI, RSI, RDX, R10, R8, R9.
//! Returning calls preserve the other GPRs and saved RFLAGS. Interrupt callers must
//! describe memory effects to their compiler; no `nomem`/`readonly` assumption is valid.

pub const ABI_VERSION: u64 = 1;
pub const SYS_EXIT: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_GETTICKS: u64 = 2;
/// Additive discovery call; kernels predating it return ERROR.
pub const SYS_GET_ABI_VERSION: u64 = 3;
pub const ERROR: u64 = u64::MAX;
pub const MAX_WRITE: usize = 4096;
