/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

use crate::println;

pub fn run_all_tests() {
    println!();
    crate::gdt::run_tests();
    crate::interrupts::run_tests();
    crate::drivers::video::framebuffer::run_tests();
    crate::init::run_tests();
    println!("\nAll tests passed!");
    exit_qemu_success();
}

fn exit_qemu_success() -> ! {
    use x86_64::instructions::port::Port;
    unsafe { Port::new(0xf4).write(0u32); }
    loop {}
}

pub fn exit_qemu_failure() {
    use x86_64::instructions::port::Port;
    unsafe { Port::new(0xf4).write(1u32); }
}
