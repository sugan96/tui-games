use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEventKind};

/// Longest gap between OS key repeats that still counts as held. macOS repeats
/// every 15 to 120ms at the default settings. Raise it if a slow repeat rate never holds.
pub const REPEAT_WINDOW: Duration = Duration::from_millis(150);

/// Which key is held down, one key at a time. The last press wins.
/// With release events a key is held from Press to Release. Without them the
/// OS key repeat is the only signal: a Press of the same key within
/// REPEAT_WINDOW of the previous one means held, and the hold ends
/// REPEAT_WINDOW after the last repeat.
pub struct Input {
    /// The terminal sends key release events, so they end a hold instead of a timeout.
    release_events: bool,
    key: Option<KeyCode>,
    /// None while waiting for a release event.
    until: Option<Instant>,
    last_press: Option<(KeyCode, Instant)>,
    /// Time `held` checks against. The shell sets it before each game tick.
    now: Instant,
}

impl Input {
    pub fn new(release_events: bool) -> Input {
        Input {
            release_events,
            key: None,
            until: None,
            last_press: None,
            now: Instant::now(),
        }
    }

    /// Forget any hold. Keeps the release events setting.
    pub fn clear(&mut self) {
        *self = Input::new(self.release_events);
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

    fn press(&mut self, key: KeyCode, now: Instant) {
        if self.release_events {
            self.key = Some(key);
            self.until = None;
            return;
        }
        let repeat = self
            .last_press
            .is_some_and(|(k, t)| k == key && now - t <= REPEAT_WINDOW);
        self.key = repeat.then_some(key);
        self.until = Some(now + REPEAT_WINDOW);
        self.last_press = Some((key, now));
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

    #[test]
    fn key_repeat_holds_until_repeats_stop() {
        let ms = |n| Duration::from_millis(n);
        let t = Instant::now();
        let mut i = Input::new(false);
        i.event(R, PRESS, t);
        assert!(!held_at(&mut i, R, t), "a single tap is not a hold");
        i.event(R, PRESS, t + ms(400)); // first repeat after the OS delay
        assert!(!held_at(&mut i, R, t + ms(400)));
        i.event(R, PRESS, t + ms(450));
        assert!(held_at(&mut i, R, t + ms(500)));
        assert!(!held_at(&mut i, R, t + ms(600)), "repeats stopped");
        i.event(R, PRESS, t + ms(580));
        assert!(held_at(&mut i, R, t + ms(590)));
        i.event(KeyCode::Up, PRESS, t + ms(640));
        assert!(
            !held_at(&mut i, R, t + ms(650)),
            "another key ends the hold"
        );
    }

    #[test]
    fn release_events_hold_from_press_to_release() {
        let t = Instant::now();
        let mut i = Input::new(true);
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
        let mut i = Input::new(true);
        i.event(R, PRESS, t);
        i.clear();
        assert!(!i.held(R));
        i.event(R, PRESS, t);
        assert!(held_at(&mut i, R, t + Duration::from_secs(5)));
    }
}
