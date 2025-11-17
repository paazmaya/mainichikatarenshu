//! Input handling module for button and dial inputs
//!
//! This module provides a simple API for handling button presses and dial rotation
//! with proper debouncing and event queuing.

use anyhow::Result;
use embedded_hal::digital::blocking::InputPin;
use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{self, Input, InterruptType, PinDriver, Pull},
    task::Notification,
};
use log::{info, warn};
use std::sync::Arc;

// Re-export the public types
pub mod types;
pub use types::*;

// Button configuration
const DEBOUNCE_MS: u32 = 50; // Debounce time in milliseconds
const LONG_PRESS_MS: u32 = 1000; // Long press duration in milliseconds

/// Main input manager that handles all input devices
pub struct InputManager {
    buttons: [ButtonHandler; 6],
    dial: Option<Dial>,
    notification: Notification,
}

impl InputManager {
    /// Create a new InputManager with the specified pins
    pub fn new(
        exit_pin: gpio::Gpio1<Input>,
        menu_pin: gpio::Gpio2<Input>,
        up_pin: gpio::Gpio6<Input>,
        down_pin: gpio::Gpio4<Input>,
        conf_pin: gpio::Gpio5<Input>,
        reset_pin: gpio::Gpio3<Input>,
        // Optional dial pins (CLK, DT, SW)
        dial_pins: Option<(gpio::Gpio7<Input>, gpio::Gpio8<Input>, gpio::Gpio9<Input>)>,
    ) -> Result<Self> {
        let notification = Notification::new();

        // Initialize buttons
        let buttons = [
            ButtonHandler::new(Button::Exit, exit_pin, notification.notifier())?,
            ButtonHandler::new(Button::Menu, menu_pin, notification.notifier())?,
            ButtonHandler::new(Button::Up, up_pin, notification.notifier())?,
            ButtonHandler::new(Button::Down, down_pin, notification.notifier())?,
            ButtonHandler::new(Button::Confirm, conf_pin, notification.notifier())?,
            ButtonHandler::new(Button::Reset, reset_pin, notification.notifier())?,
        ];

        // Initialize dial if pins are provided
        let dial = if let Some((clk_pin, dt_pin, sw_pin)) = dial_pins {
            Some(Dial::new(clk_pin, dt_pin, sw_pin, notification.notifier())?)
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
struct ButtonHandler {
    button: Button,
    pin: PinDriver<'static, Input>,
    last_state: ButtonState,
    last_change: u32,
    event_queue: heapless::spsc::Queue<ButtonEvent, 8>,
    notification: Arc<Notification>,
}

impl ButtonHandler {
    fn new(
        button: Button,
        pin: impl InputPin + 'static,
        notification: esp_idf_svc::hal::task::notify::Notification,
    ) -> Result<Self> {
        let mut pin = PinDriver::input(pin)?;
        pin.set_pull(Pull::Up)?;
        pin.set_interrupt_type(InterruptType::AnyEdge)?;
        pin.enable_interrupt()?;

        let notification = Arc::new(notification);
        let notification_clone = notification.clone();

        // Set up interrupt handler in a separate thread
        std::thread::spawn(move || {
            while let Ok(_) = pin.wait_for_any_edge() {
                // Notify the main thread
                notification_clone.notify(0);
            }
        });

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
        let current_time = esp_idf_svc::hal::task::current_tick() as u32;
        let current_state = if self.pin.is_low()? {
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
                self.notification.notify(0);
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
                self.notification.notify(0);
            }
        }

        Ok(())
    }

    fn take_event(&mut self) -> Option<ButtonEvent> {
        self.update().ok()?;
        self.event_queue.dequeue()
    }
}

/// Handler for rotary encoder dial
struct Dial {
    clk_pin: PinDriver<'static, Input>,
    dt_pin: PinDriver<'static, Input>,
    sw_pin: PinDriver<'static, Input>,
    last_clk_state: bool,
    last_sw_state: bool,
    notification: Arc<Notification>,
}

impl Dial {
    pub fn new(
        clk_pin: impl InputPin + 'static,
        dt_pin: impl InputPin + 'static,
        sw_pin: impl InputPin + 'static,
        notification: esp_idf_svc::hal::task::notify::Notification,
    ) -> Result<Self> {
        let mut clk = PinDriver::input(clk_pin)?;
        let mut dt = PinDriver::input(dt_pin)?;
        let mut sw = PinDriver::input(sw_pin)?;

        clk.set_pull(Pull::Up)?;
        dt.set_pull(Pull::Up)?;
        sw.set_pull(Pull::Up)?;

        let clk_state = clk.is_high()?;
        let sw_state = sw.is_low()?;

        let notification = Arc::new(notification);
        let notification_clone = notification.clone();

        // Spawn a thread to monitor the dial
        std::thread::spawn(move || {
            let mut last_clk = clk_state;
            let mut last_sw = sw_state;

            loop {
                let clk_state = clk.is_high().unwrap_or(last_clk);
                let dt_state = dt.is_high().unwrap_or(!last_clk);
                let sw_state = sw.is_low().unwrap_or(last_sw);

                // Check for rotation
                if clk_state != last_clk {
                    if clk_state != dt_state {
                        // Clockwise rotation
                        notification_clone.notify(1);
                    } else {
                        // Counter-clockwise rotation
                        notification_clone.notify(2);
                    }
                    last_clk = clk_state;
                }

                // Check for button press
                if sw_state != last_sw {
                    notification_clone.notify(if sw_state { 3 } else { 4 });
                    last_sw = sw_state;
                }

                FreeRtos::delay_ms(1);
            }
        });

        Ok(Self {
            clk_pin: clk,
            dt_pin: dt,
            sw_pin,
            last_clk_state: clk_state,
            last_sw_state: sw_state,
            notification,
        })
    }

    pub fn check_events(&mut self) -> Option<DialEvent> {
        match self.notification.try_wait() {
            Some(1) => Some(DialEvent::Rotated(DialDirection::Clockwise)),
            Some(2) => Some(DialEvent::Rotated(DialDirection::CounterClockwise)),
            Some(3) => Some(DialEvent::Pressed),
            Some(4) => Some(DialEvent::Released),
            _ => None,
        }
    }
}
