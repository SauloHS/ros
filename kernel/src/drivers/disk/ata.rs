/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

use x86_64::instructions::port::Port;

const DATA: u16 = 0x1F0;
const SECTOR_COUNT: u16 = 0x1F2;
const LBA_LOW: u16 = 0x1F3;
const LBA_MID: u16 = 0x1F4;
const LBA_HIGH: u16 = 0x1F5;
const DRIVE_SELECT: u16 = 0x1F6;
const COMMAND_STATUS: u16 = 0x1F7;
const DEVICE_CONTROL: u16 = 0x3F6;

const STATUS_BSY: u8 = 0x80;
const STATUS_DRDY: u8 = 0x40;
const STATUS_DRQ: u8 = 0x08;
const STATUS_ERR: u8 = 0x01;

const CMD_READ_SECTORS: u8 = 0x20;
const CMD_IDENTIFY: u8 = 0xEC;

pub const SECTOR_SIZE: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Ata,
    Atapi,
    None,
}

pub struct Drive {
    master: bool,
    present: bool,
}

impl Drive {
    pub fn new_master() -> Self {
        Drive {
            master: true,
            present: false,
        }
    }

    pub fn new_slave() -> Self {
        Drive {
            master: false,
            present: false,
        }
    }

    pub fn disable_interrupts() {
        use x86_64::instructions::port::Port;
        let mut ctrl: Port<u8> = Port::new(DEVICE_CONTROL);
        unsafe { ctrl.write(0x02) }
        let mut pic2: Port<u8> = Port::new(0xA1);
        let mask = unsafe { pic2.read() };
        unsafe { pic2.write(mask | 0xC0) }
    }

    pub fn probe(&mut self) -> DeviceType {
        let mut drive_sel: Port<u8> = Port::new(DRIVE_SELECT);
        let mut status: Port<u8> = Port::new(COMMAND_STATUS);
        let mut lba_mid: Port<u8> = Port::new(LBA_MID);
        let mut lba_high: Port<u8> = Port::new(LBA_HIGH);

        unsafe { drive_sel.write(if self.master { 0xA0 } else { 0xB0 }) }
        unsafe {
            status.read();
            status.read();
            status.read();
            status.read();
        }

        let mut cmd: Port<u8> = Port::new(COMMAND_STATUS);
        unsafe { cmd.write(CMD_IDENTIFY) }

        let s = unsafe { status.read() };
        if s == 0 {
            return DeviceType::None;
        }

        for _ in 0..100000 {
            let s = unsafe { status.read() };
            if s & STATUS_BSY == 0 {
                if s & STATUS_ERR != 0 {
                    let mid = unsafe { lba_mid.read() };
                    let high = unsafe { lba_high.read() };
                    return if mid == 0x14 && high == 0xEB {
                        DeviceType::Atapi
                    } else {
                        DeviceType::None
                    };
                }
                if s & STATUS_DRQ != 0 {
                    let mut data: Port<u16> = Port::new(DATA);
                    for _ in 0..256 {
                        unsafe { data.read() };
                    }
                    self.present = true;
                    return DeviceType::Ata;
                }
            }
        }

        DeviceType::None
    }

    #[allow(dead_code)]
    fn wait_busy(&self) -> Result<(), &'static str> {
        let mut status: Port<u8> = Port::new(COMMAND_STATUS);
        for _ in 0..1000000 {
            let s = unsafe { status.read() };
            if s & STATUS_BSY == 0 {
                return Ok(());
            }
        }
        Err("ata: BSY timeout")
    }

    fn wait_drq(&self) -> Result<(), &'static str> {
        let mut status: Port<u8> = Port::new(COMMAND_STATUS);
        for _ in 0..1000000 {
            let s = unsafe { status.read() };
            if s & STATUS_BSY == 0 {
                if s & STATUS_DRQ != 0 {
                    return Ok(());
                }
                if s & STATUS_ERR != 0 {
                    return Err("ata: ERR during read");
                }
            }
        }
        Err("ata: DRQ timeout")
    }

    fn wait_ready(&self) -> Result<(), &'static str> {
        let mut status: Port<u8> = Port::new(COMMAND_STATUS);
        for _ in 0..1000000 {
            let s = unsafe { status.read() };
            if s & STATUS_BSY == 0 {
                if s & STATUS_DRDY != 0 {
                    return Ok(());
                }
            }
        }
        Err("ata: DRDY timeout")
    }

    fn delay(&self) {
        let mut status: Port<u8> = Port::new(COMMAND_STATUS);
        unsafe {
            status.read();
            status.read();
            status.read();
            status.read();
        }
    }

    pub fn read_sectors(&mut self, lba: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
        if !self.present {
            return Err("ata: drive not present");
        }

        let sectors = buffer.len() / SECTOR_SIZE;
        if buffer.len() % SECTOR_SIZE != 0 {
            return Err("ata: buffer must be multiple of sector size");
        }
        if sectors == 0 || sectors > 256 {
            return Err("ata: invalid sector count (1-256)");
        }

        self.wait_ready()?;

        let master_bit = if self.master { 0 } else { 0x10 };
        let mut ds: Port<u8> = Port::new(DRIVE_SELECT);
        unsafe { ds.write(0xE0 | master_bit | ((lba >> 24) & 0x0F) as u8) }
        self.delay();

        let mut sc: Port<u8> = Port::new(SECTOR_COUNT);
        unsafe { sc.write(sectors as u8) }

        let mut ll: Port<u8> = Port::new(LBA_LOW);
        unsafe { ll.write(lba as u8) }

        let mut lm: Port<u8> = Port::new(LBA_MID);
        unsafe { lm.write((lba >> 8) as u8) }

        let mut lh: Port<u8> = Port::new(LBA_HIGH);
        unsafe { lh.write((lba >> 16) as u8) }

        let mut cmd: Port<u8> = Port::new(COMMAND_STATUS);
        unsafe { cmd.write(CMD_READ_SECTORS) }

        let mut data: Port<u16> = Port::new(DATA);
        for s in 0..sectors {
            self.wait_drq()?;
            for i in 0..256 {
                let val = unsafe { data.read() };
                let off = s * SECTOR_SIZE + i * 2;
                buffer[off] = (val & 0xFF) as u8;
                buffer[off + 1] = (val >> 8) as u8;
            }
        }

        Ok(())
    }
}
