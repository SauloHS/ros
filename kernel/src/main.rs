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

extern crate alloc;

mod allocator;
mod drivers;
mod gdt;
mod init;
mod interrupts;
mod memory;
mod task;

use crate::init::{hlt_loop, init};
use bootloader_api::{
    BootInfo,
    config::{BootloaderConfig, Mapping},
    entry_point,
};
use core::panic::PanicInfo;
use memory::BootInfoFrameAllocator;
use task::{Task, executor::Executor, keyboard};
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

    let phys_mem_offset = VirtAddr::new(
        boot_info
            .physical_memory_offset
            .into_option()
            .expect("physical memory offset missing"),
    );
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    let mut executor = Executor::new();
    executor.spawn(Task::new(keyboard::print_keypresses()));
    executor.run();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    hlt_loop();
}
