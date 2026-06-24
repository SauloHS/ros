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
mod gdt;
mod init;
mod interrupts;
mod memory;

use crate::memory::active_level_4_table;
use crate::init::{hlt_loop, init};
use bootloader_api::{BootInfo, entry_point, config::{BootloaderConfig, Mapping}};
use core::panic::PanicInfo;
use x86_64::VirtAddr;

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let info = framebuffer.info();
        let buffer = framebuffer.buffer_mut();
        drivers::video::framebuffer::init(buffer, info);
    }
    init();
    println!("ROS");

    let phys_mem_offset = VirtAddr::new(
        boot_info.physical_memory_offset
            .into_option()
            .expect("physical memory offset missing")
    );
    let l4_table = unsafe { active_level_4_table(phys_mem_offset) };

    for (i, entry) in l4_table.iter().enumerate() {
        use x86_64::structures::paging::PageTable;
        if !entry.is_unused() {
            println!("L4 Entry {}: {:?}", i, entry);

        }
    }

    hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    hlt_loop();
}
