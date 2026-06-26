/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

/*
File created by Saulo Henrique Santos Dorotéio.
Last updated by Saulo Henrique Santos Dorotéio, at 06/22/2026.
See LICENSE file for licensing information */

use std::{env, path::PathBuf, process::Command};

fn main() {
    let kernel = env::var_os("CARGO_BIN_FILE_KERNEL_kernel").unwrap();
    let kernel_path = PathBuf::from(kernel);
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    let bios_path = out_dir.join("bios.img");
    bootloader::BiosBoot::new(&kernel_path)
        .create_disk_image(&bios_path)
        .unwrap();

    println!("cargo:rustc-env=BIOS_PATH={}", bios_path.display());

    let init_elf = out_dir.join("init.elf");
    let status = Command::new("gcc")
        .args([
            "-m64",
            "-ffreestanding",
            "-nostdlib",
            "-static",
            "-no-pie",
            "-fno-stack-protector",
            "-o",
        ])
        .arg(&init_elf)
        .args(["init.c", "libros.c", "-Wl,-Ttext-segment=0x400000"])
        .status()
        .expect("failed to compile init.c");
    assert!(status.success(), "gcc compilation of init.c failed");

    println!("cargo:rerun-if-changed=init.c");
    println!("cargo:rerun-if-changed=libros.c");
    println!("cargo:rerun-if-changed=libros.h");
    println!("cargo:rustc-env=INIT_ELF_PATH={}", init_elf.display());
}
