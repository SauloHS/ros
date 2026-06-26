/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

use core::arch::naked_asm;

#[repr(C)]
struct SyscallRegs {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbp: u64,
    rbx: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    rax: u64,
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn int_0x80_handler_frame() {
    naked_asm!(
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov rdi, rsp",
        "call {handler}",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "iretq",
        handler = sym handle_syscall,
    )
}

#[unsafe(no_mangle)]
extern "C" fn handle_syscall(regs: &mut SyscallRegs) {
    match regs.rax {
        1 => {
            let fd = regs.rdi;
            let buf = regs.rsi as *const u8;
            let len = regs.rdx as usize;
            regs.rax = sys_write(fd, buf, len) as u64;
        }
        60 => {
            let code = regs.rdi;
            sys_exit(code);
        }
        _ => {
            regs.rax = !0;
        }
    }
}

fn sys_write(fd: u64, buf: *const u8, len: usize) -> i64 {
    if fd != 1 && fd != 2 {
        return -1;
    }
    let slice = unsafe { core::slice::from_raw_parts(buf, len) };

    if let Ok(s) = core::str::from_utf8(slice) {
        crate::print!("{}", s);
    } else {
        for &b in slice {
            if b.is_ascii() {
                crate::print!("{}", b as char);
            }
        }
    }
    len as i64
}

fn sys_exit(_code: u64) -> ! {
    crate::arch::x86_64::init::hlt_loop()
}
