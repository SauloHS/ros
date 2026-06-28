/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

use lazy_static::lazy_static;
use x86_64::VirtAddr;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

const KERNEL_STACK_SIZE: usize = 4096 * 8;
static mut KERNEL_STACK: [u8; KERNEL_STACK_SIZE] = [0; KERNEL_STACK_SIZE];

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::new(core::ptr::addr_of!(STACK) as u64);
            let stack_end = stack_start + STACK_SIZE as u64;
            stack_end
        };
        tss.privilege_stack_table[0] = {
            let stack_start = VirtAddr::new(core::ptr::addr_of!(KERNEL_STACK) as u64);
            stack_start + KERNEL_STACK_SIZE as u64
        };
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.append(Descriptor::kernel_code_segment());
        let data_selector = gdt.append(Descriptor::kernel_data_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
        // syscall/sysret requires: user_data at index N, user_code at N+1
        // so that STAR[63:48]+8 = user_data and STAR[63:48]+16 = user_code
        let user_data_selector = gdt.append(Descriptor::user_data_segment());
        let user_code_selector = gdt.append(Descriptor::user_code_segment());
        (
            gdt,
            Selectors {
                code_selector,
                data_selector,
                tss_selector,
                user_code_selector,
                user_data_selector,
            },
        )
    };
}

pub struct Selectors {
    pub code_selector: SegmentSelector,
    pub data_selector: SegmentSelector,
    pub tss_selector: SegmentSelector,
    pub user_code_selector: SegmentSelector,
    pub user_data_selector: SegmentSelector,
}

pub fn selectors() -> &'static Selectors {
    &GDT.1
}

pub fn kernel_stack_top() -> u64 {
    let stack_start = VirtAddr::new(unsafe { core::ptr::addr_of!(KERNEL_STACK) as u64 });
    (stack_start + KERNEL_STACK_SIZE as u64).as_u64()
}

#[repr(C)]
struct SyscallStack {
    user_rsp: u64,
    kernel_rsp: u64,
}

static mut SYSCALL_STACK: SyscallStack = SyscallStack {
    user_rsp: 0,
    kernel_rsp: 0,
};

pub fn init_syscall() {
    unsafe {
        SYSCALL_STACK.kernel_rsp = kernel_stack_top();

        // Enable SCE (Syscall Enable) in EFER (MSR 0xC0000080)
        let mut efer_lo: u32;
        let mut efer_hi: u32;
        core::arch::asm!("rdmsr", in("ecx") 0xC0000080u32, out("eax") efer_lo, out("edx") efer_hi);
        let efer = (efer_hi as u64) << 32 | efer_lo as u64;
        let efer = efer | 1; // set SCE bit
        core::arch::asm!("wrmsr", in("ecx") 0xC0000080u32, in("eax") efer as u32,
            in("edx") (efer >> 32) as u32);

        // Set KernelGSBase (MSR 0xC0000102) to point to SYSCALL_STACK.
        // swapgs exchanges GS.base with this MSR, so the kernel can access
        // the per-CPU area via gs: after the first swapgs.
        let gs_base = core::ptr::addr_of!(SYSCALL_STACK) as u64;
        core::arch::asm!("wrmsr", in("ecx") 0xC0000102u32, in("eax") gs_base as u32,
            in("edx") (gs_base >> 32) as u32);

        // STAR MSR (0xC0000081):
        //   bits 47:32 = kernel CS = 0x08 (code_selector)
        //   bits 63:48 = user_cs_base such that:
        //     SYSRET: CS = base+16 = 0x30|3, SS = base+8 = 0x28|3
        //     => base = 0x30 - 16 = 0x20
        let user_cs_selector = selectors().user_code_selector.0 & 0xFFF8;
        let star_val =
            ((user_cs_selector as u64 - 16) << 48) | ((selectors().code_selector.0 as u64) << 32);
        core::arch::asm!("wrmsr", in("ecx") 0xC0000081u32, in("eax") star_val as u32,
            in("edx") (star_val >> 32) as u32);

        // LSTAR MSR (0xC0000082): handler address
        let handler = crate::arch::x86_64::syscall::syscall_entry as *const () as u64;
        core::arch::asm!("wrmsr", in("ecx") 0xC0000082u32, in("eax") handler as u32,
            in("edx") (handler >> 32) as u32);

        // SF_MASK MSR (0xC0000084): clear IF (bit 9) on syscall entry
        let mask: u64 = 0x200;
        core::arch::asm!("wrmsr", in("ecx") 0xC0000084u32, in("eax") mask as u32,
            in("edx") (mask >> 32) as u32);
    }
}

pub fn init() {
    use x86_64::instructions::segmentation::{CS, SS, Segment};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        SS::set_reg(GDT.1.data_selector);
        load_tss(GDT.1.tss_selector);
    }
}
