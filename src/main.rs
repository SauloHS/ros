/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

fn main() {
    let bios_path = env!("BIOS_PATH");

    let nographic = std::env::args().any(|arg| arg == "--nographic");
    let debug = std::env::args().any(|arg| arg == "--debug");

    let disk_image = std::env::var("DISK_IMAGE").unwrap_or_else(|_| "disk.img".to_string());
    let disk_path = Path::new(&disk_image);

    if !disk_path.exists() {
        eprintln!(
            "Disk image '{}' not found. Creating 64 MB FAT32 disk image...",
            disk_path.display()
        );
        create_fat32_disk(disk_path).unwrap_or_else(|e| {
            eprintln!("Failed to create disk image: {}", e);
            std::process::exit(1);
        });
        eprintln!("Disk image created successfully.");
    }

    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    cmd.arg("-drive")
        .arg(format!("format=raw,file={bios_path}"))
        .arg("-drive")
        .arg(format!("format=raw,file={}", disk_path.display()));

    if nographic {
        cmd.arg("-nographic");
    }

    if debug {
        cmd.arg("-d")
            .arg("int,cpu_reset")
            .arg("-D")
            .arg("qemu.log")
            .arg("--no-reboot")
            .arg("--no-shutdown");
    }

    let status = cmd.status().unwrap();
    std::process::exit(status.code().unwrap_or(-1));
}

fn create_fat32_disk(path: &Path) -> std::io::Result<()> {
    let ss: u64 = 512;
    let spc: u8 = 8;
    let rsvd: u16 = 32;
    let nfats: u8 = 2;
    let total_sectors: u64 = 64 * 1024 * 1024 / ss;
    let data_sectors = total_sectors - rsvd as u64;
    let total_clusters = data_sectors / spc as u64;
    let fat_entries = total_clusters + 2;
    let fat_sectors = ((fat_entries * 4 + ss - 1) / ss) as u32;
    let first_data_sector = rsvd as u64 + nfats as u64 * fat_sectors as u64;

    let file = std::fs::File::create(path)?;
    file.set_len(total_sectors * ss)?;
    let mut f = std::io::BufWriter::new(file);

    let w32 = |buf: &mut [u8], off: usize, v: u32| {
        buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
    };
    let w16 = |buf: &mut [u8], off: usize, v: u16| {
        buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
    };

    let mut bpb = vec![0u8; ss as usize];
    bpb[0] = 0xEB;
    bpb[1] = 0x58;
    bpb[2] = 0x90;
    bpb[3..11].copy_from_slice(b"ROS     ");
    w16(&mut bpb, 0x0B, ss as u16);
    bpb[0x0D] = spc;
    w16(&mut bpb, 0x0E, rsvd);
    bpb[0x10] = nfats;
    w16(&mut bpb, 0x11, 0);
    w16(&mut bpb, 0x13, 0);
    bpb[0x15] = 0xF8;
    w16(&mut bpb, 0x16, 0);
    w16(&mut bpb, 0x18, 32);
    w16(&mut bpb, 0x1A, 64);
    w32(&mut bpb, 0x1C, 0);
    w32(&mut bpb, 0x20, total_sectors as u32);
    w32(&mut bpb, 0x24, fat_sectors);
    w16(&mut bpb, 0x28, 0);
    w16(&mut bpb, 0x2A, 0);
    w32(&mut bpb, 0x2C, 2);
    w16(&mut bpb, 0x30, 1);
    w16(&mut bpb, 0x32, 6);
    bpb[0x40] = 0x80;
    bpb[0x42] = 0x29;
    w32(&mut bpb, 0x43, 0x12345678);
    bpb[0x47..0x52].copy_from_slice(b"ROS DISK   ");
    bpb[0x52..0x5A].copy_from_slice(b"FAT32   ");
    bpb[0x1FE] = 0x55;
    bpb[0x1FF] = 0xAA;
    f.seek(SeekFrom::Start(0))?;
    f.write_all(&bpb)?;

    let mut fsinfo = vec![0u8; ss as usize];
    fsinfo[0..4].copy_from_slice(b"RRaA");
    w32(&mut fsinfo, 0x1E4, 0xFFFFFFFF);
    w32(&mut fsinfo, 0x1E8, 2);
    fsinfo[0x1FC] = 0x55;
    fsinfo[0x1FD] = 0xAA;
    f.seek(SeekFrom::Start(1 * ss))?;
    f.write_all(&fsinfo)?;

    f.seek(SeekFrom::Start(6 * ss))?;
    f.write_all(&bpb)?;

    let fat_byte_size = fat_sectors as usize * ss as usize;
    let mut fat = vec![0u8; fat_byte_size];
    fat[0..4].copy_from_slice(&[0xF8, 0xFF, 0xFF, 0x0F]);
    fat[4..8].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0x0F]);
    fat[8..12].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0x0F]);

    let fat1_lba = rsvd as u64;
    f.seek(SeekFrom::Start(fat1_lba * ss))?;
    f.write_all(&fat)?;
    f.seek(SeekFrom::Start((fat1_lba + fat_sectors as u64) * ss))?;
    f.write_all(&fat)?;

    let content = b"ROS Disk Test - FAT32 driver is working!\nHello from the kernel!\n";
    let csize = spc as usize * ss as usize;
    let mut root = vec![0u8; csize];

    root[0..8].copy_from_slice(b"README  ");
    root[8..11].copy_from_slice(b"TXT");
    root[0x0B] = 0x20;
    w16(&mut root, 0x1A, 3);
    w16(&mut root, 0x14, 0);
    w32(&mut root, 0x1C, content.len() as u32);

    f.seek(SeekFrom::Start(first_data_sector * ss))?;
    f.write_all(&root)?;

    f.seek(SeekFrom::Start((first_data_sector + spc as u64) * ss))?;
    f.write_all(content)?;

    let eoc = [0xFFu8, 0xFF, 0xFF, 0x0F];
    let mut write_fat = |cluster: u32, val: &[u8; 4]| -> std::io::Result<()> {
        let off = fat1_lba * ss + cluster as u64 * 4;
        f.seek(SeekFrom::Start(off))?;
        f.write_all(val)?;
        f.seek(SeekFrom::Start(off + fat_sectors as u64 * ss))?;
        f.write_all(val)
    };

    write_fat(3, &eoc)?;

    w32(&mut fsinfo, 0x1E4, (total_clusters - 2) as u32);
    w32(&mut fsinfo, 0x1E8, 5);
    f.seek(SeekFrom::Start(1 * ss))?;
    f.write_all(&fsinfo)?;

    f.flush()?;
    Ok(())
}
