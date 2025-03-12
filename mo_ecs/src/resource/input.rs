//! Input service, ray casting service and utils
use bevy_ecs::prelude::Resource;
use bevy_math::Vec2;
use foldhash::{HashMap, HashMapExt};
use std::path::{Path, PathBuf};
use winit::event::{ElementState, KeyEvent, Modifiers, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::PhysicalKey;

/// Action requirements trait
pub trait TIntoAction: Copy + Eq + std::hash::Hash + std::fmt::Debug {}
impl<T: Copy + Eq + Send + Sync + std::hash::Hash + std::fmt::Debug> TIntoAction for T {}

/// Input button abstraction
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)] // TODO: add support for serialization
pub enum EInputButton {
    /// Key by the code
    Key(PhysicalKey),
    /// Left mouse button
    MouseLeft,
    /// Right Mouse Button
    MouseRight,
    /// Middle mouse button
    MouseMiddle,
    /// Mouse button by the code
    MouseOther(u16),
}

/// State of a button
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum EInputState {
    /// Button was activated (pressed)
    Activated,
    /// Button is hold
    Hold,
    /// Button was deactivated (released)
    Deactivated,
}

/// Input event abstraction
pub enum EInputEvent {
    /// Copy to clipboard event
    Copy,
    /// Cut to clipboard event
    Cut,
    /// Custom key event
    Key(FKeyEvent),
    /// Text input event
    Text(String),
}

/// Key input event
#[derive(Debug)]
pub struct FKeyEvent {
    /// Key code of the event trigger
    pub key_code: PhysicalKey,
    /// Key state
    pub pressed: bool,
    /// Active modifiers
    pub modifiers: Modifiers,
}

/// Sample input Actions enumeration
#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
pub enum EInputAction {
    /// Move forward
    MoveForward,
    /// Move Backward
    MoveBackward,
    /// Move Left
    MoveLeft,
    /// Move Right
    MoveRight,
}

/// Game action to input mapping
pub trait TActionMapper<T: TIntoAction> {
    /// Checks if action is mapped and returns an appropriate button and modifiers
    fn action_mapped(&self, action: T) -> Option<(EInputButton, Modifiers)>;
}

/// Standard Mapper
pub struct FActionMapper<T: TIntoAction> {
    map: HashMap<T, (EInputButton, Modifiers)>,
}

impl<T> FActionMapper<T>
where
    T: TIntoAction,
{
    /// Constructs new [`Mapper`]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Constructs new [`Mapper`] from actions list
    pub fn with_actions(actions: &[(T, EInputButton, Modifiers)]) -> Self {
        let mut mapper = Self::new();
        mapper.set(actions);
        mapper
    }

    /// Add a new action to mapper. If action already exists, it will be overridden
    pub fn add_action(&mut self, action: T, button: EInputButton, modifiers: Modifiers) {
        self.map.insert(action, (button, modifiers));
    }

    /// Add multiple actions to mapper. Existing actions will be overridden
    pub fn add_actions(&mut self, actions: &[(T, EInputButton, Modifiers)]) {
        for (action, button, modifiers) in actions {
            self.map.insert(*action, (*button, *modifiers));
        }
    }

    /// Get button that is binded to that action
    pub fn get_button(&self, action: T) -> Option<(EInputButton, Modifiers)> {
        self.map.get(&action).map(|(b, m)| (*b, *m))
    }

    /// Remove action from mapper
    pub fn remove_action(&mut self, action: T) {
        self.map.remove(&action);
    }

    /// Remove multiple actions from mapper
    pub fn remove_actions(&mut self, actions: Vec<T>) {
        for action in actions.iter() {
            self.map.remove(action);
        }
    }

    /// Removes all actions and set new ones
    pub fn set(&mut self, actions: &[(T, EInputButton, Modifiers)]) {
        self.map.clear();
        self.add_actions(actions);
    }
}

impl<T: TIntoAction> Default for FActionMapper<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Input Service
///
/// Collects input events, tracks state changes and provides mapping to game actions
#[derive(Resource)]
pub struct Input {
    mapper: Box<dyn std::any::Any + Send + Sync>,
    states: HashMap<EInputButton, (EInputState, Modifiers)>,
    mouse_scroll_delta: f32,
    mouse_horizontal_scroll_delta: f32,
    mouse_position: Option<Vec2>,
    mouse_delta: Vec2,
    mouse_moved: bool,
    window_size: Vec2, // TODO: move to other struct or service
    /// events collector
    pub events: Vec<EInputEvent>,
    /// modifiers collector
    pub modifiers: Modifiers,
    /// Currently dropped files. Used for drag & drop.
    pub dropped_files: Option<Vec<PathBuf>>,
    /// Currently held files above the window.
    pub hovered_files: Vec<PathBuf>,
}

