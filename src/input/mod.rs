//! Input handling module for button and dial inputs
//!
//! This module provides a simple API for handling button presses and dial rotation
//! with proper debouncing and event queuing.

use anyhow::Result;
use esp_idf_svc::hal::gpio::{InputPin, OutputPin};
use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{self, Input, InterruptType, PinDriver, Pull},
};
use esp_idf_svc::hal::task::notification::Notification;
use log::{info, warn};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

// Re-export the public types
pub mod types;
pub use types::*;

// Button configuration
const DEBOUNCE_MS: u32 = 50; // Debounce time in milliseconds
const LONG_PRESS_MS: u32 = 1000; // Long press duration in milliseconds

// Trait objects for dynamic dispatch
trait ButtonHandlerTrait {
    fn take_event(&mut self) -> Option<ButtonEvent>;
}

trait DialTrait {
    fn check_events(&mut self) -> Option<DialEvent>;
}

/// Main input manager that handles all input devices
pub struct InputManager {
    buttons: [Box<dyn ButtonHandlerTrait>; 6],
    dial: Option<Box<dyn DialTrait>>,
    notification: Notification,
}

impl<T: InputPin + OutputPin + 'static> ButtonHandlerTrait for ButtonHandler<T> {
    fn take_event(&mut self) -> Option<ButtonEvent> {
        self.update().ok()?;
        self.event_queue.dequeue()
    }
}

impl<T: InputPin + OutputPin + 'static, U: InputPin + OutputPin + 'static, V: InputPin + OutputPin + 'static> DialTrait for Dial<T, U, V> {
    fn check_events(&mut self) -> Option<DialEvent> {
        match self.notification.wait(0) {
            Some(notification) => match notification.get() {
                1 => Some(DialEvent::Rotated(DialDirection::Clockwise)),
                2 => Some(DialEvent::Rotated(DialDirection::CounterClockwise)),
                3 => Some(DialEvent::Pressed),
                4 => Some(DialEvent::Released),
                _ => None,
            },
            None => None,
        }
    }
}

impl InputManager {
    /// Create a new InputManager with the specified pins
    pub fn new(
        exit_pin: impl InputPin + OutputPin + 'static,
        menu_pin: impl InputPin + OutputPin + 'static,
        up_pin: impl InputPin + OutputPin + 'static,
        down_pin: impl InputPin + OutputPin + 'static,
        conf_pin: impl InputPin + OutputPin + 'static,
        reset_pin: impl InputPin + OutputPin + 'static,
        // Optional dial pins (CLK, DT, SW)
        dial_pins: Option<(impl InputPin + OutputPin + 'static, impl InputPin + OutputPin + 'static, impl InputPin + OutputPin + 'static)>,
    ) -> Result<Self> {
        let notification = Notification::new();

        // Initialize buttons
        let buttons: [Box<dyn ButtonHandlerTrait>; 6] = [
            Box::new(ButtonHandler::new(Button::Exit, exit_pin, Notification::new())?),
            Box::new(ButtonHandler::new(Button::Menu, menu_pin, Notification::new())?),
            Box::new(ButtonHandler::new(Button::Up, up_pin, Notification::new())?),
            Box::new(ButtonHandler::new(Button::Down, down_pin, Notification::new())?),
            Box::new(ButtonHandler::new(Button::Confirm, conf_pin, Notification::new())?),
            Box::new(ButtonHandler::new(Button::Reset, reset_pin, Notification::new())?),
        ];

        // Initialize dial if pins are provided
        let dial = if let Some((clk_pin, dt_pin, sw_pin)) = dial_pins {
            Some(Box::new(Dial::new(clk_pin, dt_pin, sw_pin, Notification::new())?) as Box<dyn DialTrait>)
        } else {
            None
        };

        Ok(Self {
            buttons,
            dial,
            notification,
        })
    }

    /// Wait for and return the next input event
    pub fn wait_for_event(&mut self) -> Option<InputEvent> {
        // Wait for any input event
        self.notification.wait(esp_idf_svc::hal::delay::BLOCK);

        // Check for button events first
        for button in &mut self.buttons {
            if let Some(event) = button.take_event() {
                return Some(InputEvent::Button(event));
            }
        }

        // Then check for dial events
        if let Some(dial) = &mut self.dial {
            if let Some(event) = dial.check_events() {
                return Some(InputEvent::Dial(event));
            }
        }

        None
    }

    /// Check for input events without blocking
    pub fn check_events(&mut self) -> Option<InputEvent> {
        // Check for button events first
        for button in &mut self.buttons {
            if let Some(event) = button.take_event() {
                return Some(InputEvent::Button(event));
            }
        }

        // Then check for dial events
        if let Some(dial) = &mut self.dial {
            if let Some(event) = dial.check_events() {
                return Some(InputEvent::Dial(event));
            }
        }

        None
    }
}

/// Handler for individual buttons
struct ButtonHandler<T: InputPin + OutputPin> {
    button: Button,
    pin: PinDriver<'static, T, Input>,
    last_state: ButtonState,
    last_change: u32,
    event_queue: heapless::spsc::Queue<ButtonEvent, 8>,
    notification: Arc<Notification>,
}

impl<T: InputPin + OutputPin + 'static> ButtonHandler<T> {
    fn new(
        button: Button,
        pin: T,
        notification: Notification,
    ) -> Result<Self> {
        let mut pin = PinDriver::input(pin)?;
        pin.set_pull(Pull::Up)?;
        pin.set_interrupt_type(InterruptType::AnyEdge)?;
        pin.enable_interrupt()?;

        let notification = Arc::new(notification);

        Ok(Self {
            button,
            pin,
            last_state: ButtonState::Released,
            last_change: 0,
            event_queue: heapless::spsc::Queue::new(),
            notification,
        })
    }

