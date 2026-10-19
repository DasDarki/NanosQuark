#[cfg(unix)]
pub fn pin_library() {
    unsafe {
        let mut info: libc::Dl_info = std::mem::zeroed();
        if libc::dladdr(pin_library as *const libc::c_void, &mut info) != 0
            && !info.dli_fname.is_null()
        {
            libc::dlopen(
                info.dli_fname,
                libc::RTLD_NOW | libc::RTLD_NOLOAD | libc::RTLD_NODELETE,
            );
        }
    }
}

#[cfg(windows)]
pub fn pin_library() {
    use windows_sys::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN, GetModuleHandleExW,
    };

    unsafe {
        let mut module = std::ptr::null_mut();
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            pin_library as *const u16,
            &mut module,
        );
    }
}
