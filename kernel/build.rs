/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let kernel_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root_dir = kernel_dir.parent().unwrap().to_path_buf();

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
        .arg(root_dir.join("init.c"))
        .arg(root_dir.join("libros.c"))
        .arg("-Wl,-Ttext-segment=0x400000")
        .status()
        .expect("failed to compile init.c");
    assert!(status.success(), "gcc compilation failed");

    println!(
        "cargo:rerun-if-changed={}",
        root_dir.join("init.c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        root_dir.join("libros.c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        root_dir.join("libros.h").display()
    );

    println!("cargo:rustc-env=INIT_ELF_PATH={}", init_elf.display());
}
