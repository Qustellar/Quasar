use bevy_ecs::prelude::Resource;
use qst_core::glam::Vec2;
use std::collections::HashSet;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::KeyCode;

#[derive(Clone, Copy, Debug)]
enum PendingEvent {
    Key(KeyCode, ElementState),
    Mouse(MouseButton, ElementState),
    Cursor(Vec2),
    Wheel(Vec2),
    Entered,
    Left,
    Focused(bool),
}

#[derive(Clone, Debug)]
pub struct ButtonState<T> {
    pressed: HashSet<T>,
    just_pressed: HashSet<T>,
    just_released: HashSet<T>,
}

impl<T> Default for ButtonState<T> {
    fn default() -> Self {
        Self {
            pressed: HashSet::new(),
            just_pressed: HashSet::new(),
            just_released: HashSet::new(),
        }
    }
}

impl<T: Eq + std::hash::Hash + Copy> ButtonState<T> {
    pub fn pressed(&self, button: T) -> bool {
        self.pressed.contains(&button)
    }
    pub fn just_pressed(&self, button: T) -> bool {
        self.just_pressed.contains(&button)
    }
    pub fn just_released(&self, button: T) -> bool {
        self.just_released.contains(&button)
    }
    pub fn clear_transient(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
    }
    fn set(&mut self, button: T, state: ElementState) {
        match state {
            ElementState::Pressed if self.pressed.insert(button) => {
                self.just_pressed.insert(button);
            }
            ElementState::Released if self.pressed.remove(&button) => {
                self.just_released.insert(button);
            }
            _ => {}
        }
    }
    fn clear_pressed(&mut self) {
        for button in self.pressed.drain() {
            self.just_released.insert(button);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct KeyboardState {
    buttons: ButtonState<KeyCode>,
}
impl KeyboardState {
    pub fn pressed(&self, key: KeyCode) -> bool {
        self.buttons.pressed(key)
    }
    pub fn just_pressed(&self, key: KeyCode) -> bool {
        self.buttons.just_pressed(key)
    }
    pub fn just_released(&self, key: KeyCode) -> bool {
        self.buttons.just_released(key)
    }
}

#[derive(Clone, Debug, Default)]
pub struct MouseState {
    buttons: ButtonState<MouseButton>,
    pub position: Option<Vec2>,
    pub delta: Vec2,
    pub wheel_delta: Vec2,
    pub inside_window: bool,
}
impl MouseState {
    pub fn pressed(&self, button: MouseButton) -> bool {
        self.buttons.pressed(button)
    }
    pub fn just_pressed(&self, button: MouseButton) -> bool {
        self.buttons.just_pressed(button)
    }
    pub fn just_released(&self, button: MouseButton) -> bool {
        self.buttons.just_released(button)
    }
}

#[derive(Resource, Clone, Debug, Default)]
pub struct InputState {
    pub keyboard: KeyboardState,
    pub mouse: MouseState,
    pub focused: bool,
    pending_events: Vec<PendingEvent>,
}

/// Reserved extension point for future gamepad support.
#[derive(Clone, Debug, Default)]
pub struct GamepadInput;

impl InputState {
    /// Commits window events collected since the previous frame. Game systems
    /// observe the committed transient state during PreUpdate.
    pub fn begin_frame(&mut self) {
        self.keyboard.buttons.clear_transient();
        self.mouse.buttons.clear_transient();
        self.mouse.delta = Vec2::ZERO;
        self.mouse.wheel_delta = Vec2::ZERO;
        let events = std::mem::take(&mut self.pending_events);
        for event in events {
            match event {
                PendingEvent::Key(key, state) => self.keyboard.buttons.set(key, state),
                PendingEvent::Mouse(button, state) => self.mouse.buttons.set(button, state),
                PendingEvent::Cursor(next) => {
                    if let Some(previous) = self.mouse.position {
                        self.mouse.delta += next - previous;
                    }
                    self.mouse.position = Some(next);
                }
                PendingEvent::Wheel(delta) => self.mouse.wheel_delta += delta,
                PendingEvent::Entered => self.mouse.inside_window = true,
                PendingEvent::Left => self.mouse.inside_window = false,
                PendingEvent::Focused(focused) => {
                    self.focused = focused;
                    if !focused {
                        self.keyboard.buttons.clear_pressed();
                        self.mouse.buttons.clear_pressed();
                    }
                }
            }
        }
    }

    #[deprecated(note = "use begin_frame; transient state is committed from queued window events")]
    pub fn clear_transient(&mut self) {
        self.keyboard.buttons.clear_transient();
        self.mouse.buttons.clear_transient();
        self.mouse.delta = Vec2::ZERO;
        self.mouse.wheel_delta = Vec2::ZERO;
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let winit::keyboard::PhysicalKey::Code(code) = event.physical_key {
                    self.pending_events
                        .push(PendingEvent::Key(code, event.state));
                }
            }
            WindowEvent::MouseInput { state, button, .. } => self
                .pending_events
                .push(PendingEvent::Mouse(*button, *state)),
            WindowEvent::CursorMoved { position, .. } => {
                let next = Vec2::new(position.x as f32, position.y as f32);
                self.pending_events.push(PendingEvent::Cursor(next));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(x, y) => Vec2::new(*x, *y),
                    MouseScrollDelta::PixelDelta(value) => {
                        Vec2::new(value.x as f32, value.y as f32)
                    }
                };
                self.pending_events.push(PendingEvent::Wheel(delta));
            }
            WindowEvent::CursorEntered { .. } => self.pending_events.push(PendingEvent::Entered),
            WindowEvent::CursorLeft { .. } => self.pending_events.push(PendingEvent::Left),
            WindowEvent::Focused(focused) => {
                self.pending_events.push(PendingEvent::Focused(*focused))
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transient_state_is_one_frame() {
        let mut input = InputState::default();
        input.handle_window_event(&WindowEvent::Focused(true));
        input
            .pending_events
            .push(PendingEvent::Key(KeyCode::KeyA, ElementState::Pressed));
        input.begin_frame();
        assert!(input.keyboard.just_pressed(KeyCode::KeyA));
        input.begin_frame();
        assert!(!input.keyboard.just_pressed(KeyCode::KeyA));
        assert!(input.keyboard.pressed(KeyCode::KeyA));
    }
}
