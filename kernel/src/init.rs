/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

pub fn init() {
    crate::gdt::init();
    crate::interrupts::init_idt();
    //use x86_64::instructions::segmentation::{SS, Segment};
    //crate::println!("(DEBUG) Current SS: {:?}", SS::get_reg());
    unsafe { crate::interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
