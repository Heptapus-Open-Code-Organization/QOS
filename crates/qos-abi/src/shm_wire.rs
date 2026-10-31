//! Explicit little-endian call-frame decoding and response-only encoding.

use crate::shm::{ShmCall, CALL_SIZE, STATUS_ERR};

const _: () = {
    assert!(core::mem::size_of::<ShmCall>() == CALL_SIZE);
    assert!(core::mem::offset_of!(ShmCall, abi_version) == 0);
    assert!(core::mem::offset_of!(ShmCall, op) == 4);
    assert!(core::mem::offset_of!(ShmCall, status) == 8);
    assert!(core::mem::offset_of!(ShmCall, _reserved) == 12);
    assert!(core::mem::offset_of!(ShmCall, ret0) == 16);
    assert!(core::mem::offset_of!(ShmCall, ret1) == 24);
    assert!(core::mem::offset_of!(ShmCall, arg0) == 32);
    assert!(core::mem::offset_of!(ShmCall, arg1) == 40);
};

impl ShmCall {
    /// Decode a checked, owned snapshot. No typed view into user memory is formed.
    pub fn from_le_bytes(bytes: &[u8; CALL_SIZE]) -> Self {
        Self {
            abi_version: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            op: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            status: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            _reserved: u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            ret0: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            ret1: u64::from_le_bytes(bytes[24..32].try_into().unwrap()),
            arg0: u64::from_le_bytes(bytes[32..40].try_into().unwrap()),
            arg1: u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
        }
    }

    /// Initialize every returning call's response, including rejected versions/ops.
    /// Individual operations may supply documented error details afterwards.
    pub fn reset_response(&mut self) {
        self.status = STATUS_ERR;
        self.ret0 = 0;
        self.ret1 = 0;
    }

    /// Update only response fields in the original snapshot. Request and reserved
    /// bytes stay unchanged, even if a dispatcher changes its local request fields.
    pub fn write_response(&self, bytes: &mut [u8; CALL_SIZE]) {
        bytes[8..12].copy_from_slice(&self.status.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.ret0.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.ret1.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Deliberately distinct, non-symmetric bytes detect swapped fields/endian errors.
    const WIRE: [u8; CALL_SIZE] = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
    ];

    #[test]
    fn decodes_golden_wire_layout() {
        let call = ShmCall::from_le_bytes(&WIRE);
        assert_eq!(call.abi_version, 0x04030201);
        assert_eq!(call.op, 0x08070605);
        assert_eq!(call.status, 0x0c0b0a09);
        assert_eq!(call._reserved, 0x100f0e0d);
        assert_eq!(call.ret0, 0x1817161514131211);
        assert_eq!(call.ret1, 0x201f1e1d1c1b1a19);
        assert_eq!(call.arg0, 0x2827262524232221);
        assert_eq!(call.arg1, 0x302f2e2d2c2b2a29);
    }

    #[test]
    fn response_encoding_preserves_original_request_bytes() {
        let mut call = ShmCall::from_le_bytes(&WIRE);
        call.abi_version = 0;
        call.op = 0;
        call._reserved = 0;
        call.arg0 = 0;
        call.arg1 = 0;
        call.status = 0x11223344;
        call.ret0 = 0x0102030405060708;
        call.ret1 = 0xf1f2f3f4f5f6f7f8;
        let mut bytes = WIRE;
        call.write_response(&mut bytes);
        assert_eq!(&bytes[..8], &WIRE[..8]);
        assert_eq!(&bytes[12..16], &WIRE[12..16]);
        assert_eq!(&bytes[32..], &WIRE[32..]);
        assert_eq!(&bytes[8..12], &[0x44, 0x33, 0x22, 0x11]);
        assert_eq!(&bytes[16..24], &[8, 7, 6, 5, 4, 3, 2, 1]);
        assert_eq!(
            &bytes[24..32],
            &[0xf8, 0xf7, 0xf6, 0xf5, 0xf4, 0xf3, 0xf2, 0xf1]
        );
    }
}
