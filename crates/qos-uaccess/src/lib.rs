//! User-copy policy for QOS's current four-level paging/4 KiB user mappings.
//! No dereferences or allocation: a trusted adapter supplies page-table entry reads.
#![no_std]

pub const PAGE_SIZE: usize = 4096;
pub const MAX_COPY: usize = 4096;
/// P4[0] is private user space; the other entries are shared kernel mappings (ADR-0023).
pub const USER_LIMIT: u64 = 1 << 39;
const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
const PRESENT: u64 = 1;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const HUGE: u64 = 1 << 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidRange,
    TooLarge,
    Unmapped,
    Permission,
    UnsupportedPageSize,
}

#[derive(Clone, Copy, Debug)]
pub struct UserRange {
    start: u64,
    len: usize,
}

impl UserRange {
    /// Empty copies are no-ops, including null pointers; all nonempty copies are bounded.
    pub fn new(start: u64, len: usize) -> Result<Self, Error> {
        if len > MAX_COPY {
            return Err(Error::TooLarge);
        }
        if len != 0 {
            let end = start
                .checked_add((len - 1) as u64)
                .ok_or(Error::InvalidRange)?;
            if start == 0 || end >= USER_LIMIT {
                return Err(Error::InvalidRange);
            }
        }
        Ok(Self { start, len })
    }

    pub fn pages(self) -> impl Iterator<Item = u64> {
        let first = self.start & !(PAGE_SIZE as u64 - 1);
        let count = if self.len == 0 {
            0
        } else {
            ((self.start - first) as usize + self.len + PAGE_SIZE - 1) / PAGE_SIZE
        };
        (0..count).map(move |i| first + (i * PAGE_SIZE) as u64)
    }
}

/// Translate one user page, requiring PRESENT/USER and, for writes, WRITABLE at *every*
/// level. Reject huge mappings until user VM supports them. The leaf's bit 7 is PAT, not PS.
pub fn user_page(
    address: u64,
    root: u64,
    access: Access,
    mut read_entry: impl FnMut(u64, usize) -> Option<u64>,
) -> Result<u64, Error> {
    if address >= USER_LIMIT || root & !ADDRESS_MASK != 0 {
        return Err(Error::InvalidRange);
    }
    let mut table = root;
    for (level, shift) in [39, 30, 21, 12].into_iter().enumerate() {
        let entry =
            read_entry(table, ((address >> shift) & 0x1ff) as usize).ok_or(Error::Unmapped)?;
        if entry & PRESENT == 0 {
            return Err(Error::Unmapped);
        }
        if entry & USER == 0 || (access == Access::Write && entry & WRITABLE == 0) {
            return Err(Error::Permission);
        }
        if level < 3 && entry & HUGE != 0 {
            return Err(Error::UnsupportedPageSize);
        }
        table = entry & ADDRESS_MASK;
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translate(entries: [u64; 4], access: Access) -> Result<u64, Error> {
        user_page(0x4000_0000, 0x1000, access, |table, index| {
            let level = (table / 0x1000 - 1) as usize;
            let expected = [0, 1, 0, 0][level];
            assert_eq!(index, expected);
            Some(entries[level])
        })
    }

    fn entries() -> [u64; 4] {
        [0x2007, 0x3007, 0x4007, 0x5007]
    }

    #[test]
    fn ranges_reject_bad_addresses_and_lengths() {
        for address in [
            0,
            USER_LIMIT,
            0x0000_8000_0000_0000,
            0xffff_8000_0000_0000,
            u64::MAX,
        ] {
            assert_eq!(UserRange::new(address, 1).unwrap_err(), Error::InvalidRange);
        }
        assert!(UserRange::new(USER_LIMIT - 1, 1).is_ok());
        assert!(UserRange::new(USER_LIMIT - 1, 2).is_err());
        assert!(UserRange::new(u64::MAX - 1, 4).is_err());
        assert_eq!(
            UserRange::new(1, MAX_COPY + 1).unwrap_err(),
            Error::TooLarge
        );
    }

    #[test]
    fn page_coverage_handles_empty_and_cross_page_buffers() {
        assert_eq!(UserRange::new(u64::MAX, 0).unwrap().pages().count(), 0);
        assert_eq!(UserRange::new(4096, 4096).unwrap().pages().count(), 1);
        let mut pages = UserRange::new(8191, 4096).unwrap().pages();
        assert_eq!(pages.next(), Some(4096));
        assert_eq!(pages.next(), Some(8192));
        assert_eq!(pages.next(), None);
    }

    #[test]
    fn read_and_write_require_user_permission_at_every_level() {
        assert_eq!(translate(entries(), Access::Read), Ok(0x5000));
        assert_eq!(translate(entries(), Access::Write), Ok(0x5000));
        for level in 0..4 {
            let mut e = entries();
            e[level] &= !USER;
            for access in [Access::Read, Access::Write] {
                assert_eq!(translate(e, access), Err(Error::Permission));
            }
        }
    }

    #[test]
    fn ancestor_read_only_permissions_block_writes_but_allow_reads() {
        for level in 0..4 {
            let mut e = entries();
            e[level] &= !WRITABLE;
            assert_eq!(translate(e, Access::Write), Err(Error::Permission));
            assert_eq!(translate(e, Access::Read), Ok(0x5000));
        }
    }

    #[test]
    fn absent_entries_and_huge_pages_are_rejected() {
        for level in 0..4 {
            let mut e = entries();
            e[level] &= !PRESENT;
            assert_eq!(translate(e, Access::Read), Err(Error::Unmapped));
        }
        for level in 0..3 {
            let mut e = entries();
            e[level] |= HUGE;
            assert_eq!(translate(e, Access::Read), Err(Error::UnsupportedPageSize));
        }
        let mut e = entries();
        e[3] |= HUGE; // PAT on a 4 KiB leaf is valid.
        assert_eq!(translate(e, Access::Read), Ok(0x5000));
        assert_eq!(
            user_page(1, 0x1000, Access::Read, |_, _| None),
            Err(Error::Unmapped)
        );
    }

    #[test]
    fn invalid_roots_and_kernel_addresses_do_not_invoke_adapter() {
        for (address, root) in [(USER_LIMIT, 0x1000), (1, 0x1001), (1, u64::MAX)] {
            assert_eq!(
                user_page(address, root, Access::Read, |_, _| panic!("must not read")),
                Err(Error::InvalidRange)
            );
        }
    }
}
