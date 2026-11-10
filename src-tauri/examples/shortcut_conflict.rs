//! Manual test fixture, never included in NEXUS. Run before NEXUS to reserve its
//! shortcut, then press Enter here to release it. Process exit also releases it.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{ffi::c_void, io};
    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterHotKey(window: *mut c_void, id: i32, modifiers: u32, key: u32) -> i32;
        fn UnregisterHotKey(window: *mut c_void, id: i32) -> i32;
    }
    // SAFETY: a null HWND registers on this thread. ID 1 belongs to this fixture.
    if unsafe { RegisterHotKey(std::ptr::null_mut(), 1, 0x4000 | 0x0002 | 0x0004, 0x20) } == 0 {
        return Err(io::Error::last_os_error().into());
    }
    println!(
        "Holding Ctrl+Shift+Space. Start NEXUS now; it should show a warning. Press Enter to release."
    );
    let read = io::stdin().read_line(&mut String::new());
    // SAFETY: unregister on the same thread that registered the shortcut above.
    let released = unsafe { UnregisterHotKey(std::ptr::null_mut(), 1) };
    read?;
    if released == 0 {
        return Err(io::Error::last_os_error().into());
    }
    println!("Shortcut released.");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This manual shortcut-conflict fixture is Windows-only.");
}
