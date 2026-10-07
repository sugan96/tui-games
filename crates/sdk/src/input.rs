use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEventKind};

/// Longest time a hold lasts after the last OS key repeat, and the window
/// until a gap between repeats has been measured. macOS repeats every 15 to
/// 120ms at the default settings.
pub const REPEAT_WINDOW: Duration = Duration::from_millis(150);
/// Shortest time a hold lasts after the last repeat.
pub const MIN_WINDOW: Duration = Duration::from_millis(30);
/// Longest gap between a press and its first OS repeat that still continues the
/// hold, with `hold_on_press`. Without it the limit is REPEAT_WINDOW.
pub const FIRST_REPEAT: Duration = Duration::from_millis(600);
/// How long a press holds its key with `hold_on_press` before any repeat arrives.
pub const PRESS_HOLD: Duration = Duration::from_millis(400);
/// The hold ends this many measured repeat gaps after the last repeat.
pub const REPEAT_FACTOR: f64 = 1.5;

/// Which key is held down, one key at a time. The last press wins.
/// With release events a key is held from Press to Release. Without them the
/// OS key repeat is the only signal: a Press of the same key within
/// FIRST_REPEAT of the press with hold_on_press, REPEAT_WINDOW without, then
/// within the window of the previous repeat, continues the hold, and the hold
/// ends one window after the last repeat.
/// The window is REPEAT_FACTOR times the larger of the last two gaps measured
/// between repeats, from MIN_WINDOW to REPEAT_WINDOW, so one late event read
/// cannot break a hold.
pub struct Input {
    /// The terminal sends key release events, so they end a hold instead of a timeout.
    release_events: bool,
    /// Without release events a press starts a hold for PRESS_HOLD.
    hold_on_press: bool,
    key: Option<KeyCode>,
    /// None while waiting for a release event.
    until: Option<Instant>,
    last_press: Option<(KeyCode, Instant)>,
    /// Follow-up presses of the current key.
    repeats: u32,
    /// Gap between the last two repeats.
    interval: Option<Duration>,
    /// The gap measured before `interval`.
    previous: Option<Duration>,
    /// Time `held` checks against. The shell sets it before each game tick.
    now: Instant,
}

impl Input {
    pub fn new(release_events: bool, hold_on_press: bool) -> Input {
        Input {
            release_events,
            hold_on_press,
            key: None,
            until: None,
            last_press: None,
            repeats: 0,
            interval: None,
            previous: None,
            now: Instant::now(),
        }
    }

    /// Forget any hold. Keeps the release events and hold on press settings.
    pub fn clear(&mut self) {
        *self = Input::new(self.release_events, self.hold_on_press);
    }

    /// Feed every key event, including repeats and releases.
    pub fn event(&mut self, key: KeyCode, kind: KeyEventKind, now: Instant) {
        self.now = now;
        match kind {
            KeyEventKind::Press => self.press(key, now),
            KeyEventKind::Release if self.key == Some(key) => self.key = None,
            _ => {}
        }
    }

    pub fn set_now(&mut self, now: Instant) {
        self.now = now;
    }

    /// True while `key` is held at the time of the last set_now or event.
    pub fn held(&self, key: KeyCode) -> bool {
        match self.until {
            Some(u) if self.now >= u => false,
            _ => self.key == Some(key),
        }
    }

    /// True while `key` is held on the press alone: no release events,
    /// `hold_on_press`, and no repeat has continued the hold yet.
    pub fn provisional(&self, key: KeyCode) -> bool {
        self.held(key) && !self.release_events && self.hold_on_press && self.repeats == 0
    }

    fn press(&mut self, key: KeyCode, now: Instant) {
        if self.release_events {
            self.key = Some(key);
            self.until = None;
            return;
        }
        let gap_limit = if self.repeats == 0 && self.hold_on_press {
            FIRST_REPEAT
        } else {
            self.window()
        };
        match self.last_press {
            Some((k, t)) if k == key && now - t <= gap_limit => {
                self.repeats += 1;
                if self.repeats >= 2 {
                    self.previous = self.interval.replace(now - t);
                }
                self.key = Some(key);
                self.until = Some(now + self.window());
            }
            _ => {
                self.repeats = 0;
                self.interval = None;
                self.previous = None;
                self.key = self.hold_on_press.then_some(key);
                self.until = Some(now + PRESS_HOLD);
            }
        }
        self.last_press = Some((key, now));
    }

