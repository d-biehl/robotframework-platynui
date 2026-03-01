//! Focus target types for input routing.
//!
//! These wrapper enums allow the compositor to direct keyboard and pointer
//! focus to different kinds of surfaces (toplevel windows, popups, layer
//! surfaces, etc.) through smithay's `SeatHandler` focus type system.

use std::borrow::Cow;

use smithay::{
    backend::input::KeyState,
    desktop::{PopupKind, Window},
    input::{
        Seat,
        keyboard::{KeyboardTarget, KeysymHandle, ModifiersState},
        pointer::{
            AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
            GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
            GestureSwipeUpdateEvent, MotionEvent, PointerTarget, RelativeMotionEvent,
        },
        touch::TouchTarget,
    },
    reexports::wayland_server::{Resource, backend::ObjectId, protocol::wl_surface::WlSurface},
    utils::{IsAlive, Serial},
    wayland::seat::WaylandFocus,
};

use crate::state::State;

/// Target for keyboard focus.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyboardFocusTarget {
    /// A toplevel or popup window.
    Window(Window),
    /// A popup surface (needed for popup grabs).
    Popup(Box<PopupKind>),
}

/// Target for pointer focus.
///
/// Pointer events need to be routed to the exact `WlSurface` under the cursor
/// (which may be a subsurface or popup), not just the toplevel.  The `Surface`
/// variant carries the specific surface so that `wl_pointer.enter` and friends
/// reference the right object for the client.
#[derive(Debug, Clone, PartialEq)]
pub enum PointerFocusTarget {
    /// A toplevel window (used as fallback when no specific surface is found).
    Window(Window),
    /// A specific Wayland surface (subsurface, popup, or toplevel surface).
    Surface(WlSurface),
}

// -- IsAlive --

impl IsAlive for KeyboardFocusTarget {
    fn alive(&self) -> bool {
        match self {
            Self::Window(w) => w.alive(),
            Self::Popup(p) => p.alive(),
        }
    }
}

impl IsAlive for PointerFocusTarget {
    fn alive(&self) -> bool {
        match self {
            Self::Window(w) => w.alive(),
            Self::Surface(s) => s.alive(),
        }
    }
}

// -- WaylandFocus --

impl WaylandFocus for KeyboardFocusTarget {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            Self::Window(w) => w.wl_surface(),
            Self::Popup(p) => Some(Cow::Borrowed(p.wl_surface())),
        }
    }

    fn same_client_as(&self, object_id: &ObjectId) -> bool {
        match self {
            Self::Window(w) => w.same_client_as(object_id),
            Self::Popup(p) => p.wl_surface().id().same_client_as(object_id),
        }
    }
}

impl WaylandFocus for PointerFocusTarget {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            Self::Window(w) => w.wl_surface(),
            Self::Surface(s) => Some(Cow::Borrowed(s)),
        }
    }

    fn same_client_as(&self, object_id: &ObjectId) -> bool {
        match self {
            Self::Window(w) => w.same_client_as(object_id),
            Self::Surface(s) => s.id().same_client_as(object_id),
        }
    }
}

// -- From impls --

impl From<Window> for KeyboardFocusTarget {
    fn from(w: Window) -> Self {
        Self::Window(w)
    }
}

impl From<Window> for PointerFocusTarget {
    fn from(w: Window) -> Self {
        Self::Window(w)
    }
}

impl From<WlSurface> for PointerFocusTarget {
    fn from(s: WlSurface) -> Self {
        Self::Surface(s)
    }
}

impl From<PopupKind> for KeyboardFocusTarget {
    fn from(p: PopupKind) -> Self {
        Self::Popup(Box::new(p))
    }
}

impl From<KeyboardFocusTarget> for PointerFocusTarget {
    fn from(target: KeyboardFocusTarget) -> Self {
        match target {
            KeyboardFocusTarget::Window(w) => Self::Window(w),
            KeyboardFocusTarget::Popup(p) => Self::Surface(p.wl_surface().clone()),
        }
    }
}

// -- Helper to extract &WlSurface from focus targets --

/// Helper macro to delegate a focus target method to the inner `WlSurface`.
/// `Cow<'_, WlSurface>` doesn't implement the target traits, but `WlSurface` does.
/// We dereference the Cow to get `&WlSurface` and call the trait method on it.
macro_rules! delegate_to_surface {
    ($self:expr, $method:ident, $($arg:expr),* $(,)?) => {
        match $self {
            Self::Window(w) => {
                if let Some(surface) = w.wl_surface() {
                    <WlSurface as KeyboardTarget<State>>::$method(&*surface, $($arg),*);
                }
            }
            Self::Popup(p) => {
                <WlSurface as KeyboardTarget<State>>::$method(p.wl_surface(), $($arg),*);
            }
        }
    };
}

macro_rules! delegate_to_surface_pointer {
    ($self:expr, $method:ident, $($arg:expr),* $(,)?) => {
        match $self {
            Self::Window(w) => {
                if let Some(surface) = w.wl_surface() {
                    <WlSurface as PointerTarget<State>>::$method(&*surface, $($arg),*);
                }
            }
            Self::Surface(s) => {
                <WlSurface as PointerTarget<State>>::$method(s, $($arg),*);
            }
        }
    };
}

macro_rules! delegate_to_surface_touch {
    ($self:expr, $method:ident, $($arg:expr),* $(,)?) => {
        match $self {
            Self::Window(w) => {
                if let Some(surface) = w.wl_surface() {
                    <WlSurface as TouchTarget<State>>::$method(&*surface, $($arg),*);
                }
            }
            Self::Surface(s) => {
                <WlSurface as TouchTarget<State>>::$method(s, $($arg),*);
            }
        }
    };
}

