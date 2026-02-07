use crate::{PAGE_R, PAGE_U, PAGE_W, PAGE_X, PageTable, map_page};
use alloc::vec;
use alloc::vec::Vec;
use elf::ElfBytes;
use elf::abi::{EM_RISCV, ET_EXEC, PT_LOAD};
use elf::endian::LittleEndian;
use elf::file::Class;

pub fn load_elf(bytes: &[u8], page_table: &mut PageTable) {
    let elf = ElfBytes::<LittleEndian>::minimal_parse(bytes).unwrap();
    assert_eq!(elf.ehdr.class, Class::ELF64);
    assert_eq!(elf.ehdr.endianness, LittleEndian);
    assert_eq!(elf.ehdr.version, 1);
    assert_eq!(elf.ehdr.osabi, 0);
    assert_eq!(elf.ehdr.abiversion, 0);
    assert_eq!(elf.ehdr.e_type, ET_EXEC);
    assert_eq!(elf.ehdr.e_machine, EM_RISCV);
    assert_eq!(elf.ehdr.e_entry, 0x1000000);
    // TODO: Consider phoff, shoff, flags, ehsize, phentsize, phnum, shentsize, shnum, shstrndx.

    let segments = elf.segments().unwrap();
    for segment in segments {
        if segment.p_type != PT_LOAD {
            continue;
        }

        assert!(segment.p_vaddr.is_multiple_of(4096));
        assert!(segment.p_vaddr >= 0x1000000);
        assert!(segment.p_filesz <= segment.p_memsz);
        assert!(segment.p_memsz <= 0x1800000 - 0x1000000);
        assert!(segment.p_vaddr + segment.p_memsz <= 0x1800000);
        assert_eq!(segment.p_align, 4096);
        // TODO: Validate p_flags

        let page_count = segment.p_memsz.div_ceil(4096) as usize;
        let pages = vec![0u8; 4096 * page_count];

        for i in 0..page_count {
            map_page(
                page_table,
                segment.p_vaddr as usize + 4096 * i,
                pages.as_ptr() as usize + 4096 * i,
                // TODO: Apply p_flags.
                PAGE_U | PAGE_R | PAGE_W | PAGE_X,
            );
        }

        let data = elf.segment_data(&segment).unwrap();
        let flat_pointer = Vec::leak(pages).as_mut_ptr();
        let flat_memory =
            unsafe { core::slice::from_raw_parts_mut(flat_pointer, page_count * 4096) };
        flat_memory[..segment.p_filesz as usize].copy_from_slice(data);
        flat_memory[segment.p_memsz as usize..].fill(0);
    }
}
