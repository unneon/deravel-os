use crate::page::PAGE_SIZE;
use crate::page::{PageAligned, PageFlags};
use crate::{PageTable, map_pages};
use alloc::vec;
use alloc::vec::Vec;
use elf::ElfBytes;
use elf::abi::{EM_RISCV, ET_EXEC, PF_R, PF_W, PF_X, PT_LOAD};
use elf::endian::LittleEndian;
use elf::file::Class;
use elf::segment::ProgramHeader;

const USER_START: usize = 0x1000000;
const USER_END: usize = 0x1800000;

pub macro const_elf($name:ident $path:literal) {
    const $name: PageAligned<[u8; include_bytes!(env!($path)).len()]> =
        PageAligned(*include_bytes!(env!($path)));
}

pub fn load_elf(elf_bytes: &[u8], page_table: &mut PageTable) -> usize {
    let elf = ElfBytes::<LittleEndian>::minimal_parse(elf_bytes).unwrap();
    assert_eq!(elf.ehdr.class, Class::ELF64);
    assert_eq!(elf.ehdr.endianness, LittleEndian);
    assert_eq!(elf.ehdr.version, 1);
    assert_eq!(elf.ehdr.osabi, 0);
    assert_eq!(elf.ehdr.abiversion, 0);
    assert_eq!(elf.ehdr.e_type, ET_EXEC);
    assert_eq!(elf.ehdr.e_machine, EM_RISCV);
    // TODO: Consider phoff, shoff, flags, ehsize, phentsize, phnum, shentsize, shnum, shstrndx.

    let segments = elf.segments().unwrap();
    for segment in segments {
        if segment.p_type != PT_LOAD {
            continue;
        }

        assert!(segment.p_vaddr.is_multiple_of(PAGE_SIZE as u64));
        assert!(segment.p_vaddr as usize >= USER_START);
        assert!(segment.p_filesz <= segment.p_memsz);
        assert!(segment.p_memsz as usize <= USER_END - USER_START);
        assert!(segment.p_vaddr + segment.p_memsz <= USER_END as u64);
        assert_eq!(segment.p_align, PAGE_SIZE as u64);

        let data = elf.segment_data(&segment).unwrap();
        let flags = paging_flags(&segment);

        if flags.is_writable() {
            let page_count = (segment.p_memsz as usize).div_ceil(PAGE_SIZE);
            let pages = vec![0u8; PAGE_SIZE * page_count];

            map_pages(
                page_table,
                segment.p_vaddr as usize,
                pages.as_ptr() as usize,
                flags,
                page_count,
            );

            let flat_pointer = Vec::leak(pages).as_mut_ptr();
            let flat_memory =
                unsafe { core::slice::from_raw_parts_mut(flat_pointer, PAGE_SIZE * page_count) };
            flat_memory[..segment.p_filesz as usize].copy_from_slice(data);
            flat_memory[segment.p_memsz as usize..].fill(0);
        } else {
            assert!((data.as_ptr() as usize).is_multiple_of(PAGE_SIZE));
            assert!(elf_data_is_zero_padded(&segment, elf_bytes));

            map_pages(
                page_table,
                segment.p_vaddr as usize,
                data.as_ptr() as usize,
                flags,
                data.len().div_ceil(PAGE_SIZE),
            );
        }
    }

    elf.ehdr.e_entry as usize
}

fn paging_flags(segment: &ProgramHeader) -> PageFlags {
    let readable = segment.p_flags & PF_R != 0;
    let writable = segment.p_flags & PF_W != 0;
    let executable = segment.p_flags & PF_X != 0;
    assert!(readable);
    assert!(!(writable && executable));
    if writable {
        PageFlags::readwrite().user()
    } else if executable {
        PageFlags::executable().user()
    } else {
        PageFlags::readonly().user()
    }
}

fn elf_data_is_zero_padded(segment: &ProgramHeader, elf_bytes: &[u8]) -> bool {
    let file_segment_start = segment.p_offset as usize;
    let file_segment_fake_end = file_segment_start + segment.p_filesz as usize;
    let file_segment_real_end =
        (file_segment_start + segment.p_memsz as usize).next_multiple_of(PAGE_SIZE);
    elf_bytes[file_segment_fake_end..file_segment_real_end]
        .iter()
        .all(|&b| b == 0)
}