// -- KeyboardTarget --

impl KeyboardTarget<State> for KeyboardFocusTarget {
    fn enter(&self, seat: &Seat<State>, data: &mut State, keys: Vec<KeysymHandle<'_>>, serial: Serial) {
        delegate_to_surface!(self, enter, seat, data, keys, serial);
    }

    fn leave(&self, seat: &Seat<State>, data: &mut State, serial: Serial) {
        delegate_to_surface!(self, leave, seat, data, serial);
    }

    fn key(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        key: KeysymHandle<'_>,
        state: KeyState,
        serial: Serial,
        time: u32,
    ) {
        delegate_to_surface!(self, key, seat, data, key, state, serial, time);
    }

    fn modifiers(&self, seat: &Seat<State>, data: &mut State, modifiers: ModifiersState, serial: Serial) {
        delegate_to_surface!(self, modifiers, seat, data, modifiers, serial);
    }
}

// -- PointerTarget --

impl PointerTarget<State> for PointerFocusTarget {
    fn enter(&self, seat: &Seat<State>, data: &mut State, event: &MotionEvent) {
        delegate_to_surface_pointer!(self, enter, seat, data, event);
    }

    fn motion(&self, seat: &Seat<State>, data: &mut State, event: &MotionEvent) {
        delegate_to_surface_pointer!(self, motion, seat, data, event);
    }

    fn relative_motion(&self, seat: &Seat<State>, data: &mut State, event: &RelativeMotionEvent) {
        delegate_to_surface_pointer!(self, relative_motion, seat, data, event);
    }

    fn button(&self, seat: &Seat<State>, data: &mut State, event: &ButtonEvent) {
        delegate_to_surface_pointer!(self, button, seat, data, event);
    }

    fn axis(&self, seat: &Seat<State>, data: &mut State, frame: AxisFrame) {
        delegate_to_surface_pointer!(self, axis, seat, data, frame);
    }

    fn frame(&self, seat: &Seat<State>, data: &mut State) {
        delegate_to_surface_pointer!(self, frame, seat, data);
    }

    fn leave(&self, seat: &Seat<State>, data: &mut State, serial: Serial, time: u32) {
        delegate_to_surface_pointer!(self, leave, seat, data, serial, time);
    }

    fn gesture_swipe_begin(&self, seat: &Seat<State>, data: &mut State, event: &GestureSwipeBeginEvent) {
        delegate_to_surface_pointer!(self, gesture_swipe_begin, seat, data, event);
    }

    fn gesture_swipe_update(&self, seat: &Seat<State>, data: &mut State, event: &GestureSwipeUpdateEvent) {
        delegate_to_surface_pointer!(self, gesture_swipe_update, seat, data, event);
    }

    fn gesture_swipe_end(&self, seat: &Seat<State>, data: &mut State, event: &GestureSwipeEndEvent) {
        delegate_to_surface_pointer!(self, gesture_swipe_end, seat, data, event);
    }

    fn gesture_pinch_begin(&self, seat: &Seat<State>, data: &mut State, event: &GesturePinchBeginEvent) {
        delegate_to_surface_pointer!(self, gesture_pinch_begin, seat, data, event);
    }

    fn gesture_pinch_update(&self, seat: &Seat<State>, data: &mut State, event: &GesturePinchUpdateEvent) {
        delegate_to_surface_pointer!(self, gesture_pinch_update, seat, data, event);
    }

    fn gesture_pinch_end(&self, seat: &Seat<State>, data: &mut State, event: &GesturePinchEndEvent) {
        delegate_to_surface_pointer!(self, gesture_pinch_end, seat, data, event);
    }

    fn gesture_hold_begin(&self, seat: &Seat<State>, data: &mut State, event: &GestureHoldBeginEvent) {
        delegate_to_surface_pointer!(self, gesture_hold_begin, seat, data, event);
    }

    fn gesture_hold_end(&self, seat: &Seat<State>, data: &mut State, event: &GestureHoldEndEvent) {
        delegate_to_surface_pointer!(self, gesture_hold_end, seat, data, event);
    }
}

// -- TouchTarget --

impl TouchTarget<State> for PointerFocusTarget {
    fn down(&self, seat: &Seat<State>, data: &mut State, event: &smithay::input::touch::DownEvent, seq: Serial) {
        delegate_to_surface_touch!(self, down, seat, data, event, seq);
    }

    fn up(&self, seat: &Seat<State>, data: &mut State, event: &smithay::input::touch::UpEvent, seq: Serial) {
        delegate_to_surface_touch!(self, up, seat, data, event, seq);
    }

    fn motion(&self, seat: &Seat<State>, data: &mut State, event: &smithay::input::touch::MotionEvent, seq: Serial) {
        delegate_to_surface_touch!(self, motion, seat, data, event, seq);
    }

    fn frame(&self, seat: &Seat<State>, data: &mut State, seq: Serial) {
        delegate_to_surface_touch!(self, frame, seat, data, seq);
    }

    fn cancel(&self, seat: &Seat<State>, data: &mut State, seq: Serial) {
        delegate_to_surface_touch!(self, cancel, seat, data, seq);
    }

    fn shape(&self, seat: &Seat<State>, data: &mut State, event: &smithay::input::touch::ShapeEvent, seq: Serial) {
        delegate_to_surface_touch!(self, shape, seat, data, event, seq);
    }

    fn orientation(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &smithay::input::touch::OrientationEvent,
        seq: Serial,
    ) {
        delegate_to_surface_touch!(self, orientation, seat, data, event, seq);
    }
}
