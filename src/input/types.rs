//! Types for input handling

/// Button identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Exit,
    Menu,
    Up,
    Down,
    Confirm,
    Reset,
}

/// Button state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Pressed,
    Released,
}

/// Button events
#[derive(Debug, Clone, Copy)]
pub enum ButtonEvent {
    Pressed(Button),
    Released(Button),
    LongPress(Button),
}

/// Dial rotation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialDirection {
    Clockwise,
    CounterClockwise,
}

/// Dial events
#[derive(Debug, Clone, Copy)]
pub enum DialEvent {
    Rotated(DialDirection),
    Pressed,
    Released,
}

/// Unified input event type
#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    Button(ButtonEvent),
    Dial(DialEvent),
}

impl From<ButtonEvent> for InputEvent {
    fn from(event: ButtonEvent) -> Self {
        InputEvent::Button(event)
    }
}

impl From<DialEvent> for InputEvent {
    fn from(event: DialEvent) -> Self {
        InputEvent::Dial(event)
    }
}

impl std::fmt::Display for Button {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Button::Exit => write!(f, "Exit"),
            Button::Menu => write!(f, "Menu"),
            Button::Up => write!(f, "Up"),
            Button::Down => write!(f, "Down"),
            Button::Confirm => write!(f, "Confirm"),
            Button::Reset => write!(f, "Reset"),
        }
    }
}

impl std::fmt::Display for ButtonEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ButtonEvent::Pressed(btn) => write!(f, "{} pressed", btn),
            ButtonEvent::Released(btn) => write!(f, "{} released", btn),
            ButtonEvent::LongPress(btn) => write!(f, "{} long press", btn),
        }
    }
}

impl std::fmt::Display for DialEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DialEvent::Rotated(dir) => match dir {
                DialDirection::Clockwise => write!(f, "Dial rotated clockwise"),
                DialDirection::CounterClockwise => write!(f, "Dial rotated counter-clockwise"),
            },
            DialEvent::Pressed => write!(f, "Dial pressed"),
            DialEvent::Released => write!(f, "Dial released"),
        }
    }
}

impl std::fmt::Display for InputEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputEvent::Button(evt) => write!(f, "{}", evt),
            InputEvent::Dial(evt) => write!(f, "{}", evt),
        }
    }
}
