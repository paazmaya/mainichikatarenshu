//! Input handling module for button and dial inputs
//!
//! This module provides a simple API for handling button presses and dial rotation
//! with proper debouncing and event queuing.

use anyhow::{Context, Result};
use esp_idf_svc::hal::gpio::{InputPin, OutputPin};
use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{Input, InterruptType, PinDriver, Pull},
};
use esp_idf_svc::hal::task::notification::Notification;
use log::warn;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::num::NonZeroU32;
// use core::time::{SystemTime, UNIX_EPOCH, Duration};

// Re-export the public types
pub mod types;
pub use types::*;

// Button configuration
const DEBOUNCE_MS: u32 = 50; // Debounce time in milliseconds
const LONG_PRESS_MS: u32 = 1000; // Long press duration in milliseconds

// Trait objects for dynamic dispatch
/// Trait for handling button input events
/// 
/// This trait defines the interface for button handlers that can detect
/// button presses, releases, and long presses with proper debouncing.
trait ButtonHandlerTrait {
    /// Take the next button event from the queue, if available
    /// 
    /// Returns `Some(ButtonEvent)` if an event is available, `None` otherwise.
    /// This method should be called periodically to check for new button events.
    fn take_event(&mut self) -> Option<ButtonEvent>;
}

/// Trait for handling rotary encoder dial input events
/// 
/// This trait defines the interface for dial handlers that can detect
/// rotation (clockwise/counter-clockwise) and button presses/releases.
trait DialTrait {
    /// Check for dial events since the last call
    /// 
    /// Returns `Some(DialEvent)` if an event occurred, `None` otherwise.
    /// This method should be called periodically to check for new dial events.
    fn check_events(&mut self) -> Option<DialEvent>;
}

/// Main input manager that handles all input devices
/// 
/// This struct manages button and dial input devices, providing a unified
/// interface for checking input events. It uses a shared notification system
/// to coordinate between different input handlers.
pub struct InputManager {
    /// Array of button handlers for the 6 main buttons
    buttons: [Box<dyn ButtonHandlerTrait>; 6],
    /// Optional dial handler for rotary encoder input
    dial: Option<Box<dyn DialTrait>>,
    /// Shared notification system for input event coordination
    notification: Arc<Notification>,
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
    /// Create a new InputManager with the specified buttons (no dial)
    /// 
    /// # Arguments
    /// * `exit_pin` - Pin for the Exit button
    /// * `menu_pin` - Pin for the Menu button  
    /// * `up_pin` - Pin for the Up button
    /// * `down_pin` - Pin for the Down button
    /// * `conf_pin` - Pin for the Confirm button
    /// * `reset_pin` - Pin for the Reset button
    /// 
    /// # Returns
    /// Returns `Ok(InputManager)` if initialization succeeds, `Err` otherwise.
    pub fn new(
        exit_pin: impl InputPin + OutputPin + 'static,
        menu_pin: impl InputPin + OutputPin + 'static,
        up_pin: impl InputPin + OutputPin + 'static,
        down_pin: impl InputPin + OutputPin + 'static,
        conf_pin: impl InputPin + OutputPin + 'static,
        reset_pin: impl InputPin + OutputPin + 'static,
    ) -> Result<Self> {
        Self::new_with_dial(
            exit_pin,
            menu_pin,
            up_pin,
            down_pin,
            conf_pin,
            reset_pin,
            None::<(esp_idf_svc::hal::gpio::Gpio0, esp_idf_svc::hal::gpio::Gpio0, esp_idf_svc::hal::gpio::Gpio0)>,
        )
    }

