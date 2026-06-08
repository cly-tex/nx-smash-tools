#![no_std]

use core::ffi::{c_char, c_void};

use object::LittleEndian;

pub type Dyn64 = object::elf::Dyn64<LittleEndian>;
pub type Sym64 = object::elf::Sym64<LittleEndian>;
pub type Rela64 = object::elf::Rela64<LittleEndian>;
pub type Rel64 = object::elf::Rel64<LittleEndian>;

#[repr(C)]
pub struct ModuleObject {
    next: *mut Self,
    prev: *mut Self,
    rela_or_rel_plt: *mut c_void,
    rela_or_rel: *mut c_void,
    module_base: *mut c_char,
    dynamic: *mut Dyn64,
    is_rela: bool,
    rela_or_rel_plt_size: u64,
    dt_init: Option<extern "C" fn()>,
    dt_fini: Option<extern "C" fn()>,
    hash_bucket: *mut u32,
    hash_chain: *mut u32,
    dynstr: *mut c_char,
    dynsym: *mut Sym64,
    dynstr_size: u64,
    got: *mut *mut c_void,
    rela_dyn_size: u64,
    rel_dyn_size: u64,
    rel_count: u64,
    rela_count: u64,
    hash_nchain_value: u64,
    hash_nbucket_value: u64,
    got_stub_ptr: *mut c_void,
    soname_idx: u64,
    nro_size: usize,
    cannot_revert_symbols: bool,
}

/// Performs basic relocations for a module, should primarily only be used while initializing a sysmodule
///
/// Reference taken from [exlaunch](https://github.com/dt-12345/exlaunch/blob/625d537f89b665dc14faa4ddad67ff96282c177f/source/rtld/relocation.cpp)
///
/// # Safety
/// - Caller must ensure that `aslr_base` is the address of the base of a module loaded into executable memory
/// - Caller must ensure that `dynamic` is non-null, properly aligned, and points to valid `Dyn64` values
pub unsafe fn relocate_raw(aslr_base: usize, mut dyn_ptr: *const Dyn64) {
    let mut rela = 0usize;
    let mut rel = 0usize;

    let mut rela_entry_size = size_of::<Rela64>();
    let mut rel_entry_size = size_of::<Rel64>();

    let mut rela_entry_count = 0usize;
    let mut rel_entry_count = 0usize;

    let mut rela_size = 0usize;
    let mut rel_size = 0usize;

    loop {
        let dynamic = unsafe { &*dyn_ptr };

        match dynamic.d_tag.get(LittleEndian) {
            object::elf::DT_NULL => break,
            object::elf::DT_RELA => rela = aslr_base + dynamic.d_val.get(LittleEndian) as usize,
            object::elf::DT_RELAENT => rela_entry_size = dynamic.d_val.get(LittleEndian) as usize,
            object::elf::DT_RELASZ => rela_size = dynamic.d_val.get(LittleEndian) as usize,
            object::elf::DT_REL => rel = aslr_base + dynamic.d_val.get(LittleEndian) as usize,
            object::elf::DT_RELENT => rel_entry_size = dynamic.d_val.get(LittleEndian) as usize,
            object::elf::DT_RELSZ => rel_size = dynamic.d_val.get(LittleEndian) as usize,
            object::elf::DT_RELACOUNT => {
                rela_entry_count = dynamic.d_val.get(LittleEndian) as usize
            }
            object::elf::DT_RELCOUNT => rel_entry_count = dynamic.d_val.get(LittleEndian) as usize,
            _ => {}
        }

        dyn_ptr = unsafe { dyn_ptr.add(1) };
    }

    if rela_entry_count == 0 {
        rela_entry_count = rela_size / rela_entry_size
    }

    if rel_entry_count == 0 {
        rel_entry_count = rel_size / rel_entry_size
    }

    for i in 0..rel_entry_count {
        let entry = unsafe { &*((rel + i * rel_entry_size) as *const Rel64) };
        if entry.r_type(LittleEndian) == object::elf::R_AARCH64_RELATIVE {
            let ptr = (aslr_base + entry.r_offset.get(LittleEndian) as usize) as *mut usize;
            unsafe { *ptr += aslr_base };
        }
    }

    for i in 0..rela_entry_count {
        let entry = unsafe { &*((rela + i * rel_entry_size) as *const Rela64) };
        if entry.r_type(LittleEndian, false) == object::elf::R_AARCH64_RELATIVE {
            let ptr = (aslr_base + entry.r_offset.get(LittleEndian) as usize) as *mut usize;
            unsafe {
                *ptr += aslr_base.wrapping_add_signed(entry.r_addend.get(LittleEndian) as isize)
            }
        }
    }
}
