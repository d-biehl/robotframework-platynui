//! Focus target types for input routing.
//!
//! These wrapper enums allow the compositor to direct keyboard and pointer
//! focus to different kinds of surfaces (toplevel windows, popups, layer
//! surfaces, etc.) through smithay's `SeatHandler` focus type system.

use std::{borrow::Cow, sync::Arc};

use smithay::{
    backend::input::{InputTime, KeyState, TabletToolDescriptor},
    desktop::{LayerSurface, PopupKind, Window},
    input::{
        Seat,
        dnd::{DndFocus, OfferData, Source},
        keyboard::{KeyboardTarget, KeysymHandle, ModifiersState},
        pointer::{
            AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
            GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
            GestureSwipeUpdateEvent, MotionEvent, PointerTarget, RelativeMotionEvent,
        },
        tablet::{
            Tablet,
            tool::{
                AxisFrame as ToolAxisFrame, ButtonEvent as ToolButtonEvent, DownEvent as ToolDownEvent,
                MotionEvent as ToolMotionEvent, TabletToolTarget, UpEvent as ToolUpEvent,
            },
        },
        touch::{FrameMarker, TouchTarget},
    },
    reexports::wayland_server::{DisplayHandle, Resource, backend::ObjectId, protocol::wl_surface::WlSurface},
    utils::{IsAlive, Logical, Point, Serial},
    wayland::{seat::WaylandFocus, selection::data_device::WlOfferData},
    xwayland::{X11Surface, xwm::XwmOfferData},
};

use crate::state::State;

