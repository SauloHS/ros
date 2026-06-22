/*
File created by Saulo Henrique Santos Dorotéio.
Last updated by Saulo Henrique Santos Dorotéio, at 06/22/2026.
See LICENSE file for licensing information */

pub fn init() {
    crate::gdt::init();
    crate::interrupts::init_idt();
    unsafe { crate::interrupts::PICS.lock().initialize() };
    unsafe {
        let mut pics = crate::interrupts::PICS.lock();
        pics.initialize();
        pics.write_masks(0b1111_1100, 0b1111_1111);
    }

    x86_64::instructions::interrupts::enable();
}