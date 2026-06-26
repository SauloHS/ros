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

mod arch;
mod drivers;
mod elf;
mod mm;
mod sched;

use crate::arch::x86_64::init::{hlt_loop, init};
use crate::arch::x86_64::memory::BootInfoFrameAllocator;
use bootloader_api::{
    BootInfo,
    config::{BootloaderConfig, Mapping},
    entry_point,
};
use core::panic::PanicInfo;
use x86_64::VirtAddr;
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB};

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
    let mut mapper = unsafe { arch::x86_64::memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };

    mm::allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    if let Err(e) = try_run_init(&mut mapper, &mut frame_allocator) {
        println!("init failed: {}", e);
    }

    hlt_loop()
}

fn try_run_init(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), &'static str> {
    use crate::drivers::disk::ata::{DeviceType, Drive};
    use crate::drivers::disk::fat32::Fat32;

    Drive::disable_interrupts();

    let mut drive = Drive::new_master();
    if !matches!(drive.probe(), DeviceType::Ata) {
        drive = Drive::new_slave();
        if !matches!(drive.probe(), DeviceType::Ata) {
            return Err("no ATA drive found");
        }
    }

    let mut fs = Fat32::new(drive).map_err(|_| "FAT32 mount failed")?;
    let entries = fs.root_entries().map_err(|_| "root dir read failed")?;

    let elf_name = entries
        .iter()
        .find(|e| !e.is_dir && (e.name == "INIT.ELF" || e.name == "init.elf"))
        .ok_or("INIT.ELF not found")?
        .name
        .clone();

    let elf_data = fs.open_file(&elf_name).map_err(|_| "open file failed")?;
    let entry = elf::load_elf(&elf_data, mapper, frame_allocator).map_err(|_| "ELF load failed")?;

    let user_stack_bottom = VirtAddr::new(0x7FFFFF0000);
    let user_stack_size: u64 = 64 * 1024;
    let user_stack_top = user_stack_bottom + user_stack_size;

    for offset in (0..user_stack_size).step_by(4096) {
        let page = Page::containing_address(user_stack_bottom + offset);
        let frame = frame_allocator
            .allocate_frame()
            .ok_or("out of memory for user stack")?;
        let flags =
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
        unsafe {
            mapper
                .map_to(page, frame, flags, frame_allocator)
                .map_err(|_| "user stack map failed")?
                .flush();
        }
    }

    crate::arch::x86_64::user::enter_user_mode(entry, user_stack_top);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    hlt_loop()
}