    /// Create a new InputManager with buttons and optional dial
    /// 
    /// # Arguments
    /// * `exit_pin` - Pin for the Exit button
    /// * `menu_pin` - Pin for the Menu button
    /// * `up_pin` - Pin for the Up button
    /// * `down_pin` - Pin for the Down button
    /// * `conf_pin` - Pin for the Confirm button
    /// * `reset_pin` - Pin for the Reset button
    /// * `dial_pins` - Optional tuple of (CLK, DT, SW) pins for rotary encoder
    /// 
    /// # Returns
    /// Returns `Ok(InputManager)` if initialization succeeds, `Err` otherwise.
    pub fn new_with_dial(
        exit_pin: impl InputPin + OutputPin + 'static,
        menu_pin: impl InputPin + OutputPin + 'static,
        up_pin: impl InputPin + OutputPin + 'static,
        down_pin: impl InputPin + OutputPin + 'static,
        conf_pin: impl InputPin + OutputPin + 'static,
        reset_pin: impl InputPin + OutputPin + 'static,
        // Optional dial pins (CLK, DT, SW)
        dial_pins: Option<(impl InputPin + OutputPin + 'static, impl InputPin + OutputPin + 'static, impl InputPin + OutputPin + 'static)>,
    ) -> Result<Self> {
        let notification = Arc::new(Notification::new());

        // Initialize buttons - use shared notification system
        let buttons: [Box<dyn ButtonHandlerTrait>; 6] = [
            Box::new(ButtonHandler::new(Button::Exit, exit_pin, notification.clone())
                .with_context(|| "Failed to initialize Exit button (GPIO pin 1)")?),
            Box::new(ButtonHandler::new(Button::Menu, menu_pin, notification.clone())
                .with_context(|| "Failed to initialize Menu button (GPIO pin 2)")?),
            Box::new(ButtonHandler::new(Button::Up, up_pin, notification.clone())
                .with_context(|| "Failed to initialize Up button (GPIO pin 6)")?),
            Box::new(ButtonHandler::new(Button::Down, down_pin, notification.clone())
                .with_context(|| "Failed to initialize Down button (GPIO pin 4)")?),
            Box::new(ButtonHandler::new(Button::Confirm, conf_pin, notification.clone())
                .with_context(|| "Failed to initialize Confirm button (GPIO pin 5)")?),
            Box::new(ButtonHandler::new(Button::Reset, reset_pin, notification.clone())
                .with_context(|| "Failed to initialize Reset button (GPIO pin 3)")?),
        ];

        // Initialize dial if pins are provided - use shared notification
        let dial = if let Some((clk_pin, dt_pin, sw_pin)) = dial_pins {
            Some(Box::new(Dial::new(clk_pin, dt_pin, sw_pin, notification.clone())
                .with_context(|| "Failed to initialize rotary encoder dial")?) as Box<dyn DialTrait>)
        } else {
            None
        };

        Ok(Self {
            buttons,
            dial,
            notification,
        })
    }

    /// Check for and return the next input event (non-blocking)
    /// 
    /// This method checks all input handlers for pending events and returns
    /// the first available event. Button events are checked before dial events.
    /// 
    /// # Returns
    /// Returns `Some(InputEvent)` if an event is available, `None` otherwise.
    /// 
    /// # Examples
    /// ```
    /// if let Some(event) = input_manager.check_events() {
    ///     match event {
    ///         InputEvent::Button(button_event) => handle_button(button_event),
    ///         InputEvent::Dial(dial_event) => handle_dial(dial_event),
    ///     }
    /// }
    /// ```
    pub fn check_events(&mut self) -> Option<InputEvent> {
        // Check for button events first
        for button in &mut self.buttons {
            if let Some(event) = button.take_event() {
                return Some(InputEvent::Button(event));
            }
        }

        // Check for dial events
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
        notification: Arc<Notification>,
    ) -> Result<Self> {
        let mut pin = PinDriver::input(pin)
            .map_err(|e| anyhow::anyhow!("Failed to initialize button pin for {:?}: {}", button, e))?;
        
        pin.set_pull(Pull::Up)
            .map_err(|e| anyhow::anyhow!("Failed to set pull-up for {:?}: {}", button, e))?;
        pin.set_interrupt_type(InterruptType::AnyEdge)
            .map_err(|e| anyhow::anyhow!("Failed to set interrupt type for {:?}: {}", button, e))?;
        pin.enable_interrupt()
            .map_err(|e| anyhow::anyhow!("Failed to enable interrupt for {:?}: {}", button, e))?;

        let notifier = notification.notifier();
        let notifier_clone = notifier.clone();

        // Set up interrupt handler in a separate thread
        // TODO: Fix thread spawn for no-std compatibility
        // std::thread::spawn(move || {
        //     loop {
        //         // Simple polling approach to avoid move issues
        //         FreeRtos::delay_ms(Duration::from_millis(10).as_millis() as u32);
        //         // Just notify that there might be a change - the update method will handle debouncing
        //         unsafe {
        //             if let Some(value) = NonZeroU32::new(0) {
        //                 notifier_clone.notify(value);
        //             }
        //         }
        //     }
        // });

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
        // TODO: Fix time tracking for no-std compatibility
        // let current_time = SystemTime::now()
        //     .duration_since(UNIX_EPOCH)
        //     .map_err(|e| anyhow::anyhow!("Failed to get system time: {}", e))?
        //     .as_millis() as u32;
        let current_time = 0; // Placeholder
        
        let current_state = self.pin.is_low();

        let current_state = if current_state {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        };

        // Check for state change
        if current_state != self.last_state {
            if current_time - self.last_change > DEBOUNCE_MS {
                self.last_change = current_time;
                self.last_state = current_state;

                let event = match current_state {
                    ButtonState::Pressed => ButtonEvent::Pressed(self.button),
                    ButtonState::Released => ButtonEvent::Released(self.button),
                };

                if self.event_queue.enqueue(event).is_err() {
                    log::warn!("Button event queue full for {:?}!", self.button);
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
            } else {
                log::warn!("Failed to enqueue long press event for {:?}", self.button);
            }
        }

        Ok(())
    }
}

/// Handler for rotary encoder dial
struct Dial<T: InputPin + OutputPin, U: InputPin + OutputPin, V: InputPin + OutputPin> {
    _phantom: core::marker::PhantomData<(T, U, V)>,
    last_clk_state: bool,
    last_sw_state: bool,
    notification: Arc<Notification>,
}

impl<T: InputPin + OutputPin + 'static, U: InputPin + OutputPin + 'static, V: InputPin + OutputPin + 'static> Dial<T, U, V> {
    pub fn new(
        clk_pin: T,
        dt_pin: U,
        sw_pin: V,
        notification: Arc<Notification>,
    ) -> Result<Self> {
        let mut clk = PinDriver::input(clk_pin)
            .map_err(|e| anyhow::anyhow!("Failed to initialize CLK pin: {}", e))?;
        let mut dt = PinDriver::input(dt_pin)
            .map_err(|e| anyhow::anyhow!("Failed to initialize DT pin: {}", e))?;
        let mut sw = PinDriver::input(sw_pin)
            .map_err(|e| anyhow::anyhow!("Failed to initialize SW pin: {}", e))?;

        clk.set_pull(Pull::Up)
            .map_err(|e| anyhow::anyhow!("Failed to set CLK pull-up: {}", e))?;
        dt.set_pull(Pull::Up)
            .map_err(|e| anyhow::anyhow!("Failed to set DT pull-up: {}", e))?;
        sw.set_pull(Pull::Up)
            .map_err(|e| anyhow::anyhow!("Failed to set SW pull-up: {}", e))?;

        let clk_state = clk.is_high();
        let sw_state = sw.is_low();

        let notifier = notification.notifier();
        let notifier_clone = notifier.clone();

        // Spawn a thread to monitor the dial
        // TODO: Fix thread spawn for no-std compatibility
        // std::thread::spawn(move || {
        //     let mut last_clk = clk_state;
        //     let mut last_sw = sw_state;
        //
        //     loop {
        //         let clk_state = clk.is_high();
        //         let dt_state = dt.is_high();
        //         let sw_state = sw.is_low();

                // Check for rotation
                // if clk_state != last_clk {
                //     if clk_state != dt_state {
                //         // Clockwise rotation
                //         unsafe {
                //             if let Some(value) = NonZeroU32::new(1) {
                //                 notifier_clone.notify(value);
                //             }
                //         }
                //     } else {
                //         // Counter-clockwise rotation
                //         unsafe {
                //             if let Some(value) = NonZeroU32::new(2) {
                //                 notifier_clone.notify(value);
                //             }
                //         }
                //     }
                //     last_clk = clk_state;
                // }

                // // Check for button press
                // if sw_state != last_sw {
                //     unsafe {
                //         if let Some(value) = NonZeroU32::new(if sw_state { 3 } else { 4 }) {
                //             notifier_clone.notify(value);
                //         }
                //     }
                //     last_sw = sw_state;
                // }

                // FreeRtos::delay_ms(1);
            // }

        Ok(Self {
            _phantom: core::marker::PhantomData,
            last_clk_state: clk_state,
            last_sw_state: sw_state,
            notification,
        })
    }
}
