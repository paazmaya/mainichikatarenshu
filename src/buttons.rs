//! Debouncing for active-low CrowPanel buttons.

/// Button identifiers corresponding to CrowPanel hardware inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonId {
    /// Confirm button / rotary push switch (GPIO5)
    Confirm,
    /// Menu button (GPIO2)
    Menu,
    /// Exit button (GPIO1)
    Exit,
    /// Rotary switch Down (GPIO4)
    Down,
    /// Rotary switch Up (GPIO6)
    Up,
}

/// A debounced button transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    /// The button went down.
    Pressed,
    /// The button was let go.
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

/// A sample was older than the previous one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockWentBackwards;

impl Debouncer {
    /// A released button that reports changes stable for `debounce_ms`.
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

/// A bank of buttons debounced with a common monotonic millisecond clock.
#[derive(Clone, Copy, Debug)]
pub struct ButtonBank {
    confirm: Debouncer,
    menu: Debouncer,
    exit: Debouncer,
    down: Debouncer,
    up: Debouncer,
}

impl ButtonBank {
    /// Released buttons sharing one debounce interval.
    pub fn new(debounce_ms: u64, now: u64) -> Self {
        Self {
            confirm: Debouncer::new(debounce_ms, now),
            menu: Debouncer::new(debounce_ms, now),
            exit: Debouncer::new(debounce_ms, now),
            down: Debouncer::new(debounce_ms, now),
            up: Debouncer::new(debounce_ms, now),
        }
    }

    /// Sample a button by ID; `high == false` means pressed.
    pub fn sample(
        &mut self,
        id: ButtonId,
        high: bool,
        now: u64,
    ) -> Result<Option<Event>, ClockWentBackwards> {
        match id {
            ButtonId::Confirm => self.confirm.sample(high, now),
            ButtonId::Menu => self.menu.sample(high, now),
            ButtonId::Exit => self.exit.sample(high, now),
            ButtonId::Down => self.down.sample(high, now),
            ButtonId::Up => self.up.sample(high, now),
        }
    }

    /// Sample several buttons at the same instant and return those just pressed.
    ///
    /// `levels` pairs each button with its electrical level (`false` = pressed).
    pub fn pressed<const N: usize>(
        &mut self,
        levels: [(ButtonId, bool); N],
        now: u64,
    ) -> Result<heapless::Vec<ButtonId, N>, ClockWentBackwards> {
        let mut pressed = heapless::Vec::new();
        for (id, high) in levels {
            if self.sample(id, high, now)? == Some(Event::Pressed) {
                // Capacity equals the number of samples, so this cannot fail.
                let _ = pressed.push(id);
            }
        }
        Ok(pressed)
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

    #[test]
    fn button_bank_samples_independently() {
        let mut bank = ButtonBank::new(50, 0);
        assert_eq!(bank.sample(ButtonId::Confirm, false, 20), Ok(None));
        assert_eq!(
            bank.sample(ButtonId::Confirm, false, 80),
            Ok(Some(Event::Pressed))
        );
        assert_eq!(bank.sample(ButtonId::Menu, true, 80), Ok(None));
        assert_eq!(bank.sample(ButtonId::Menu, false, 100), Ok(None));
        assert_eq!(
            bank.sample(ButtonId::Menu, false, 160),
            Ok(Some(Event::Pressed))
        );
    }

    #[test]
    fn pressed_reports_only_new_presses() {
        let mut bank = ButtonBank::new(50, 0);
        let levels = |confirm, menu| [(ButtonId::Confirm, confirm), (ButtonId::Menu, menu)];
        assert!(bank.pressed(levels(false, true), 10).unwrap().is_empty());
        assert_eq!(
            bank.pressed(levels(false, false), 60).unwrap(),
            [ButtonId::Confirm]
        );
        assert_eq!(
            bank.pressed(levels(false, false), 110).unwrap(),
            [ButtonId::Menu]
        );
        assert!(bank.pressed(levels(false, false), 200).unwrap().is_empty());
        assert_eq!(
            bank.pressed(levels(true, true), 199),
            Err(ClockWentBackwards)
        );
    }
}
