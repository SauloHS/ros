/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

mod drivers;
mod interrupts;
mod init;
mod gdt;

use bootloader_api::{entry_point, BootInfo};
use core::panic::PanicInfo;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let info = framebuffer.info();
        let buffer = framebuffer.buffer_mut();
        drivers::video::framebuffer::init(buffer, info);
    }
    init::init();
    
    println!("ROS");

    loop {} 
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
} 