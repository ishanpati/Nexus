use super::{
    InstalledApp, LaunchTarget,
    foreground::{self, AppIdentity, Candidate, Desktop, Intent, WindowIdentity},
    launching::LaunchOutcome,
};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, LPARAM, PROPERTYKEY},
        Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute},
        Storage::Packaging::Appx::GetApplicationUserModelId,
        System::{
            Com::{
                CoTaskMemFree,
                StructuredStorage::{PropVariantClear, PropVariantToStringAlloc},
            },
            Threading::*,
        },
        UI::{
            Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
            Shell::PropertiesSystem::*,
            WindowsAndMessaging::*,
        },
    },
    core::{BOOL, PWSTR, w},
};

pub(super) struct Process(pub HANDLE);
impl Drop for Process {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}
#[derive(Default)]
pub(super) struct Receipt {
    pub process: Option<Process>,
    pub pid: u32,
}
impl Receipt {
    pub fn activated(pid: u32) -> Self {
        Self {
            process: unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
                .ok()
                .map(Process),
            pid,
        }
    }
    pub fn shell(handle: HANDLE) -> Self {
        let process = (!handle.is_invalid()).then_some(Process(handle));
        let pid = process.as_ref().map_or(0, |p| unsafe { GetProcessId(p.0) });
        Self { process, pid }
    }
}
fn input() -> Option<u32> {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetLastInputInfo(&mut info) }
        .as_bool()
        .then_some(info.dwTime)
}
pub(super) fn intent() -> impl Fn() -> bool + Send + Sync {
    let captured = Intent {
        input: input(),
        interaction: foreground::interaction(),
    };
    move || captured.permits(input(), foreground::interaction())
}
fn foreground() -> usize {
    unsafe { GetForegroundWindow() }.0 as usize
}
fn uncloaked(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            std::mem::size_of::<u32>() as u32,
        )
    }
    .is_ok()
        && cloaked == 0
}
unsafe extern "system" fn collect(hwnd: HWND, data: LPARAM) -> BOOL {
    let list = unsafe { &mut *(data.0 as *mut Vec<usize>) };
    if unsafe { IsWindowVisible(hwnd) }.as_bool()
        && uncloaked(hwnd)
        && unsafe { GetWindow(hwnd, GW_OWNER) }.is_err()
        && unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } & WS_EX_TOOLWINDOW.0 as isize == 0
    {
        list.push(hwnd.0 as usize);
    }
    BOOL(1)
}
fn windows() -> Vec<usize> {
    let mut list = Vec::<usize>::new();
    unsafe {
        let _ = EnumWindows(
            Some(collect),
            LPARAM((&mut list as *mut Vec<usize>) as isize),
        );
    }
    list
}
fn window_id(hwnd: HWND) -> Option<String> {
    let store: IPropertyStore = unsafe { SHGetPropertyStoreForWindow(hwnd) }.ok()?;
    let mut key = PROPERTYKEY::default();
    unsafe { PSGetPropertyKeyFromName(w!("System.AppUserModel.ID"), &mut key) }.ok()?;
    let mut value = unsafe { store.GetValue(&key) }.ok()?;
    let result = unsafe { PropVariantToStringAlloc(&value) }
        .ok()
        .and_then(|s| {
            let text = unsafe { s.to_string() }.ok();
            unsafe { CoTaskMemFree(Some(s.0.cast())) };
            text
        });
    unsafe {
        let _ = PropVariantClear(&mut value);
    }
    result.filter(|s| !s.is_empty())
}
fn process_id(process: HANDLE) -> Option<String> {
    let mut length = 0;
    unsafe {
        let _ = GetApplicationUserModelId(process, &mut length, None);
    }
    if length == 0 || length > 32768 {
        return None;
    }
    let mut buffer = vec![0; length as usize];
    unsafe { GetApplicationUserModelId(process, &mut length, Some(PWSTR(buffer.as_mut_ptr()))) }
        .is_ok()
        .then(|| String::from_utf16_lossy(&buffer[..length.saturating_sub(1) as usize]))
}
fn image(process: HANDLE) -> Option<String> {
    let mut buffer = vec![0; 32768];
    let mut length = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    }
    .ok()?;
    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}
