/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::drivers::disk::ata::{Drive, SECTOR_SIZE};

const FAT32_PTYPE: u8 = 0x0B;
const FAT32_LBA_PTYPE: u8 = 0x0C;

const ATTR_DIRECTORY: u8 = 0x10;
const ATTR_VOLUME_ID: u8 = 0x08;
const ATTR_LFN: u8 = 0x0F;

const EOC_MARK: u32 = 0x0FFFFFF8;

fn rd_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn rd_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

#[derive(Debug)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u32,
    pub first_cluster: u32,
}

#[allow(dead_code)]
pub struct Fat32 {
    drive: Drive,
    partition_lba: u32,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fats: u8,
    sectors_per_fat: u32,
    root_cluster: u32,
    first_data_sector: u32,
}

impl Fat32 {
    pub fn new(mut drive: Drive) -> Result<Self, &'static str> {
        let mut sector0 = [0u8; SECTOR_SIZE];
        drive.read_sectors(0, &mut sector0)?;

        if sector0[510] != 0x55 || sector0[511] != 0xAA {
            return Err("fat32: no boot signature at sector 0");
        }

        let partition_lba = Self::find_fat32_partition(&sector0);

        let bpb = if let Some(part_lba) = partition_lba {
            let mut bpb = [0u8; SECTOR_SIZE];
            drive.read_sectors(part_lba, &mut bpb)?;
            if bpb[510] != 0x55 || bpb[511] != 0xAA {
                return Err("fat32: bad BPB signature in partition");
            }
            bpb
        } else {
            if rd_u16(&sector0, 0x0B) == SECTOR_SIZE as u16 {
                sector0
            } else {
                return Err("fat32: no FAT32 partition found");
            }
        };

