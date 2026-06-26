/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

use x86_64::VirtAddr;

use crate::arch::x86_64::gdt;

pub fn enter_user_mode(entry: VirtAddr, stack_top: VirtAddr) -> ! {
    let cs = gdt::selectors().user_code_selector.0 as u64;
    let ss = gdt::selectors().user_data_selector.0 as u64;
    unsafe {
        core::arch::asm!(
            "push {ss}",
            "push {rsp}",
            "push {rflags}",
            "push {cs}",
            "push {rip}",
            "iretq",
            ss = in(reg) ss,
            rsp = in(reg) stack_top.as_u64(),
            rflags = in(reg) 0x202u64,
            cs = in(reg) cs,
            rip = in(reg) entry.as_u64(),
            options(noreturn)
        )
    }
}