    fn update(&mut self) -> Result<()> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u32;
        let current_state = if self.pin.is_low() {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        };

        // Check for state change
        if current_state != self.last_state {
            if current_time - self.last_change > DEBOUNCE_MS {
                self.last_change = current_time;
                self.last_state = current_state;

                // Queue the event
                let event = match current_state {
                    ButtonState::Pressed => ButtonEvent::Pressed(self.button),
                    ButtonState::Released => ButtonEvent::Released(self.button),
                    _ => return Ok(()),
                };

                if self.event_queue.enqueue(event).is_err() {
                    warn!("Button event queue full!");
                }

                // Notify the main thread
                unsafe {
                    self.notification.notifier().notify(NonZeroU32::new(0).unwrap());
                }
            }
        } else if current_state == ButtonState::Pressed
            && current_time - self.last_change > LONG_PRESS_MS
        {
            // Long press detection
            if self
                .event_queue
                .enqueue(ButtonEvent::LongPress(self.button))
                .is_ok()
            {
                unsafe {
                    self.notification.notifier().notify(NonZeroU32::new(0).unwrap());
                }
            }
        }

        Ok(())
    }
}

/// Handler for rotary encoder dial
struct Dial<T: InputPin + OutputPin, U: InputPin + OutputPin, V: InputPin + OutputPin> {
    clk_pin: PinDriver<'static, T, Input>,
    dt_pin: PinDriver<'static, U, Input>,
    sw_pin: PinDriver<'static, V, Input>,
    last_clk_state: bool,
    last_sw_state: bool,
    notification: Arc<Notification>,
}

impl<T: InputPin + OutputPin + 'static, U: InputPin + OutputPin + 'static, V: InputPin + OutputPin + 'static> Dial<T, U, V> {
    pub fn new(
        clk_pin: T,
        dt_pin: U,
        sw_pin: V,
        notification: Notification,
    ) -> Result<Self> {
        let mut clk = PinDriver::input(clk_pin)?;
        let mut dt = PinDriver::input(dt_pin)?;
        let mut sw = PinDriver::input(sw_pin)?;

        clk.set_pull(Pull::Up)?;
        dt.set_pull(Pull::Up)?;
        sw.set_pull(Pull::Up)?;

        let clk_state = clk.is_high();
        let sw_state = sw.is_low();

        let notification = Arc::new(notification);

        Ok(Self {
            clk_pin: clk,
            dt_pin: dt,
            sw_pin: sw,
            last_clk_state: clk_state,
            last_sw_state: sw_state,
            notification,
        })
    }
}