        Self::parse_bpb(drive, bpb, partition_lba.unwrap_or(0))
    }

    fn find_fat32_partition(sector0: &[u8; 512]) -> Option<u32> {
        for i in 0..4 {
            let off = 446 + i * 16;
            let ptype = sector0[off + 4];
            if ptype == FAT32_PTYPE || ptype == FAT32_LBA_PTYPE {
                return Some(rd_u32(sector0, off + 8));
            }
        }
        None
    }

    fn parse_bpb(
        drive: Drive,
        bpb: [u8; SECTOR_SIZE],
        partition_lba: u32,
    ) -> Result<Self, &'static str> {
        let bytes_per_sector = rd_u16(&bpb, 0x0B);
        if bytes_per_sector != SECTOR_SIZE as u16 {
            return Err("fat32: unsupported bytes per sector");
        }

        let sectors_per_cluster = bpb[0x0D];
        let reserved_sectors = rd_u16(&bpb, 0x0E);
        let fats = bpb[0x10];

        let sectors_per_fat_16 = rd_u16(&bpb, 0x16);
        let total_sectors_16 = rd_u16(&bpb, 0x13);
        let total_sectors_32 = rd_u32(&bpb, 0x20);

        let total_sectors = if total_sectors_16 != 0 {
            total_sectors_16 as u32
        } else {
            total_sectors_32
        };

        let sectors_per_fat = if sectors_per_fat_16 != 0 {
            sectors_per_fat_16 as u32
        } else {
            rd_u32(&bpb, 0x24)
        };

        let root_cluster = rd_u32(&bpb, 0x2C);

        let first_data_sector =
            partition_lba + reserved_sectors as u32 + (fats as u32) * sectors_per_fat;

        if sectors_per_cluster == 0 || !sectors_per_cluster.is_power_of_two() {
            return Err("fat32: invalid sectors per cluster");
        }

        let _ = total_sectors; // unused in read-only, but kept for validation

        Ok(Fat32 {
            drive,
            partition_lba,
            sectors_per_cluster,
            reserved_sectors,
            fats,
            sectors_per_fat,
            root_cluster,
            first_data_sector,
        })
    }

    fn cluster_to_lba(&self, cluster: u32) -> u32 {
        self.first_data_sector + (cluster - 2) * (self.sectors_per_cluster as u32)
    }

    fn cluster_byte_size(&self) -> usize {
        (self.sectors_per_cluster as usize) * SECTOR_SIZE
    }

    fn read_fat_entry(&mut self, cluster: u32) -> Result<u32, &'static str> {
        let fat_off = (cluster as usize) * 4;
        let fat_base = self.partition_lba + self.reserved_sectors as u32;
        let sector_idx = fat_off / SECTOR_SIZE;
        let byte_off = fat_off % SECTOR_SIZE;

        let mut sector = [0u8; SECTOR_SIZE];
        self.drive
            .read_sectors(fat_base + sector_idx as u32, &mut sector)?;

        let val = rd_u32(&sector, byte_off);
        Ok(val & 0x0FFFFFFF)
    }

    fn is_eoc(val: u32) -> bool {
        val >= EOC_MARK
    }

    fn read_cluster(&mut self, cluster: u32, buf: &mut [u8]) -> Result<(), &'static str> {
        let lba = self.cluster_to_lba(cluster);
        let size = self.cluster_byte_size();
        if buf.len() < size {
            return Err("fat32: buffer too small for cluster");
        }
        self.drive.read_sectors(lba, &mut buf[..size])
    }

    fn read_dir_entries(&mut self, cluster: u32) -> Result<Vec<DirEntry>, &'static str> {
        let mut entries = Vec::new();
        let mut cur = cluster;
        let csize = self.cluster_byte_size();

        loop {
            let mut buf = alloc::vec![0u8; csize];
            self.read_cluster(cur, &mut buf)?;

            for off in (0..csize).step_by(32) {
                let first = buf[off];
                if first == 0x00 {
                    return Ok(entries);
                }
                if first == 0xE5 {
                    continue;
                }

                let attrs = buf[off + 0x0B];
                if attrs == ATTR_LFN || attrs & ATTR_VOLUME_ID != 0 {
                    continue;
                }

                let name_raw = &buf[off..off + 8];
                let ext_raw = &buf[off + 8..off + 11];

                let name = Self::parse_name(name_raw, ext_raw);

                let cluster_hi = rd_u16(&buf, off + 0x14);
                let cluster_lo = rd_u16(&buf, off + 0x1A);
                let first_cluster = (cluster_hi as u32) << 16 | cluster_lo as u32;
                let size = rd_u32(&buf, off + 0x1C);

                entries.push(DirEntry {
                    name,
                    is_dir: attrs & ATTR_DIRECTORY != 0,
                    size,
                    first_cluster,
                });
            }

            cur = self.read_fat_entry(cur)?;
            if Self::is_eoc(cur) {
                break;
            }
        }

        Ok(entries)
    }

    fn parse_name(name: &[u8], ext: &[u8]) -> String {
        fn trim(s: &[u8]) -> &str {
            let end = s
                .iter()
                .rposition(|&c| c != b' ')
                .map(|i| i + 1)
                .unwrap_or(0);
            core::str::from_utf8(&s[..end]).unwrap_or("")
        }
        let n = trim(name);
        let e = trim(ext);
        if e.is_empty() {
            n.to_string()
        } else {
            alloc::format!("{}.{}", n, e)
        }
    }

    pub fn root_entries(&mut self) -> Result<Vec<DirEntry>, &'static str> {
        self.read_dir_entries(self.root_cluster)
    }

    pub fn open_file(&mut self, path: &str) -> Result<Vec<u8>, &'static str> {
        let path = path.trim_start_matches('/');
        if path.is_empty() {
            return Err("fat32: empty path");
        }

        let parts: Vec<&str> = path.split('/').collect();
        let mut cur_cluster = self.root_cluster;

        for (i, part) in parts.iter().enumerate() {
            let entries = self.read_dir_entries(cur_cluster)?;
            let is_last = i == parts.len() - 1;

            let found = entries.iter().find(|e| e.name == *part);
            match found {
                Some(e) if is_last => {
                    if e.is_dir {
                        return Err("fat32: path is a directory, not a file");
                    }
                    return self.read_file_data(e.first_cluster, e.size);
                }
                Some(e) if e.is_dir => {
                    cur_cluster = e.first_cluster;
                }
                Some(_) => return Err("fat32: intermediate path component is not a directory"),
                None => return Err("fat32: path component not found"),
            }
        }

        Err("fat32: path is a directory")
    }

    fn read_file_data(&mut self, first_cluster: u32, size: u32) -> Result<Vec<u8>, &'static str> {
        let size = size as usize;
        let mut data = alloc::vec![0u8; size];

        if size == 0 {
            return Ok(data);
        }

        let mut cluster = first_cluster;
        let csize = self.cluster_byte_size();
        let mut offset = 0;

        while !Self::is_eoc(cluster) && offset < size {
            let mut buf = alloc::vec![0u8; csize];
            self.read_cluster(cluster, &mut buf)?;

            let to_copy = core::cmp::min(csize, size - offset);
            data[offset..offset + to_copy].copy_from_slice(&buf[..to_copy]);
            offset += to_copy;

            cluster = self.read_fat_entry(cluster)?;
        }

        Ok(data)
    }

    #[allow(dead_code)]
    pub fn volume_label(&mut self) -> Result<Option<String>, &'static str> {
        let mut bpb = [0u8; SECTOR_SIZE];
        self.drive.read_sectors(self.partition_lba, &mut bpb)?;
        let sig = bpb[0x42];
        if sig != 0x29 && sig != 0x28 {
            return Ok(None);
        }
        let label_raw = &bpb[0x47..0x52];
        let end = label_raw
            .iter()
            .rposition(|&c| c != b' ')
            .map(|i| i + 1)
            .unwrap_or(0);
        if end == 0 {
            return Ok(None);
        }
        let label = core::str::from_utf8(&label_raw[..end])
            .map(|s| s.to_string())
            .ok();
        Ok(label)
    }
}