/// Target for keyboard focus.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyboardFocusTarget {
    /// A toplevel or popup window.
    Window(Window),
    /// A popup surface (needed for popup grabs).
    Popup(Box<PopupKind>),
    /// A layer surface (panels, overlays, lock screens).
    LayerSurface(LayerSurface),
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
            Self::LayerSurface(l) => l.alive(),
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
            Self::LayerSurface(l) => Some(Cow::Borrowed(l.wl_surface())),
        }
    }

    fn same_client_as(&self, object_id: &ObjectId) -> bool {
        match self {
            Self::Window(w) => w.same_client_as(object_id),
            Self::Popup(p) => p.wl_surface().id().same_client_as(object_id),
            Self::LayerSurface(l) => l.wl_surface().id().same_client_as(object_id),
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

impl From<LayerSurface> for KeyboardFocusTarget {
    fn from(l: LayerSurface) -> Self {
        Self::LayerSurface(l)
    }
}

impl From<KeyboardFocusTarget> for PointerFocusTarget {
    fn from(target: KeyboardFocusTarget) -> Self {
        match target {
            KeyboardFocusTarget::Window(w) => Self::Window(w),
            KeyboardFocusTarget::Popup(p) => Self::Surface(p.wl_surface().clone()),
            KeyboardFocusTarget::LayerSurface(l) => Self::Surface(l.wl_surface().clone()),
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
            Self::LayerSurface(l) => {
                <WlSurface as KeyboardTarget<State>>::$method(l.wl_surface(), $($arg),*);
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
        time: InputTime,
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

    fn leave(&self, seat: &Seat<State>, data: &mut State, serial: Serial, time: InputTime) {
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
    fn down(&self, seat: &Seat<State>, data: &mut State, event: &smithay::input::touch::DownEvent) {
        delegate_to_surface_touch!(self, down, seat, data, event);
    }

    fn up(&self, seat: &Seat<State>, data: &mut State, event: &smithay::input::touch::UpEvent) {
        delegate_to_surface_touch!(self, up, seat, data, event);
    }

    fn motion(&self, seat: &Seat<State>, data: &mut State, event: &smithay::input::touch::MotionEvent) {
        delegate_to_surface_touch!(self, motion, seat, data, event);
    }

    fn frame(&self, seat: &Seat<State>, data: &mut State, marker: FrameMarker) {
        delegate_to_surface_touch!(self, frame, seat, data, marker);
    }

    fn cancel(&self, seat: &Seat<State>, data: &mut State, marker: FrameMarker) {
        delegate_to_surface_touch!(self, cancel, seat, data, marker);
    }

    fn shape(&self, seat: &Seat<State>, data: &mut State, event: &smithay::input::touch::ShapeEvent) {
        delegate_to_surface_touch!(self, shape, seat, data, event);
    }

    fn orientation(&self, seat: &Seat<State>, data: &mut State, event: &smithay::input::touch::OrientationEvent) {
        delegate_to_surface_touch!(self, orientation, seat, data, event);
    }

    fn last_frame(&self, seat: &Seat<State>, data: &mut State) -> Option<FrameMarker> {
        match self {
            Self::Window(w) => {
                w.wl_surface().and_then(|surface| <WlSurface as TouchTarget<State>>::last_frame(&surface, seat, data))
            }
            Self::Surface(s) => <WlSurface as TouchTarget<State>>::last_frame(s, seat, data),
        }
    }
}

// -- TabletToolTarget --

macro_rules! delegate_to_surface_tool {
    ($self:expr, $method:ident, $($arg:expr),* $(,)?) => {
        match $self {
            Self::Window(w) => {
                if let Some(surface) = w.wl_surface() {
                    <WlSurface as TabletToolTarget<State>>::$method(&*surface, $($arg),*);
                }
            }
            Self::Surface(s) => {
                <WlSurface as TabletToolTarget<State>>::$method(s, $($arg),*);
            }
        }
    };
}

impl TabletToolTarget<State> for PointerFocusTarget {
    fn proximity_in(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        tool: &TabletToolDescriptor,
        tablet: &Tablet,
        serial: Serial,
    ) {
        delegate_to_surface_tool!(self, proximity_in, seat, data, tool, tablet, serial);
    }

    fn proximity_out(&self, seat: &Seat<State>, data: &mut State, tool: &TabletToolDescriptor) {
        delegate_to_surface_tool!(self, proximity_out, seat, data, tool);
    }

    fn down(&self, seat: &Seat<State>, data: &mut State, tool: &TabletToolDescriptor, event: &ToolDownEvent) {
        delegate_to_surface_tool!(self, down, seat, data, tool, event);
    }

    fn up(&self, seat: &Seat<State>, data: &mut State, tool: &TabletToolDescriptor, event: &ToolUpEvent) {
        delegate_to_surface_tool!(self, up, seat, data, tool, event);
    }

    fn motion(&self, seat: &Seat<State>, data: &mut State, tool: &TabletToolDescriptor, event: &ToolMotionEvent) {
        delegate_to_surface_tool!(self, motion, seat, data, tool, event);
    }

    fn axis(&self, seat: &Seat<State>, data: &mut State, tool: &TabletToolDescriptor, frame: ToolAxisFrame) {
        delegate_to_surface_tool!(self, axis, seat, data, tool, frame);
    }

    fn button(&self, seat: &Seat<State>, data: &mut State, tool: &TabletToolDescriptor, event: &ToolButtonEvent) {
        delegate_to_surface_tool!(self, button, seat, data, tool, event);
    }

    fn frame(&self, seat: &Seat<State>, data: &mut State, tool: &TabletToolDescriptor, time: InputTime) {
        delegate_to_surface_tool!(self, frame, seat, data, tool, time);
    }
}

// -- DndFocus --

/// The drop-target protocol state of a drag over one of our focus targets:
/// a Wayland data offer, or an XDND offer to an X11 client.
pub enum CompositorOfferData<S: Source> {
    Wayland(WlOfferData<S>),
    X11(Box<XwmOfferData<S>>),
}

impl<S: Source> OfferData for CompositorOfferData<S> {
    fn disable(&self) {
        match self {
            Self::Wayland(data) => data.disable(),
            Self::X11(data) => data.disable(),
        }
    }

    fn drop(&self) {
        match self {
            Self::Wayland(data) => data.drop(),
            Self::X11(data) => OfferData::drop(data.as_ref()),
        }
    }

    fn validated(&self) -> bool {
        match self {
            Self::Wayland(data) => data.validated(),
            Self::X11(data) => data.validated(),
        }
    }
}

/// What a drag over a focus target talks to.
enum DndTarget {
    Wayland(WlSurface),
    X11(Box<X11Surface>),
}

impl PointerFocusTarget {
    fn dnd_target(&self) -> Option<DndTarget> {
        match self {
            Self::Window(w) => match w.x11_surface() {
                Some(x11) => Some(DndTarget::X11(Box::new(x11.clone()))),
                None => w.wl_surface().map(|s| DndTarget::Wayland(s.into_owned())),
            },
            Self::Surface(s) => Some(DndTarget::Wayland(s.clone())),
        }
    }
}

impl DndFocus<State> for PointerFocusTarget {
    type OfferData<S>
        = CompositorOfferData<S>
    where
        S: Source;

    fn enter<S: Source>(
        &self,
        data: &mut State,
        dh: &DisplayHandle,
        source: Arc<S>,
        seat: &Seat<State>,
        location: Point<f64, Logical>,
        serial: &Serial,
    ) -> Option<CompositorOfferData<S>> {
        match self.dnd_target()? {
            DndTarget::Wayland(s) => {
                DndFocus::enter(&s, data, dh, source, seat, location, serial).map(CompositorOfferData::Wayland)
            }
            DndTarget::X11(x) => DndFocus::enter(&*x, data, dh, source, seat, location, serial)
                .map(|o| CompositorOfferData::X11(Box::new(o))),
        }
    }

    fn motion<S: Source>(
        &self,
        data: &mut State,
        offer: Option<&mut CompositorOfferData<S>>,
        seat: &Seat<State>,
        location: Point<f64, Logical>,
        time: InputTime,
    ) {
        match (self.dnd_target(), offer) {
            (Some(DndTarget::Wayland(s)), Some(CompositorOfferData::Wayland(o))) => {
                DndFocus::motion(&s, data, Some(o), seat, location, time);
            }
            (Some(DndTarget::Wayland(s)), None) => DndFocus::<State>::motion::<S>(&s, data, None, seat, location, time),
            (Some(DndTarget::X11(x)), Some(CompositorOfferData::X11(o))) => {
                DndFocus::motion(&*x, data, Some(o.as_mut()), seat, location, time);
            }
            (Some(DndTarget::X11(x)), None) => DndFocus::<State>::motion::<S>(&*x, data, None, seat, location, time),
            _ => {}
        }
    }

    fn leave<S: Source>(&self, data: &mut State, offer: Option<&mut CompositorOfferData<S>>, seat: &Seat<State>) {
        match (self.dnd_target(), offer) {
            (Some(DndTarget::Wayland(s)), Some(CompositorOfferData::Wayland(o))) => {
                DndFocus::leave(&s, data, Some(o), seat);
            }
            (Some(DndTarget::Wayland(s)), None) => DndFocus::<State>::leave::<S>(&s, data, None, seat),
            (Some(DndTarget::X11(x)), Some(CompositorOfferData::X11(o))) => {
                DndFocus::leave(&*x, data, Some(o.as_mut()), seat);
            }
            (Some(DndTarget::X11(x)), None) => DndFocus::<State>::leave::<S>(&*x, data, None, seat),
            _ => {}
        }
    }

    fn drop<S: Source>(&self, data: &mut State, offer: Option<&mut CompositorOfferData<S>>, seat: &Seat<State>) {
        match (self.dnd_target(), offer) {
            (Some(DndTarget::Wayland(s)), Some(CompositorOfferData::Wayland(o))) => {
                DndFocus::drop(&s, data, Some(o), seat);
            }
            (Some(DndTarget::Wayland(s)), None) => DndFocus::<State>::drop::<S>(&s, data, None, seat),
            (Some(DndTarget::X11(x)), Some(CompositorOfferData::X11(o))) => {
                DndFocus::drop(&*x, data, Some(o.as_mut()), seat);
            }
            (Some(DndTarget::X11(x)), None) => DndFocus::<State>::drop::<S>(&*x, data, None, seat),
            _ => {}
        }
    }
}