    /// REPEAT_FACTOR times the larger of the last two measured gaps, from
    /// MIN_WINDOW to REPEAT_WINDOW, or REPEAT_WINDOW before any gap.
    fn window(&self) -> Duration {
        match self.interval.max(self.previous) {
            None => REPEAT_WINDOW,
            Some(i) => i.mul_f64(REPEAT_FACTOR).clamp(MIN_WINDOW, REPEAT_WINDOW),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: KeyCode = KeyCode::Right;
    const PRESS: KeyEventKind = KeyEventKind::Press;

    fn held_at(i: &mut Input, key: KeyCode, now: Instant) -> bool {
        i.set_now(now);
        i.held(key)
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn key_repeat_holds_until_repeats_stop() {
        let t = Instant::now();
        let mut i = Input::new(false, false);
        i.event(R, PRESS, t);
        i.event(R, PRESS, t + ms(400)); // first repeat after the OS delay
        i.event(R, PRESS, t + ms(450));
        assert!(held_at(&mut i, R, t + ms(450)));
        i.event(R, PRESS, t + ms(500)); // 50 ms gap, window 75 ms
        assert!(held_at(&mut i, R, t + ms(570)));
        assert!(!held_at(&mut i, R, t + ms(575)), "repeats stopped");
        i.event(R, PRESS, t + ms(650));
        assert!(
            !held_at(&mut i, R, t + ms(650)),
            "a press after the window is new"
        );
        i.event(R, PRESS, t + ms(750));
        assert!(held_at(&mut i, R, t + ms(760)));
        i.event(KeyCode::Up, PRESS, t + ms(770));
        assert!(
            !held_at(&mut i, R, t + ms(780)),
            "another key ends the hold"
        );
    }

    #[test]
    fn first_repeat_within_600_ms_continues_the_hold() {
        let t = Instant::now();
        let mut i = Input::new(false, true);
        i.event(R, PRESS, t);
        i.event(R, PRESS, t + ms(375));
        assert!(held_at(&mut i, R, t + ms(380)));
        assert!(
            !held_at(&mut i, R, t + ms(375 + 150)),
            "a continued hold ends one window after the repeat, a new press would hold 400 ms"
        );
    }

    #[test]
    fn window_follows_the_measured_interval() {
        let t = Instant::now();
        let mut i = Input::new(false, true);
        for n in [0, 375, 465] {
            i.event(R, PRESS, t + ms(n));
        }
        assert!(held_at(&mut i, R, t + ms(465 + 100)));
        assert!(
            !held_at(&mut i, R, t + ms(465 + 150)),
            "window is 135 ms, not 150"
        );
        let mut i = Input::new(false, true);
        for n in [0, 375, 465, 480, 495] {
            i.event(R, PRESS, t + ms(n));
        }
        assert!(held_at(&mut i, R, t + ms(495 + 29)));
        assert!(
            !held_at(&mut i, R, t + ms(495 + 30)),
            "two short gaps in a row shrink the window, to at least 30 ms"
        );
    }

    #[test]
    fn one_short_gap_does_not_shrink_the_window() {
        // Repeats 90 ms apart, one 30 ms gap, then 90 ms again: the window
        // stays 1.5 times 90 ms, so the last repeat continues the hold.
        let t = Instant::now();
        let mut i = Input::new(false, false);
        for n in [0, 375, 465, 555, 645, 675, 765] {
            i.event(R, PRESS, t + ms(n));
        }
        assert!(held_at(&mut i, R, t + ms(765 + 100)));
        assert!(!held_at(&mut i, R, t + ms(765 + 135)), "window is 135 ms");
    }

    #[test]
    fn hold_on_press_starts_at_the_press_and_ends_after_400_ms_without_repeats() {
        let t = Instant::now();
        let mut i = Input::new(false, true);
        i.event(R, PRESS, t);
        assert!(held_at(&mut i, R, t + ms(10)));
        assert!(held_at(&mut i, R, t + ms(390)));
        assert!(!held_at(&mut i, R, t + ms(410)));
    }

    #[test]
    fn press_hold_is_provisional_until_a_repeat() {
        let t = Instant::now();
        let mut i = Input::new(false, true);
        i.event(R, PRESS, t);
        assert!(held_at(&mut i, R, t + ms(10)) && i.provisional(R));
        i.event(R, PRESS, t + ms(375));
        assert!(held_at(&mut i, R, t + ms(375)) && !i.provisional(R));
        let mut i = Input::new(false, true);
        i.event(R, PRESS, t);
        assert!(!held_at(&mut i, R, t + ms(401)) && !i.provisional(R));
    }

    #[test]
    fn release_events_are_never_provisional() {
        let t = Instant::now();
        let mut i = Input::new(true, true);
        i.event(R, PRESS, t);
        assert!(held_at(&mut i, R, t + ms(10)) && !i.provisional(R));
    }

    #[test]
    fn hold_on_press_hands_over_to_repeats() {
        let t = Instant::now();
        let mut i = Input::new(false, true);
        for n in [0, 375, 465] {
            i.event(R, PRESS, t + ms(n));
        }
        assert!(held_at(&mut i, R, t + ms(500)));
        assert!(!held_at(&mut i, R, t + ms(465 + 150 + 1)));
    }

    #[test]
    fn without_hold_on_press_a_tap_is_not_a_hold() {
        let t = Instant::now();
        let mut i = Input::new(false, false);
        i.event(R, PRESS, t);
        assert!(!held_at(&mut i, R, t), "a single tap is not a hold");
        i.event(R, PRESS, t + ms(375));
        assert!(
            !held_at(&mut i, R, t + ms(375)),
            "nor a second tap or first repeat 375 ms later, so Snake does not boost"
        );
    }

    #[test]
    fn release_events_hold_from_press_to_release() {
        let t = Instant::now();
        let mut i = Input::new(true, false);
        i.event(R, PRESS, t);
        assert!(held_at(&mut i, R, t + Duration::from_secs(5)));
        i.event(KeyCode::Up, KeyEventKind::Release, t);
        assert!(i.held(R), "releasing another key keeps the hold");
        i.event(R, KeyEventKind::Release, t);
        assert!(!i.held(R));
    }

    #[test]
    fn clear_keeps_release_events() {
        let t = Instant::now();
        let mut i = Input::new(true, false);
        i.event(R, PRESS, t);
        i.clear();
        assert!(!i.held(R));
        i.event(R, PRESS, t);
        assert!(held_at(&mut i, R, t + Duration::from_secs(5)));
    }
}
