//! Debouncing for active-low CrowPanel buttons.

/// A debounced button transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    Pressed,
    Released,
}

/// Button debouncer using a monotonic millisecond clock.
#[derive(Clone, Copy, Debug)]
pub struct Debouncer {
    stable: bool,
    candidate: bool,
    candidate_since: u64,
    last_sample: u64,
    debounce_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockWentBackwards;

impl Debouncer {
    pub fn new(debounce_ms: u64, now: u64) -> Self {
        Self {
            stable: false,
            candidate: false,
            candidate_since: now,
            last_sample: now,
            debounce_ms,
        }
    }

    /// Sample the electrical pin level; `high == false` means pressed.
    pub fn sample(&mut self, high: bool, now: u64) -> Result<Option<Event>, ClockWentBackwards> {
        if now < self.last_sample {
            return Err(ClockWentBackwards);
        }
        self.last_sample = now;
        let pressed = !high;
        if pressed != self.candidate {
            self.candidate = pressed;
            self.candidate_since = now;
        }
        if self.candidate != self.stable && now - self.candidate_since >= self.debounce_ms {
            self.stable = self.candidate;
            return Ok(Some(if self.stable {
                Event::Pressed
            } else {
                Event::Released
            }));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounce_emits_one_press_and_one_release() {
        let mut button = Debouncer::new(50, 0);
        assert_eq!(button.sample(false, 10), Ok(None));
        assert_eq!(button.sample(true, 20), Ok(None));
        assert_eq!(button.sample(false, 30), Ok(None));
        assert_eq!(button.sample(false, 79), Ok(None));
        assert_eq!(button.sample(false, 80), Ok(Some(Event::Pressed)));
        assert_eq!(button.sample(false, 200), Ok(None));
        assert_eq!(button.sample(true, 201), Ok(None));
        assert_eq!(button.sample(true, 251), Ok(Some(Event::Released)));
        assert_eq!(button.sample(true, 252), Ok(None));
    }

    #[test]
    fn rejects_backward_clock_without_changing_candidate() {
        let mut button = Debouncer::new(50, 10);
        assert_eq!(button.sample(false, 20), Ok(None));
        assert_eq!(button.sample(true, 19), Err(ClockWentBackwards));
        assert_eq!(button.sample(false, 70), Ok(Some(Event::Pressed)));
    }
}
