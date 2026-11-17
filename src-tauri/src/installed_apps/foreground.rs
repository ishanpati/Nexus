//! Platform-independent activation policy. No detached activation jobs survive a step.
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};

static GENERATION: AtomicU64 = AtomicU64::new(0);
static INTERACTION: AtomicU64 = AtomicU64::new(0);
pub(crate) fn invalidate() {
    INTERACTION.fetch_add(1, Ordering::SeqCst);
    GENERATION.fetch_add(1, Ordering::SeqCst);
}
pub(super) fn interaction() -> u64 {
    INTERACTION.load(Ordering::SeqCst)
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
        if !desktop.permitted() {
            return (detected, Activation::Suppressed);
        }
        let windows = desktop.windows();
        detected |= !windows.is_empty();
        if let Some(window) = select(&windows) {
            if !desktop.permitted() {
                return (true, Activation::Suppressed);
            }
            if desktop.foreground(window) {
                return (true, Activation::Foreground);
            }
            desktop.activate(window);
            // ShowWindowAsync and cross-thread activation may complete later.
            for attempt in 0..=4 {
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
        focused: bool,
        succeeds: bool,
        activations: usize,
        flashes: std::cell::Cell<usize>,
    }
    impl Desktop for Fake {
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
}
