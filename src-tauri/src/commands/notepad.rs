#[cfg(windows)]
use std::{
    ffi::OsString,
    io,
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    process::{Command, Stdio},
};

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetSystemDirectoryW(buffer: *mut u16, size: u32) -> u32;
}

#[cfg(windows)]
fn trusted_notepad_path() -> Result<PathBuf, String> {
    // Read from Windows itself, not PATH, the current directory, or environment variables.
    let mut buffer = vec![0u16; 32_768];
    // SAFETY: buffer is writable for exactly buffer.len() UTF-16 code units, and
    // GetSystemDirectoryW writes at most the supplied capacity and retains no pointer.
    let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if length == 0 {
        return Err(format!(
            "Could not locate the Windows system directory: {}",
            io::Error::last_os_error()
        ));
    }
    if length >= buffer.len() {
        return Err("Windows returned a system-directory path that is too long.".into());
    }
    let directory = PathBuf::from(OsString::from_wide(&buffer[..length]));
    if !directory.is_absolute() {
        return Err("Windows did not return an absolute system-directory path.".into());
    }
    Ok(directory.join("notepad.exe"))
}

#[cfg(windows)]
fn launch_error(error: io::Error) -> String {
    match error.kind() {
        io::ErrorKind::NotFound => "Windows Notepad could not be found. Install or repair Notepad in Windows Settings, then try again.".into(),
        io::ErrorKind::PermissionDenied => "Windows denied permission to open Notepad. Check your device's application restrictions.".into(),
        _ => format!("Windows could not open Notepad. Please try again. Details: {error}"),
    }
}

#[cfg(windows)]
pub(super) fn launch() -> Result<(), String> {
    let executable = trusted_notepad_path()?;
    let directory = executable
        .parent()
        .ok_or("Windows returned an invalid Notepad location.")?;
    // No shell, arguments, caller-supplied path, or executable-name fallback.
    Command::new(&executable)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_child| ())
        .map_err(launch_error)
}

#[cfg(not(windows))]
pub(super) fn launch() -> Result<(), String> {
    Err("Open Notepad is available only on Windows.".into())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn resolves_absolute_system_notepad_without_launching() {
        let executable = trusted_notepad_path().unwrap();
        assert!(executable.is_absolute());
        assert_eq!(executable.file_name().unwrap(), "notepad.exe");
    }

    #[test]
    fn explains_common_launch_failures() {
        assert!(
            launch_error(io::Error::from(io::ErrorKind::NotFound)).contains("Install or repair")
        );
        assert!(
            launch_error(io::Error::from(io::ErrorKind::PermissionDenied))
                .contains("denied permission")
        );
        assert!(launch_error(io::Error::other("test failure")).contains("test failure"));
    }
}
