//! Low-level assembly stubs.
//!
//! Assembled by LLVM's integrated assembler via `global_asm!` so the kernel builds with a
//! pure-Rust toolchain — no external C compiler (`cc`) is required. (Previously these lived
//! in `asm_stubs.s` and were compiled by the `cc` crate, which broke local Windows builds
//! when no ELF-producing C compiler was on PATH.)
//!
//! AT&T syntax is kept verbatim from the original `.s` file via `options(att_syntax)`.

use core::arch::global_asm;

global_asm!(
    r#"
    .section .text

    .global asm_triple_fault
asm_triple_fault:
    cli
    xor %eax, %eax
    lidt (%rax)
    int3
    hlt

    .p2align 4
    .global asm_iretq_to_user
asm_iretq_to_user:
    pushq %rcx
    pushq %rsi
    pushq %r8
    pushq %rdx
    pushq %rdi
    mov %rcx, %rax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    xor %rax, %rax
    xor %rbx, %rbx
    xor %rcx, %rcx
    xor %rdx, %rdx
    xor %rsi, %rsi
    xor %rdi, %rdi
    xor %r8, %r8
    xor %r9, %r9
    xor %r10, %r10
    xor %r11, %r11
    xor %r12, %r12
    xor %r13, %r13
    xor %r14, %r14
    xor %r15, %r15
    xor %rbp, %rbp
    iretq

    # Preemptive timer ISR (Phase 2.1). The CPU has already pushed the iretq frame
    # (ss, rsp, rflags, cs, rip). We push all 15 GPRs in an order that matches the
    # [r15..rax] layout expected by tasking/kthread (r15 ends up at the lowest address,
    # i.e. at the saved stack pointer). We then call timer_dispatch(saved_rsp) which
    # returns the stack pointer to resume on (possibly a different thread), restore the
    # GPRs from there, and iretq. Interrupts stay disabled (interrupt gate) throughout.
    .p2align 4
    .global asm_timer_isr
asm_timer_isr:
    # Clear live DF/AC before any Rust code; leave the CPU's saved RFLAGS unchanged.
    # POPFQ avoids requiring the optional CLAC instruction on older CPUs.
    pushfq
    andq $-263169, (%rsp)   # ~(DF | AC), bits 10 and 18
    popfq
    push %rax
    push %rbx
    push %rcx
    push %rdx
    push %rsi
    push %rdi
    push %rbp
    push %r8
    push %r9
    push %r10
    push %r11
    push %r12
    push %r13
    push %r14
    push %r15
    mov %rsp, %rdi          # arg0 = saved_rsp (points at r15)
    and $-16, %rsp          # 16-byte align for the SysV call (scratch below frame)
    call timer_dispatch
    mov %rax, %rsp          # resume on the returned stack (same or another thread)
    pop %r15
    pop %r14
    pop %r13
    pop %r12
    pop %r11
    pop %r10
    pop %r9
    pop %r8
    pop %rbp
    pop %rdi
    pop %rsi
    pop %rdx
    pop %rcx
    pop %rbx
    pop %rax
    iretq
    "#,
    options(att_syntax)
);

global_asm!(
    r#"
    .section .text

    # Register-based syscall ISR (Phase 2.2), vector 0x81. Saves the full register context in
    # the same [r15..rax] layout, calls syscall_dispatch(frame) which reads the syscall number
    # (rax) and arguments (rdi/rsi/...) and writes the return value into the saved rax slot,
    # then restores and iretq. No PIC EOI (software interrupt). No stack switch: a normal
    # syscall returns to the same Ring-3 context (exit/fault park via the timer instead).
    .p2align 4
    .global asm_syscall_isr
asm_syscall_isr:
    pushfq
    andq $-263169, (%rsp)   # clear live DF/AC; iretq restores the user's saved flags
    popfq
    push %rax
    push %rbx
    push %rcx
    push %rdx
    push %rsi
    push %rdi
    push %rbp
    push %r8
    push %r9
    push %r10
    push %r11
    push %r12
    push %r13
    push %r14
    push %r15
    mov %rsp, %rdi          # arg0 = frame pointer (points at r15)
    and $-16, %rsp          # 16-byte align for the SysV call
    call syscall_dispatch
    mov %rax, %rsp          # syscall_dispatch returns the frame pointer
    pop %r15
    pop %r14
    pop %r13
    pop %r12
    pop %r11
    pop %r10
    pop %r9
    pop %r8
    pop %rbp
    pop %rdi
    pop %rsi
    pop %rdx
    pop %rcx
    pop %rbx
    pop %rax
    iretq
    "#,
    options(att_syntax)
);

// Preserve each hardware frame/error-code layout by jumping, not calling, into the
// compiler's x86-interrupt handler after clearing live DF/AC. No GPR is modified.
global_asm!(
    r#"
    .section .text
    .macro sanitized_gate entry, target
    .p2align 4
    .global \entry
\entry:
    pushfq
    andq $-263169, (%rsp)
    popfq
    jmp \target
    .endm
    sanitized_gate asm_shm_syscall_isr, syscall_interrupt_handler
    sanitized_gate asm_breakpoint_isr, breakpoint_handler
    sanitized_gate asm_invalid_opcode_isr, invalid_opcode_handler
    sanitized_gate asm_gpf_isr, general_protection_fault_handler
    sanitized_gate asm_page_fault_isr, page_fault_handler
    sanitized_gate asm_double_fault_isr, double_fault_handler
    sanitized_gate asm_keyboard_isr, keyboard_interrupt_handler
    sanitized_gate asm_mouse_isr, mouse_interrupt_handler
    sanitized_gate asm_xhci_isr, xhci_interrupt_handler
    sanitized_gate asm_spurious_isr, spurious_interrupt_handler
    "#,
    options(att_syntax)
);

extern "C" {
    /// Raw preemptive timer ISR entry (installed into IDT[Timer] via `set_handler_addr`).
    pub fn asm_timer_isr();
    /// Raw register-based syscall ISR entry (installed into IDT[0x81]).
    pub fn asm_syscall_isr();
    pub fn asm_shm_syscall_isr();
    pub fn asm_breakpoint_isr();
    pub fn asm_invalid_opcode_isr();
    pub fn asm_gpf_isr();
    pub fn asm_page_fault_isr();
    pub fn asm_double_fault_isr();
    pub fn asm_keyboard_isr();
    pub fn asm_mouse_isr();
    pub fn asm_xhci_isr();
    pub fn asm_spurious_isr();
}
