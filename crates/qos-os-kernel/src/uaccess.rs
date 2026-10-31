//! Bounded resident user copies through the supervisor physical-memory mapping.
//! Interrupt exclusion pins CR3/mappings on today's single BSP. SMP/demand paging must add
//! address-space locks/frame pinning before enabling concurrent unmap. See ADR-0023.

pub use qos_uaccess::{Access, Error, MAX_COPY};
use qos_uaccess::{UserRange, PAGE_SIZE};

fn page(address: u64, root: u64, offset: u64, access: Access) -> Result<u64, Error> {
    qos_uaccess::user_page(address, root, access, |table, index| {
        let virtual_address = offset.checked_add(table)?.checked_add((index * 8) as u64)?;
        x86_64::VirtAddr::try_new(virtual_address).ok()?;
        // Page tables are kernel-owned and resident; no user reference or mutable mapper alias.
        Some(unsafe { core::ptr::read_volatile(virtual_address as *const u64) })
    })
}

pub fn validate(ptr: u64, len: usize, access: Access) -> Result<(), Error> {
    let range = UserRange::new(ptr, len)?;
    x86_64::instructions::interrupts::without_interrupts(|| {
        let root = crate::memory::current_cr3_frame().start_address().as_u64();
        let offset = crate::memory::phys_offset().as_u64();
        for address in range.pages() {
            page(address, root, offset, access)?;
        }
        Ok(())
    })
}

fn transfer(ptr: u64, kernel: *mut u8, len: usize, access: Access) -> Result<(), Error> {
    let range = UserRange::new(ptr, len)?;
    x86_64::instructions::interrupts::without_interrupts(|| {
        let root = crate::memory::current_cr3_frame().start_address().as_u64();
        let offset = crate::memory::phys_offset().as_u64();
        // Validate the *whole* range before changing any caller-visible bytes.
        for address in range.pages() {
            page(address, root, offset, access)?;
        }
        let mut copied = 0;
        while copied < len {
            let address = ptr + copied as u64;
            let physical_page = page(address, root, offset, access)?;
            let in_page = (address & (PAGE_SIZE as u64 - 1)) as usize;
            let n = (PAGE_SIZE - in_page).min(len - copied);
            let alias = offset
                .checked_add(physical_page)
                .and_then(|a| a.checked_add(in_page as u64))
                .ok_or(Error::InvalidRange)? as *mut u8;
            // The kernel buffer is a resident, kernel-owned slice; user frames are allocator-
            // owned distinct frames. Access is via a supervisor alias, so SMAP stays enabled.
            unsafe {
                match access {
                    Access::Read => core::ptr::copy_nonoverlapping(alias, kernel.add(copied), n),
                    Access::Write => core::ptr::copy_nonoverlapping(kernel.add(copied), alias, n),
                }
            }
            copied += n;
        }
        Ok(())
    })
}

pub fn copy_from_user(ptr: u64, out: &mut [u8]) -> Result<(), Error> {
    transfer(ptr, out.as_mut_ptr(), out.len(), Access::Read)
}

pub fn copy_to_user(ptr: u64, input: &[u8]) -> Result<(), Error> {
    transfer(ptr, input.as_ptr() as *mut u8, input.len(), Access::Write)
}

#[cfg(feature = "verify")]
pub fn verify_mappings() -> bool {
    use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags as F, Size4KiB};
    use x86_64::VirtAddr;
    const DATA: u64 = 0x4002_0000;
    let flags = F::PRESENT | F::WRITABLE | F::USER_ACCESSIBLE | F::NO_EXECUTE;
    let (root, mut mapper, mut pages) = crate::memory::with_ctx(|_, allocator| {
        let (root, mut mapper) = crate::memory::create_user_pagetable(allocator);
        let mut pages = alloc::vec::Vec::new();
        for address in [DATA, DATA + 4096, crate::syscall::ABI_CALL_ADDR] {
            let p = Page::<Size4KiB>::containing_address(VirtAddr::new(address));
            let frame = allocator.allocate_frame().expect("verification frame");
            unsafe {
                mapper
                    .map_to(p, frame, flags, allocator)
                    .expect("verification mapping")
                    .flush();
            }
            pages.push(p);
        }
        (root, mapper, pages)
    });
    let ok = x86_64::instructions::interrupts::without_interrupts(|| {
        let saved = crate::memory::switch_cr3(root);
        let passed = (|| -> Result<bool, Error> {
            let ptr = DATA + 4094;
            copy_to_user(ptr, &[1, 2, 3, 4])?;
            let mut out = [0u8; 4];
            copy_from_user(ptr, &mut out)?;
            if out != [1, 2, 3, 4] {
                return Ok(false);
            }
            for bad in [
                0,
                qos_uaccess::USER_LIMIT,
                0x8000_0000_0000,
                u64::MAX,
                DATA + 8192,
            ] {
                if copy_from_user(bad, &mut out[..1]).is_ok() {
                    return Ok(false);
                }
            }
            let second = Page::<Size4KiB>::containing_address(VirtAddr::new(DATA + 4096));
            unsafe {
                mapper
                    .update_flags(second, flags & !F::WRITABLE)
                    .unwrap()
                    .flush();
            }
            if copy_to_user(ptr, &[5, 6, 7, 8]) != Err(Error::Permission) {
                return Ok(false);
            }
            copy_from_user(ptr, &mut out)?;
            if out != [1, 2, 3, 4] {
                return Ok(false);
            } // preflight prevents partial writes
            unsafe {
                mapper
                    .update_flags(second, flags & !F::USER_ACCESSIBLE)
                    .unwrap()
                    .flush();
            }
            out = [99; 4];
            if copy_from_user(ptr, &mut out) != Err(Error::Permission) || out != [99; 4] {
                return Ok(false);
            }
            unsafe {
                mapper.update_flags(second, flags).unwrap().flush();
            }
            if !crate::syscall::verify_uaccess_paths(DATA) {
                return Ok(false);
            }
            let call =
                Page::<Size4KiB>::containing_address(VirtAddr::new(crate::syscall::ABI_CALL_ADDR));
            unsafe {
                mapper
                    .update_flags(call, flags & !F::WRITABLE)
                    .unwrap()
                    .flush();
            }
            let rejected = crate::syscall::verify_rejected_frame();
            unsafe {
                mapper.update_flags(call, flags).unwrap().flush();
            }
            Ok(rejected)
        })()
        .unwrap_or(false);
        crate::memory::switch_cr3(saved);
        passed
    });
    drop(mapper);
    crate::user::cleanup_spawned_user_process(root, &mut pages);
    ok
}
