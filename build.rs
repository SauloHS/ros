/*
File created by Saulo Henrique Santos Dorotéio.
Last updated by Saulo Henrique Santos Dorotéio, at 06/22/2026.
See LICENSE file for licensing information */

use std::{env, path::PathBuf};

fn main() {
    let kernel = env::var_os("CARGO_BIN_FILE_KERNEL_kernel").unwrap();
    let kernel_path = PathBuf::from(kernel);
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    let bios_path = out_dir.join("bios.img");
    bootloader::BiosBoot::new(&kernel_path)
        .create_disk_image(&bios_path)
        .unwrap();

    println!("cargo:rustc-env=BIOS_PATH={}", bios_path.display());
} 