use super::*;
use windows::{
    Win32::{
        Foundation::*,
        Security::*,
        System::{Power::*, Shutdown::LockWorkStation, Threading::*},
        UI::{Shell::*, WindowsAndMessaging::SW_SHOWNORMAL},
    },
    core::{PCWSTR, w},
};

fn failure(code: u32, action: &str) -> ActionError {
    let (kind, hint) = match code {
        5 | 1300 | 1314 => (
            ErrorCode::Restricted,
            "Windows policy or your account restricts this action. NEXUS will not request administrator elevation.",
        ),
        50 | 120 => (
            ErrorCode::Unsupported,
            "This action is not supported by this device or Windows configuration.",
        ),
        1155 | 31 => (
            ErrorCode::Unsupported,
            "The Windows Settings handler is unavailable. Check or repair Windows Settings.",
        ),
        _ => (
            ErrorCode::NativeFailure,
            "Check device support, Windows settings and system restrictions before trying again.",
        ),
    };
    ActionError::new(
        kind,
        format!("Could not {action} (Windows error {code}). {hint}"),
    )
}
pub(super) struct WindowsOperations;
impl Operations for WindowsOperations {
    fn settings(&self, page: SettingsPage) -> Result<Option<String>> {
        // Reuse the existing COM STA shell worker. Only static URI literals enter it.
        crate::installed_apps::native::on_sta(move || {
            let uri: Vec<u16> = page.uri().encode_utf16().chain(Some(0)).collect();
            let mut info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
                lpVerb: w!("open"),
                lpFile: PCWSTR(uri.as_ptr()),
                nShow: SW_SHOWNORMAL.0,
                ..Default::default()
            };
            // Nested Result preserves structured Shell errors across the shared worker.
            Ok(unsafe { ShellExecuteExW(&mut info) }
                .map(|_| None)
                .map_err(|error| failure(error.code().0 as u32 & 0xffff, "open Settings")))
        })
        .map_err(|message| ActionError::new(ErrorCode::NativeFailure, message))?
    }
    fn lock(&self) -> Result<Option<String>> {
        unsafe { LockWorkStation() }
            .map(|_| None)
            .map_err(|error| failure(error.code().0 as u32 & 0xffff, "lock this session"))
    }
    fn sleep(&self) -> Result<Option<String>> {
        let mut caps = SYSTEM_POWER_CAPABILITIES::default();
        if !unsafe { GetPwrCapabilities(&mut caps) } {
            return Err(failure(
                unsafe { GetLastError() }.0,
                "read sleep capabilities",
            ));
        }
        if !sleep_supported(&caps) {
            return Err(ActionError::new(
                ErrorCode::Unsupported,
                "This Windows configuration does not report a supported sleep state.",
            ));
        }
        let mut privilege = SleepPrivilege::enable()?;
        // No hibernation, no force request, and wake events remain enabled.
        let accepted = unsafe { SetSuspendState(false, false, false) };
        let code = unsafe { GetLastError() }.0;
        let restoration = privilege.restore();
        if !accepted {
            return Err(failure(code, "put this computer to sleep"));
        }
        Ok(restoration.err().map(|error| {
            format!(
                "Sleep was accepted, but restoring the token privilege failed. Restart NEXUS. {}",
                error.message
            )
        }))
    }
}
fn sleep_supported(caps: &SYSTEM_POWER_CAPABILITIES) -> bool {
    caps.SystemS1 || caps.SystemS2 || caps.SystemS3 || caps.AoAc
}

struct SleepPrivilege {
    token: HANDLE,
    previous: TOKEN_PRIVILEGES,
    restore_needed: bool,
}
impl SleepPrivilege {
    fn enable() -> Result<Self> {
        let mut token = HANDLE::default();
        unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
                &mut token,
            )
        }
        .map_err(|error| {
            failure(
                error.code().0 as u32 & 0xffff,
                "access the assigned sleep privilege",
            )
        })?;
        let mut guard = Self {
            token,
            previous: TOKEN_PRIVILEGES::default(),
            restore_needed: false,
        };
        let mut luid = LUID::default();
        unsafe { LookupPrivilegeValueW(None, SE_SHUTDOWN_NAME, &mut luid) }.map_err(|error| {
            failure(error.code().0 as u32 & 0xffff, "locate the sleep privilege")
        })?;
        let requested = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        let mut length = 0;
        unsafe {
            SetLastError(ERROR_SUCCESS);
        }
        unsafe {
            AdjustTokenPrivileges(
                token,
                false,
                Some(&requested),
                std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
                Some(&mut guard.previous),
                Some(&mut length),
            )
        }
        .map_err(|error| {
            failure(
                error.code().0 as u32 & 0xffff,
                "enable the assigned sleep privilege",
            )
        })?;
        let error = unsafe { GetLastError() };
        if error != ERROR_SUCCESS {
            return Err(failure(error.0, "enable the assigned sleep privilege"));
        }
        guard.restore_needed = true;
        Ok(guard)
    }
    fn restore(&mut self) -> Result<()> {
        if self.restore_needed {
            unsafe {
                SetLastError(ERROR_SUCCESS);
            }
            unsafe {
                AdjustTokenPrivileges(self.token, false, Some(&self.previous), 0, None, None)
            }
            .map_err(|error| {
                failure(
                    error.code().0 as u32 & 0xffff,
                    "restore the sleep privilege",
                )
            })?;
            let error = unsafe { GetLastError() };
            if error != ERROR_SUCCESS {
                return Err(failure(error.0, "restore the sleep privilege"));
            }
            self.restore_needed = false;
        }
        Ok(())
    }
}
impl Drop for SleepPrivilege {
    fn drop(&mut self) {
        let _ = self.restore();
        unsafe {
            let _ = CloseHandle(self.token);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supports_traditional_sleep_and_modern_standby_without_suspending() {
        let mut caps = SYSTEM_POWER_CAPABILITIES::default();
        assert!(!sleep_supported(&caps));
        caps.SystemS3 = true;
        assert!(sleep_supported(&caps));
        caps.SystemS3 = false;
        caps.AoAc = true;
        assert!(sleep_supported(&caps));
    }
    #[test]
    fn restrictions_and_unsupported_handlers_have_actionable_errors() {
        assert_eq!(failure(1314, "sleep").code, ErrorCode::Restricted);
        assert!(
            failure(5, "sleep")
                .message
                .contains("not request administrator")
        );
        assert_eq!(failure(50, "sleep").code, ErrorCode::Unsupported);
        assert_eq!(failure(1155, "open Settings").code, ErrorCode::Unsupported);
        assert_eq!(failure(123, "lock").code, ErrorCode::NativeFailure);
    }
}
