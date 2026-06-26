/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

pub fn init() {
    crate::arch::x86_64::gdt::init();
    crate::arch::x86_64::gdt::init_syscall();
    crate::arch::x86_64::interrupts::init_idt();
    unsafe { crate::arch::x86_64::interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
