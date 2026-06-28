/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

use alloc::string::String;
use alloc::vec::Vec;

pub struct InitRamFs {
    files: Vec<(String, Vec<u8>)>,
}

impl InitRamFs {
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn lookup(&self, path: &str) -> Option<&[u8]> {
        let normalized = if path.starts_with('/') { &path[1..] } else { path };
        self.files
            .iter()
            .find(|(p, _)| p.as_str() == normalized)
            .map(|(_, d)| d.as_slice())
    }
}

fn parse_hex(buf: &[u8]) -> u64 {
    let s = core::str::from_utf8(buf).unwrap_or("0");
    u64::from_str_radix(s, 16).unwrap_or(0)
}

pub fn extract(data: &[u8]) -> Result<InitRamFs, &'static str> {
    let mut files = Vec::new();
    let mut off = 0;

    while off + 110 <= data.len() {
        let hdr = &data[off..off + 110];
        if &hdr[0..6] != b"070701" {
            return Err("bad CPIO magic");
        }

        let namesize = parse_hex(&hdr[94..102]) as usize;
        let filesize = parse_hex(&hdr[54..62]) as usize;

        let name_start = off + 110;
        let name_end = name_start + namesize;
        if name_end > data.len() {
            return Err("name out of bounds");
        }
        let name_bytes = &data[name_start..name_end - 1];
        let name = core::str::from_utf8(name_bytes).map_err(|_| "invalid UTF-8")?;

        if name == "TRAILER!!!" {
            break;
        }

        let data_start = align4(name_end);
        let data_end = align4(data_start + filesize);
        if data_end > data.len() {
            return Err("file data out of bounds");
        }

        let content = data[data_start..data_start + filesize].to_vec();
        files.push((name.into(), content));
        off = data_end;
    }

    Ok(InitRamFs { files })
}

fn align4(n: usize) -> usize {
    (n + 3) & !3
}
