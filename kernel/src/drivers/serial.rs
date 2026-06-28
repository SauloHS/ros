/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

#![allow(dead_code)]

use core::fmt;
use spinning_top::Spinlock;
use x86_64::instructions::port::Port;

pub static SERIAL1: Spinlock<SerialPort> = Spinlock::new(SerialPort::new(0x3F8));

pub struct SerialPort {
    data: Port<u8>,
    int_en: Port<u8>,
    fifo_ctrl: Port<u8>,
    line_ctrl: Port<u8>,
    modem_ctrl: Port<u8>,
    line_sts: Port<u8>,
}

impl SerialPort {
    pub const fn new(base: u16) -> Self {
        SerialPort {
            data: Port::new(base),
            int_en: Port::new(base + 1),
            fifo_ctrl: Port::new(base + 2),
            line_ctrl: Port::new(base + 3),
            modem_ctrl: Port::new(base + 4),
            line_sts: Port::new(base + 5),
        }
    }

    pub fn init(&mut self) {
        unsafe {
            self.int_en.write(0x00u8);
            self.line_ctrl.write(0x80u8);
            self.data.write(0x03u8);
            self.int_en.write(0x00u8);
            self.line_ctrl.write(0x03u8);
            self.fifo_ctrl.write(0xC7u8);
            self.modem_ctrl.write(0x03u8);
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        unsafe {
            while self.line_sts.read() & 0x20 == 0 {}
            self.data.write(byte);
        }
    }

    pub fn write_bytes(&mut self, buf: &[u8]) {
        for &b in buf {
            self.write_byte(b);
        }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &b in s.as_bytes() {
            match b {
                b'\n' => {
                    self.write_byte(b'\r');
                    self.write_byte(b'\n');
                }
                b'\x08' | b'\x7f' => {
                    self.write_byte(b'\x08');
                    self.write_byte(b' ');
                    self.write_byte(b'\x08');
                }
                _ => {
                    self.write_byte(b);
                }
            }
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        use core::fmt::Write;
        let _ = write!($crate::drivers::serial::SERIAL1.lock(), $($arg)*);
    };
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}
