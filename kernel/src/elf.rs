use crate::heap::granularity::page_granular_vec;
use crate::page::{PageFlags, virt_to_phys};
use crate::process::Process;
use crate::util::untyped_box::UntypedBox;
use alloc::sync::Arc;
use core::marker::PhantomData;
use deravel_types::PAGE_SIZE;
use deravel_types::memory::USER_ELF;
use elf::ElfBytes;
use elf::abi::{EM_RISCV, ET_EXEC, PF_R, PF_W, PF_X, PT_LOAD};
use elf::endian::LittleEndian;
use elf::file::Class;
use elf::segment::ProgramHeader;

pub macro elf($ty:ty, $env:literal) {{
    {
        #[cfg(debug_assertions)]
        static ELF: &Elf<
            $ty,
            [u8; include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../target/riscv64gc-unknown-deravel/debug/",
                $env
            ))
            .len()],
        > = &Elf(
            PhantomData,
            *include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../target/riscv64gc-unknown-deravel/debug/",
                $env
            )),
        );
        #[cfg(not(debug_assertions))]
        static ELF: &Elf<
            $ty,
            [u8; include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../target/riscv64gc-unknown-deravel/release/",
                $env
            ))
            .len()],
        > = &Elf(
            PhantomData,
            *include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../target/riscv64gc-unknown-deravel/release/",
                $env
            )),
        );
        ELF
    }
}}

#[repr(align(4096))]
pub struct Elf<T, U: ?Sized>(pub PhantomData<T>, pub U);

pub fn load_elf<T, U: AsRef<[u8]>>(elf_bytes: &'static Elf<T, U>, proc: &mut Process) {
    let elf = ElfBytes::<LittleEndian>::minimal_parse(elf_bytes.1.as_ref()).unwrap();
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
        assert!(segment.p_vaddr as usize >= USER_ELF.start);
        assert!(segment.p_filesz <= segment.p_memsz);
        assert!(segment.p_memsz as usize <= USER_ELF.end - USER_ELF.start);
        assert!(segment.p_vaddr + segment.p_memsz <= USER_ELF.end as u64);
        assert_eq!(segment.p_align, PAGE_SIZE as u64);

        let data = elf.segment_data(&segment).unwrap();
        let flags = paging_flags(&segment);

        if flags.is_writable() {
            let size = (segment.p_memsz as usize).next_multiple_of(PAGE_SIZE);

            let mut pages = page_granular_vec![0u8; size];
            pages[..segment.p_filesz as usize].copy_from_slice(data);
            let pages = Arc::new(UntypedBox::new(pages.into_boxed_slice()));
            proc.alloc_at(segment.p_vaddr as usize, pages, flags);
        } else {
            assert!((data.as_ptr() as usize).is_multiple_of(PAGE_SIZE));
            assert!(elf_data_is_zero_padded(&segment, elf_bytes.1.as_ref()));

            proc.page_table.map(
                segment.p_vaddr as usize,
                virt_to_phys(data.as_ptr() as usize),
                data.len().next_multiple_of(PAGE_SIZE),
                flags,
            );
        }
    }

    proc.pc = elf.ehdr.e_entry as usize;
}

fn paging_flags(segment: &ProgramHeader) -> PageFlags {
    let readable = segment.p_flags & PF_R != 0;
    let writable = segment.p_flags & PF_W != 0;
    let executable = segment.p_flags & PF_X != 0;
    assert!(readable);
    assert!(!(writable && executable));
    if writable {
        PageFlags::read_write().user()
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
