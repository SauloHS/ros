/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

use x86_64::VirtAddr;
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB};

pub fn load_elf(
    elf_data: &[u8],
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<VirtAddr, &'static str> {
    if elf_data.len() < 64 {
        return Err("elf: file too small");
    }
    if elf_data[0..4] != [0x7F, b'E', b'L', b'F'] {
        return Err("elf: bad magic");
    }
    if elf_data[4] != 2 {
        return Err("elf: not 64-bit");
    }

    let w16 = |off: usize| -> u16 { u16::from_le_bytes([elf_data[off], elf_data[off + 1]]) };
    let w32 = |off: usize| -> u32 {
        u32::from_le_bytes([
            elf_data[off],
            elf_data[off + 1],
            elf_data[off + 2],
            elf_data[off + 3],
        ])
    };
    let w64 = |off: usize| -> u64 {
        u64::from_le_bytes([
            elf_data[off],
            elf_data[off + 1],
            elf_data[off + 2],
            elf_data[off + 3],
            elf_data[off + 4],
            elf_data[off + 5],
            elf_data[off + 6],
            elf_data[off + 7],
        ])
    };

    let e_type = w16(16);
    if e_type != 2 {
        return Err("elf: not ET_EXEC");
    }
    let e_machine = w16(18);
    if e_machine != 0x3E {
        return Err("elf: not x86_64");
    }

    let entry = w64(24);
    let phoff = w64(32);
    let phentsize = w16(54);
    let phnum = w16(56);

    if phnum == 0 {
        return Err("elf: no program headers");
    }
    if phentsize != 56 {
        return Err("elf: unexpected program header size");
    }

    let phoff = phoff as usize;

    for i in 0..phnum {
        let ph_base = phoff + i as usize * 56;
        if ph_base + 56 > elf_data.len() {
            return Err("elf: program header out of bounds");
        }

        let p_type = w32(ph_base);
        if p_type != 1 {
            continue;
        }

        let p_offset = w64(ph_base + 8) as usize;
        let p_vaddr = w64(ph_base + 16);
        let p_filesz = w64(ph_base + 32) as usize;
        let p_memsz = w64(ph_base + 40) as usize;

        if p_memsz == 0 {
            continue;
        }

        let page_start = VirtAddr::new(p_vaddr).align_down(0x1000u64);
        let page_end_vaddr = VirtAddr::new(p_vaddr + p_memsz as u64 - 1u64);
        let start_page = Page::containing_address(page_start);
        let end_page = Page::containing_address(page_end_vaddr);
        let page_range = Page::range_inclusive(start_page, end_page);

        for page in page_range {
            let frame = frame_allocator
                .allocate_frame()
                .ok_or("elf: out of memory")?;
            let flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;
            unsafe {
                mapper
                    .map_to(page, frame, flags, frame_allocator)
                    .map_err(|_| "elf: map failed")?
                    .flush();
            }
        }

        let dst = page_start.as_mut_ptr::<u8>();
        let src = &elf_data[p_offset..p_offset + p_filesz.min(elf_data.len() - p_offset)];
        unsafe {
            for i in 0..src.len() {
                core::ptr::write_volatile(dst.add(i), src[i]);
            }
            if p_memsz > src.len() {
                for i in src.len()..p_memsz {
                    core::ptr::write_volatile(dst.add(i), 0);
                }
            }
        }
    }

    Ok(VirtAddr::new(entry))
}