struct Session<'a> {
    identity: AppIdentity,
    receipt: Receipt,
    before: HashSet<usize>,
    input: Option<u32>,
    ticket: u64,
    baseline: usize,
    started: Instant,
    stale: bool,
    cancelled: &'a (dyn Fn() -> bool + Sync),
}
impl Session<'_> {
    fn matches(&self, window: usize) -> bool {
        let hwnd = HWND(window as *mut _);
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if pid == 0 || pid == std::process::id() {
            return false;
        }
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
            .ok()
            .map(Process);
        let window_id = self.identity.id.as_ref().and_then(|_| window_id(hwnd));
        let process_id = self
            .identity
            .id
            .as_ref()
            .and_then(|_| process.as_ref().and_then(|p| process_id(p.0)));
        let path = self
            .identity
            .path
            .as_ref()
            .and_then(|_| process.as_ref().and_then(|p| image(p.0)));
        let launch_process_active = self.receipt.process.as_ref().is_some_and(|p| {
            let mut code = 0;
            unsafe { GetExitCodeProcess(p.0, &mut code) }.is_ok() && code == 259 // STILL_ACTIVE
        });
        let launched_process = self.receipt.pid == pid && launch_process_active;
        self.identity.matches(WindowIdentity {
            window_id: window_id.as_deref(),
            process_id: process_id.as_deref(),
            path: path.as_deref(),
            launched_process,
            launch_process_active,
            fresh: !self.before.contains(&window),
            foreground: foreground() == window,
        })
    }
}
impl Desktop for Session<'_> {
    fn expired(&self) -> bool {
        self.started.elapsed() >= Duration::from_secs(6)
    }
    fn permitted(&self) -> bool {
        !self.stale
            && foreground::current(self.ticket)
            && !(self.cancelled)()
            && self.input.is_some()
            && input() == self.input
            && (foreground() == self.baseline || self.matches(foreground()))
    }
    fn windows(&mut self) -> Vec<Candidate> {
        let active = foreground();
        windows()
            .into_iter()
            .filter(|w| self.matches(*w))
            .map(|w| Candidate {
                window: w,
                fresh: !self.before.contains(&w),
                foreground: w == active,
            })
            .collect()
    }
    fn foreground(&self, window: usize) -> bool {
        foreground() == window && self.matches(window)
    }
    fn activate(&mut self, window: usize) {
        // Revalidate identity and user intent at the native mutation boundary.
        if self.expired() || !self.permitted() || !self.matches(window) {
            return;
        }
        let hwnd = HWND(window as *mut _);
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindowAsync(hwnd, SW_RESTORE);
            }
            if !self.expired() && self.permitted() {
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }
    fn flash(&self, window: usize) {
        if !self.permitted() || !self.matches(window) {
            return;
        }
        unsafe {
            let _ = FlashWindowEx(&FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd: HWND(window as *mut _),
                dwFlags: FLASHW_TRAY,
                uCount: 3,
                dwTimeout: 0,
            });
        }
    }
    fn wait(&mut self) {
        std::thread::sleep(Duration::from_millis(50));
    }
}
pub(super) fn launch(
    app: &InstalledApp,
    hide: &(dyn Fn() -> Result<(), String> + Sync),
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<LaunchOutcome, String> {
    let (id, path) = match &app.launch {
        LaunchTarget::Shortcut {
            app_user_model_id,
            target,
            ..
        } => (app_user_model_id.clone(), Some(target.clone())),
        LaunchTarget::PackagedApp {
            app_user_model_id, ..
        } => (Some(app_user_model_id.clone()), None),
        LaunchTarget::ShellApp {
            app_user_model_id, ..
        } => (app_user_model_id.clone(), None),
    };
    let mut session = Session {
        identity: AppIdentity {
            id,
            path,
            has_arguments: matches!(&app.launch, LaunchTarget::Shortcut {arguments,..} if !arguments.trim().is_empty()),
        },
        receipt: Receipt::default(),
        before: windows().into_iter().collect(),
        input: input(),
        ticket: foreground::ticket(),
        baseline: foreground(),
        started: Instant::now(),
        stale: false,
        cancelled,
    };
    session.receipt = super::native::launch(app)?;
    // Activation and launcher ownership are separate. The caller may hide the
    // old launcher after Shell moved focus, but must preserve a newer interaction.
    session.stale = !session.permitted();
    let warning = hide().err();
    session.baseline = foreground(); // Windows can restore the previous foreground on hide.
    let (window_detected, activation) = foreground::discover(&mut session, 100);
    Ok(LaunchOutcome::accepted(
        window_detected,
        activation,
        warning,
    ))
}