impl Input {
    /// Returns the status of the mapped action.
    pub fn action_state<T>(&self, action: T) -> Option<EInputState>
    where
        Self: TActionMapper<T>,
        T: TIntoAction,
    {
        if let Some((button, modifiers)) = self.action_mapped(action) {
            if let Some((state, state_modifiers)) = self.states.get(&button) {
                if state_modifiers.state().contains(modifiers.state()) {
                    return Some(*state);
                }
            }
        }
        None
    }

    /// Returns the status of the raw input
    pub fn button_state(&self, button: EInputButton) -> Option<EInputState> {
        self.states.get(&button).map(|(state, _modifiers)| *state)
    }

    /// Checks if mapped action button is pressed
    pub fn is_action_activated<T>(&self, action: T) -> bool
    where
        Self: TActionMapper<T>,
        T: TIntoAction,
    {
        self.action_state(action)
            .map(|state| state == EInputState::Activated)
            .unwrap_or(false)
    }

    /// Checks if mapped action button is released
    pub fn is_action_deactivated<T>(&self, action: T) -> bool
    where
        Self: TActionMapper<T>,
        T: TIntoAction,
    {
        self.action_state(action)
            .map(|state| state == EInputState::Deactivated)
            .unwrap_or(false)
    }

    /// Checks if mapped button is pressed or hold
    pub fn is_action_hold<T>(&self, action: T) -> bool
    where
        Self: TActionMapper<T>,
        T: TIntoAction,
    {
        self.action_state(action)
            .map(|state| state == EInputState::Hold || state == EInputState::Activated)
            .unwrap_or(false)
    }

    /// Set custom [`ActionMapper`]
    pub fn set_mapper(&mut self, mapper: Box<dyn std::any::Any + Send + Sync>) {
        self.mapper = mapper;
    }

