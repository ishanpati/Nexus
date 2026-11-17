//! Platform-independent activation policy. No detached activation jobs survive a step.
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};

static GENERATION: AtomicU64 = AtomicU64::new(0);
static INTERACTION: AtomicU64 = AtomicU64::new(0);
pub(crate) fn invalidate() {
    INTERACTION.fetch_add(1, Ordering::SeqCst);
    GENERATION.fetch_add(1, Ordering::SeqCst);
}
pub(crate) fn interaction() -> u64 {
    INTERACTION.load(Ordering::SeqCst)
}
pub(crate) fn owns_launcher(owner: u64, current: u64, focused: bool, intent_current: bool) -> bool {
    owner == current && (!focused || intent_current)
}
pub(super) fn ticket() -> u64 {
    GENERATION.fetch_add(1, Ordering::SeqCst) + 1
}
pub(super) fn current(ticket: u64) -> bool {
    GENERATION.load(Ordering::SeqCst) == ticket
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Activation {
    Foreground,
    Denied,
    TimedOut,
    Suppressed,
}

#[derive(Clone, Copy)]
pub(super) struct Intent {
    pub input: Option<u32>,
    pub interaction: u64,
}
impl Intent {
    pub fn permits(self, input: Option<u32>, interaction: u64) -> bool {
        self.input.is_some() && input == self.input && interaction == self.interaction
    }
}

pub(super) struct AppIdentity {
    pub id: Option<String>,
    pub path: Option<String>,
    pub has_arguments: bool,
}
pub(super) struct WindowIdentity<'a> {
    pub window_id: Option<&'a str>,
    pub process_id: Option<&'a str>,
    pub path: Option<&'a str>,
    pub launched_process: bool,
    pub launch_process_active: bool,
    pub fresh: bool,
    pub foreground: bool,
}
impl AppIdentity {
    pub fn matches(&self, window: WindowIdentity<'_>) -> bool {
        // A newly created instance may not have a window yet. Do not prematurely
        // focus an older instance while its returned launch process is alive.
        // Once a single-instance forwarder exits, exact-identity fallback is safe.
        if window.launch_process_active
            && !window.launched_process
            && !window.fresh
            && !window.foreground
        {
            return false;
        }
        // An application ID does not distinguish arbitrary shortcut arguments or
        // profiles. Never select an old background window for such a shortcut.
        if self.has_arguments && !window.fresh && !window.foreground {
            return false;
        }
        if let Some(expected) = &self.id
            && let Some(actual) = window.window_id.or(window.process_id)
        {
            return actual == expected;
        }
        if let Some(expected) = &self.path {
            if self.has_arguments && !window.launched_process && !window.foreground {
                return false;
            }
            return window
                .path
                .is_some_and(|actual| actual.eq_ignore_ascii_case(expected));
        }
        self.id.is_none() && window.launched_process
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Candidate {
    pub window: usize,
    pub fresh: bool,
    pub foreground: bool,
}

// Input is already filtered by native identity. Ambiguous existing windows must
// never be chosen by caption, enumeration order, or a shared executable basename.
pub(super) fn select(windows: &[Candidate]) -> Option<usize> {
    let unique = |items: Vec<&Candidate>| (items.len() == 1).then(|| items[0].window);
    unique(windows.iter().filter(|w| w.foreground).collect())
        .or_else(|| unique(windows.iter().filter(|w| w.fresh).collect()))
        .or_else(|| unique(windows.iter().collect()))
}

pub(super) trait Desktop {
    fn expired(&self) -> bool {
        false
    }
    fn permitted(&self) -> bool;
    fn windows(&mut self) -> Vec<Candidate>;
    fn foreground(&self, window: usize) -> bool;
    fn activate(&mut self, window: usize);
    fn flash(&self, window: usize);
    fn wait(&mut self);
}

pub(super) fn discover(desktop: &mut impl Desktop, polls: usize) -> (bool, Activation) {
    let mut detected = false;
    for _ in 0..polls {
        if desktop.expired() {
            return (detected, Activation::TimedOut);
        }
        if !desktop.permitted() {
            return (detected, Activation::Suppressed);
        }
        let windows = desktop.windows();
        detected |= !windows.is_empty();
        if let Some(window) = select(&windows) {
            if desktop.expired() {
                return (true, Activation::TimedOut);
            }
            if !desktop.permitted() {
                return (true, Activation::Suppressed);
            }
            if desktop.foreground(window) {
                return (true, Activation::Foreground);
            }
            desktop.activate(window);
            // ShowWindowAsync and cross-thread activation may complete later.
            for attempt in 0..=4 {
                if desktop.expired() {
                    return (true, Activation::TimedOut);
                }
                if !desktop.permitted() {
                    return (true, Activation::Suppressed);
                }
                if desktop.foreground(window) {
                    return (true, Activation::Foreground);
                }
                if attempt < 4 {
                    desktop.wait();
                }
            }
            if desktop.permitted() {
                desktop.flash(window);
            }
            return (true, Activation::Denied);
        }
        desktop.wait();
    }
    (detected, Activation::TimedOut)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(window: usize, fresh: bool, foreground: bool) -> Candidate {
        Candidate {
            window,
            fresh,
            foreground,
        }
    }
    #[test]
    fn selects_foreground_then_unique_new_then_unique_existing() {
        assert_eq!(select(&[c(1, false, false), c(2, false, true)]), Some(2));
        assert_eq!(select(&[c(1, false, false), c(2, true, false)]), Some(2));
        assert_eq!(select(&[c(1, false, false)]), Some(1));
        assert_eq!(select(&[c(1, false, false), c(2, false, false)]), None);
        assert_eq!(select(&[c(1, true, false), c(2, true, false)]), None);
        assert_eq!(select(&[]), None);
    }
    struct Fake {
        ticks: usize,
        appears: usize,
        stop: usize,
        deadline: usize,
        focused: bool,
        succeeds: bool,
        activations: usize,
        flashes: std::cell::Cell<usize>,
    }
    impl Desktop for Fake {
        fn expired(&self) -> bool {
            self.ticks >= self.deadline
        }
        fn permitted(&self) -> bool {
            self.ticks < self.stop
        }
        fn windows(&mut self) -> Vec<Candidate> {
            if self.ticks >= self.appears {
                vec![c(1, false, false)]
            } else {
                vec![]
            }
        }
        fn foreground(&self, _: usize) -> bool {
            self.focused
        }
        fn activate(&mut self, _: usize) {
            self.activations += 1;
            self.focused = self.succeeds;
        }
        fn flash(&self, _: usize) {
            self.flashes.set(self.flashes.get() + 1);
        }
        fn wait(&mut self) {
            self.ticks += 1;
        }
    }
    fn fake() -> Fake {
        Fake {
            ticks: 0,
            appears: 0,
            stop: usize::MAX,
            deadline: usize::MAX,
            focused: false,
            succeeds: true,
            activations: 0,
            flashes: std::cell::Cell::new(0),
        }
    }
    #[test]
    fn verifies_activation_and_flashes_only_on_denial() {
        let mut d = fake();
        assert_eq!(discover(&mut d, 10), (true, Activation::Foreground));
        let mut d = fake();
        d.succeeds = false;
        assert_eq!(discover(&mut d, 10), (true, Activation::Denied));
        assert_eq!(d.flashes.get(), 1);
    }
    #[test]
    fn bounds_slow_and_windowless_launches() {
        let mut d = fake();
        d.appears = 3;
        assert_eq!(discover(&mut d, 5), (true, Activation::Foreground));
        let mut d = fake();
        d.appears = 20;
        assert_eq!(discover(&mut d, 5), (false, Activation::TimedOut));
        assert_eq!(d.ticks, 5);
    }
    #[test]
    fn cancellation_or_stale_input_prevents_late_activation() {
        let mut d = fake();
        d.appears = 4;
        d.stop = 2;
        assert_eq!(discover(&mut d, 10), (false, Activation::Suppressed));
        assert_eq!(d.activations, 0);
        let mut d = fake();
        d.succeeds = false;
        d.stop = 1;
        assert_eq!(discover(&mut d, 10), (true, Activation::Suppressed));
        assert_eq!(d.flashes.get(), 0);
    }
    #[test]
    fn newer_interaction_invalidates_old_ticket() {
        let old = ticket();
        invalidate();
        assert!(!current(old));
    }
    #[test]
    fn admission_intent_is_not_renewed_by_worker_delay() {
        let intent = Intent {
            input: Some(10),
            interaction: 7,
        };
        assert!(intent.permits(Some(10), 7));
        assert!(!intent.permits(Some(11), 7));
        assert!(!intent.permits(Some(10), 8));
        assert!(!intent.permits(None, 7));
        assert!(
            !Intent {
                input: None,
                interaction: 7
            }
            .permits(None, 7)
        );
    }
    #[test]
    fn hide_old_launcher_after_shell_focus_or_user_switch_but_preserve_new_interaction() {
        assert!(owns_launcher(4, 4, false, true)); // Shell already activated app.
        assert!(owns_launcher(4, 4, false, false)); // User switched; no activation allowed.
        assert!(owns_launcher(4, 4, true, true)); // Normal launch handoff.
        assert!(!owns_launcher(4, 5, false, true)); // New summon/shortcut/Escape.
        assert!(!owns_launcher(4, 4, true, false)); // User typing a newer query.
    }
    #[test]
    fn deadline_is_timeout_before_discovery_and_during_verification() {
        let mut desktop = fake();
        desktop.deadline = 0;
        assert_eq!(discover(&mut desktop, 100), (false, Activation::TimedOut));
        assert_eq!(desktop.activations, 0);
        let mut desktop = fake();
        desktop.succeeds = false;
        desktop.deadline = 2;
        assert_eq!(discover(&mut desktop, 100), (true, Activation::TimedOut));
        assert_eq!(desktop.flashes.get(), 0);
        assert_eq!(desktop.ticks, 2);
    }
    fn identity() -> AppIdentity {
        AppIdentity {
            id: Some("App.One".into()),
            path: Some(r"C:\App\one.exe".into()),
            has_arguments: false,
        }
    }
    fn evidence() -> WindowIdentity<'static> {
        WindowIdentity {
            window_id: None,
            process_id: None,
            path: Some(r"C:\App\one.exe"),
            launched_process: false,
            launch_process_active: false,
            fresh: false,
            foreground: false,
        }
    }
    #[test]
    fn explicit_window_identity_overrides_process_and_path() {
        let mut window = evidence();
        window.window_id = Some("App.Other");
        window.process_id = Some("App.One");
        assert!(!identity().matches(window));
        let mut window = evidence();
        window.process_id = Some("App.Other");
        assert!(!identity().matches(window));
        let mut window = evidence();
        window.window_id = Some("App.One");
        window.path = None;
        assert!(identity().matches(window));
        let mut window = evidence();
        window.path = Some(r"C:\Other\one.exe");
        assert!(!identity().matches(window)); // Same basename is not identity.
    }
    #[test]
    fn argument_shortcut_rejects_shared_id_old_windows_and_unrelated_new_hosts() {
        let mut expected = identity();
        expected.has_arguments = true;
        let mut window = evidence();
        window.window_id = Some("App.One");
        assert!(!expected.matches(window));
        let mut window = evidence();
        window.fresh = true;
        assert!(!expected.matches(window)); // Another launch of the same executable.
        let mut window = evidence();
        window.fresh = true;
        window.launched_process = true;
        assert!(expected.matches(window));
        let mut window = evidence();
        window.foreground = true;
        assert!(expected.matches(window)); // Shell already selected this instance.
    }
    #[test]
    fn no_handle_allows_exact_path_but_never_guesses_an_unknown_shell_target() {
        assert!(identity().matches(evidence()));
        let shell = AppIdentity {
            id: None,
            path: None,
            has_arguments: false,
        };
        assert!(!shell.matches(evidence()));
        let mut window = evidence();
        window.launched_process = true;
        assert!(shell.matches(window));
        let packaged = AppIdentity {
            id: Some("Package!App".into()),
            path: None,
            has_arguments: false,
        };
        let mut window = evidence();
        window.launched_process = true;
        assert!(!packaged.matches(window)); // A PID alone cannot identify a shared host.
        let mut window = evidence();
        window.process_id = Some("Package!App");
        assert!(packaged.matches(window));
    }
    #[test]
    fn wait_for_new_instance_before_using_existing_single_instance_fallback() {
        let mut old = evidence();
        old.launch_process_active = true;
        assert!(!identity().matches(old));
        let mut new = evidence();
        new.fresh = true;
        new.launch_process_active = true;
        new.launched_process = true;
        assert!(identity().matches(new));
        let mut forwarded = evidence();
        forwarded.launch_process_active = false;
        assert!(identity().matches(forwarded));
        let mut shell_selected = evidence();
        shell_selected.launch_process_active = true;
        shell_selected.foreground = true;
        assert!(identity().matches(shell_selected));
    }
}
