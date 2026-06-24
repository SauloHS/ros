fn main() {
    let bios_path = env!("BIOS_PATH");

    let debug = std::env::args().any(|arg| arg == "--debug");

    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    cmd.arg("-drive").arg(format!("format=raw,file={bios_path}"));

    if debug {
        cmd.arg("-d").arg("int,cpu_reset")
           .arg("-D").arg("qemu.log")
           .arg("--no-reboot")
           .arg("--no-shutdown");
    }

    let status = cmd.status().unwrap();
    std::process::exit(status.code().unwrap_or(-1));
}