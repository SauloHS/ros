/*
File created by Saulo Henrique Santos Dorotéio.
Last updated by Saulo Henrique Santos Dorotéio, at 06/22/2026.
See LICENSE file for licensing information */

pub fn init() {
    crate::gdt::init();
    crate::interrupts::init_idt();
}