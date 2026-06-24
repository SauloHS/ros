/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

use std::{env, path::Path, path::PathBuf, process::Command};

fn main() {
    let kernel_bin = env::args().nth(1).expect("usage: test-runner <kernel-binary> [output-path]");
    let output = env::args().nth(2).unwrap_or_else(|| {
        let mut p = PathBuf::from(&kernel_bin);
        p.set_extension("img");
        p.to_string_lossy().to_string()
    });

    bootloader::BiosBoot::new(Path::new(&kernel_bin))
        .create_disk_image(Path::new(&output))
        .expect("failed to create boot image for test binary");

    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.arg("-drive")
        .arg(format!("format=raw,file={output}"))
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04")
        .arg("-no-reboot")
        .arg("-no-shutdown");

    if std::env::args().any(|a| a == "--debug") {
        cmd.arg("-d")
            .arg("int,cpu_reset")
            .arg("-D")
            .arg("qemu-test.log");
    }

    let status = cmd.status().expect("failed to run QEMU for tests");
    std::process::exit(status.code().unwrap_or(-1));
}