    /// Get input mapper reference
    pub fn mapper<T: 'static + Send + Sync>(&self) -> &T {
        self.mapper.downcast_ref::<T>().unwrap()
    }

    /// Get mutual mapper reference
    pub fn mapper_mut<T: 'static + Send + Sync>(&mut self) -> &mut T {
        self.mapper.downcast_mut::<T>().unwrap()
    }

    /// Set window size
    pub fn set_window_size(&mut self, width: f32, height: f32) {
        self.window_size = Vec2::new(width, height);
    }

    /// Mouse scroll delta
    ///
    /// Value can be positive (up) or negative (down)
    pub fn mouse_scroll(&self) -> f32 {
        self.mouse_scroll_delta
    }

    /// Mouse scroll delta
    ///
    /// Value can be positive (up) or negative (down)
    pub fn mouse_horizontal_scroll(&self) -> f32 {
        self.mouse_horizontal_scroll_delta
    }

    /// Current mouse position in pixel coordinates. The top-left of the window is at (0, 0)
    pub fn mouse_position(&self) -> Option<&Vec2> {
        self.mouse_position.as_ref()
    }

    /// Difference of the mouse position from the last frame in pixel coordinates
    ///
    /// The top-left of the window is at (0, 0).
    pub fn mouse_delta(&self) -> Vec2 {
        self.mouse_delta
    }

    /// Returns true if mouse was moved this frame.
    ///
    /// This can be handy to do quick check before doing some performance costly operation.
    pub fn mouse_moved(&self) -> bool {
        self.mouse_moved
    }

    /// Normalized mouse position
    ///
    /// The top-left of the window is at (0, 0), bottom-right at (1, 1)
    pub fn mouse_position_normalized(&self) -> Vec2 {
        let (x, y) = self
            .mouse_position
            .as_ref()
            .map(|p| {
                (
                    f32::clamp(p.x / self.window_size.x, 0.0, 1.0),
                    f32::clamp(p.y / self.window_size.y, 0.0, 1.0),
                )
            })
            .unwrap_or((0.0, 0.0));

        Vec2::new(x, y)
    }

    /// This method must be called periodically to update states from events
    pub(crate) fn _reset(&mut self) {
        self.mouse_moved = false;
        self.mouse_delta = Vec2 { x: 0.0, y: 0.0 };
        self.mouse_scroll_delta = 0.0;
        self.mouse_horizontal_scroll_delta = 0.0;

        self.states.retain(|_btn, (state, _modifiers)| match state {
            EInputState::Activated => {
                *state = EInputState::Hold;
                true
            }
            EInputState::Deactivated => false,
            _ => true,
        });

        self.events.clear();
    }

    /// Handles input event
    pub fn on_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => self.on_keyboard_event(event),
            WindowEvent::MouseInput { state, button, .. } => {
                self.on_mouse_click_event(*state, *button)
            }
            WindowEvent::CursorMoved { position, .. } => self.on_cursor_moved_event(position),
            WindowEvent::MouseWheel { delta, .. } => self.on_mouse_wheel_event(delta),
            WindowEvent::Resized(size) => {
                self.window_size = Vec2::new(size.width as f32, size.height as f32)
            }
            WindowEvent::ModifiersChanged(input) => self.modifiers = *input,
            // WindowEvent::ReceivedCharacter(chr) => {
            //     if is_printable(*chr) && !self.modifiers.ctrl() && !self.modifiers.logo() {
            //         if let Some(EInputEvent::Text(text)) = self.events.last_mut() {
            //             text.push(*chr);
            //         } else {
            //             self.events.push(EInputEvent::Text(chr.to_string()));
            //         }
            //     }
            // }
            WindowEvent::HoveredFile(buffer) => self.on_hovered_file_event(buffer),
            WindowEvent::HoveredFileCancelled => self.on_hovered_file_canceled_event(),
            WindowEvent::DroppedFile(buffer) => self.on_dropped_file_event(buffer),
            _ => {}
        }
    }

    pub fn on_device_event(&mut self, event: &winit::event::DeviceEvent) {
        match event {
            winit::event::DeviceEvent::MouseMotion { delta } => {
                self.on_mouse_motion_event(delta);
            }
            _ => {}
        }
    }

    fn on_cursor_moved_event(&mut self, position: &winit::dpi::PhysicalPosition<f64>) {
        self.mouse_moved = true;
        self.mouse_position = Some(Vec2 {
            x: position.x as f32,
            y: position.y as f32,
        });
    }

    fn on_keyboard_event(&mut self, input: &KeyEvent) {
        let modifiers = self.modifiers;
        self.events.push(EInputEvent::Key(FKeyEvent {
            key_code: input.physical_key,
            modifiers,
            pressed: input.state == ElementState::Pressed,
        }));
        self.on_button_state(
            EInputButton::Key(input.physical_key),
            input.state,
            modifiers,
        );
    }

    fn on_button_state(&mut self, btn: EInputButton, state: ElementState, modifiers: Modifiers) {
        match state {
            ElementState::Pressed => {
                if *self
                    .states
                    .get(&btn)
                    .map(|(state, _modifiers)| state)
                    .unwrap_or(&EInputState::Deactivated)
                    == EInputState::Deactivated
                {
                    self.states.insert(btn, (EInputState::Activated, modifiers));
                }
            }
            ElementState::Released => {
                self.states
                    .insert(btn, (EInputState::Deactivated, modifiers));
            }
        }
    }

    fn on_mouse_click_event(&mut self, state: ElementState, mouse_btn: winit::event::MouseButton) {
        let btn: EInputButton = match mouse_btn {
            MouseButton::Left => EInputButton::MouseLeft,
            MouseButton::Right => EInputButton::MouseRight,
            MouseButton::Middle => EInputButton::MouseMiddle,
            MouseButton::Other(num) => EInputButton::MouseOther(num),
            _ => return,
        };

        self.on_button_state(btn, state, self.modifiers);
    }

    fn on_mouse_wheel_event(&mut self, delta: &MouseScrollDelta) {
        let (change_x, change_y) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (*x, *y),
            MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
        };
        self.mouse_scroll_delta += change_y;
        self.mouse_horizontal_scroll_delta += change_x;
    }

    fn on_mouse_motion_event(&mut self, delta: &(f64, f64)) {
        let (x, y) = *delta; // TODO: can descruct tuple as f32?
        self.mouse_delta.x += x as f32;
        self.mouse_delta.y += y as f32;
    }

    fn on_hovered_file_event(&mut self, path: &Path) {
        self.hovered_files.push(path.to_owned());
    }

    fn on_hovered_file_canceled_event(&mut self) {
        self.hovered_files.clear();
    }

    fn on_dropped_file_event(&mut self, path: &Path) {
        let path = path.to_owned();
        if let Some(dropped_files) = self.dropped_files.as_mut() {
            dropped_files.push(path);
        } else {
            self.dropped_files = Some(vec![path]);
        }
        self.hovered_files.clear();
    }
}

impl Default for Input {
    /// [`Input`] service constructor
    fn default() -> Self {
        let mapper = Box::new(FActionMapper::<EInputAction>::new());
        Self {
            mapper,
            states: HashMap::new(),
            mouse_scroll_delta: 0.0,
            mouse_horizontal_scroll_delta: 0.0,
            mouse_position: None,
            mouse_delta: Vec2::new(0.0, 0.0),
            mouse_moved: false,
            window_size: Vec2::new(0.0, 0.0),
            events: Vec::with_capacity(8),
            modifiers: Modifiers::default(),
            dropped_files: None,
            hovered_files: Vec::new(),
        }
    }
}
