//! The built program carries the app icon.
#![cfg(windows)]
// Reading resources goes through the Windows loader, which needs `unsafe`.
#![allow(unsafe_code)]

use windows::Win32::Foundation::FreeLibrary;
use windows::Win32::System::LibraryLoader::{
    FindResourceW, LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE, LoadLibraryExW,
    LoadResource, LockResource, SizeofResource,
};
use windows::core::{HSTRING, PCWSTR};

/// Resource type of an icon group.
const RT_GROUP_ICON: usize = 14;

#[test]
fn the_program_has_an_icon_group_of_every_size() {
    let exe = HSTRING::from(env!("CARGO_BIN_EXE_endlogic-win"));
    // SAFETY: maps the file as data only; nothing in it runs.
    let module = unsafe {
        LoadLibraryExW(
            &exe,
            None,
            LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
        )
    }
    .unwrap();
    // SAFETY: integer resource ids, as MAKEINTRESOURCE makes them.
    let found = unsafe {
        FindResourceW(
            Some(module),
            PCWSTR(std::ptr::without_provenance(1)),
            PCWSTR(std::ptr::without_provenance(RT_GROUP_ICON)),
        )
    };
    assert!(!found.is_invalid(), "no icon group 1");
    // SAFETY: the resource was just found in this module.
    let size = unsafe { SizeofResource(Some(module), found) } as usize;
    // SAFETY: as above.
    let handle = unsafe { LoadResource(Some(module), found) }.unwrap();
    // SAFETY: a loaded resource stays mapped while the module is.
    let data = unsafe { LockResource(handle) }.cast::<u8>();
    assert!(!data.is_null());
    // SAFETY: `size` bytes of the locked resource, copied out.
    let bytes = unsafe { std::slice::from_raw_parts(data, size) }.to_vec();
    // SAFETY: the module was loaded above and is no longer used.
    unsafe { FreeLibrary(module) }.unwrap();

    // GRPICONDIR: reserved, type 1, count, then 14-byte entries.
    assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 1);
    let count = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
    let mut sizes: Vec<u32> = (0..count)
        .map(|k| match bytes[6 + 14 * k] {
            0 => 256,
            w => u32::from(w),
        })
        .collect();
    sizes.sort_unstable();
    assert_eq!(sizes, [16, 20, 24, 32, 40, 48, 64, 256]);
}
