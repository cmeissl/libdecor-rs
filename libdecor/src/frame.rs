use std::{
    any::Any,
    ffi::{c_void, CStr, CString},
    os::raw::c_char,
};
use wayland_client::DispatchData;
use wayland_protocols::xdg_shell::client::{xdg_surface, xdg_toplevel};

use libdecor_sys::*;

bitflags::bitflags! {
    /// The possible window states
    pub struct WindowState: libdecor_window_state {
        const ACTIVE = LIBDECOR_WINDOW_STATE_ACTIVE;
        const MAXIMIZED = LIBDECOR_WINDOW_STATE_MAXIMIZED;
        const FULLSCREEN = LIBDECOR_WINDOW_STATE_FULLSCREEN;
        const TILED_LEFT = LIBDECOR_WINDOW_STATE_TILED_LEFT;
        const TILED_RIGHT = LIBDECOR_WINDOW_STATE_TILED_RIGHT;
        const TILED_TOP = LIBDECOR_WINDOW_STATE_TILED_TOP;
        const TILED_BOTTOM = LIBDECOR_WINDOW_STATE_TILED_BOTTOM;
    }

    /// Capabilities of a [`Frame`]
    pub struct Capabilities: libdecor_capabilities {
        const MOVE = LIBDECOR_ACTION_MOVE;
        const RESIZE = LIBDECOR_ACTION_RESIZE;
        const MINIMIZE = LIBDECOR_ACTION_MINIMIZE;
        const FULLSCREEN = LIBDECOR_ACTION_FULLSCREEN;
        const CLOSE = LIBDECOR_ACTION_CLOSE;
    }
}

#[derive(Debug)]
pub enum ResizeEdge {
    None,
    Top,
    Bottom,
    Left,
    TopLeft,
    BottomLeft,
    Right,
    TopRight,
    BottomRight,
}

impl From<libdecor_resize_edge> for ResizeEdge {
    fn from(edge: libdecor_resize_edge) -> Self {
        match edge {
            LIBDECOR_RESIZE_EDGE_NONE => Self::None,
            LIBDECOR_RESIZE_EDGE_TOP => Self::Top,
            LIBDECOR_RESIZE_EDGE_BOTTOM => Self::Bottom,
            LIBDECOR_RESIZE_EDGE_LEFT => Self::Left,
            LIBDECOR_RESIZE_EDGE_TOP_LEFT => Self::TopLeft,
            LIBDECOR_RESIZE_EDGE_BOTTOM_LEFT => Self::BottomLeft,
            LIBDECOR_RESIZE_EDGE_RIGHT => Self::Right,
            LIBDECOR_RESIZE_EDGE_TOP_RIGHT => Self::TopRight,
            LIBDECOR_RESIZE_EDGE_BOTTOM_RIGHT => Self::BottomRight,
            _ => unreachable!(),
        }
    }
}

impl From<ResizeEdge> for libdecor_resize_edge {
    fn from(edge: ResizeEdge) -> Self {
        match edge {
            ResizeEdge::None => LIBDECOR_RESIZE_EDGE_NONE,
            ResizeEdge::Top => LIBDECOR_RESIZE_EDGE_TOP,
            ResizeEdge::Bottom => LIBDECOR_RESIZE_EDGE_BOTTOM,
            ResizeEdge::Left => LIBDECOR_RESIZE_EDGE_LEFT,
            ResizeEdge::TopLeft => LIBDECOR_RESIZE_EDGE_TOP_LEFT,
            ResizeEdge::BottomLeft => LIBDECOR_RESIZE_EDGE_BOTTOM_LEFT,
            ResizeEdge::Right => LIBDECOR_RESIZE_EDGE_RIGHT,
            ResizeEdge::TopRight => LIBDECOR_RESIZE_EDGE_TOP_RIGHT,
            ResizeEdge::BottomRight => LIBDECOR_RESIZE_EDGE_BOTTOM_RIGHT,
        }
    }
}

pub(crate) type FrameCallback = dyn FnMut(FrameRef, FrameRequest, DispatchData);

/// An object representing a toplevel window configuration.
#[derive(Debug)]
pub struct Configuration(*mut libdecor_configuration);

impl Configuration {
    /// Get the expected size of the content for this configuration.
    ///
    /// If the configuration doesn't contain a size, [`None`] is returned.
    pub fn content_size(&self, frame: &FrameRef) -> Option<(i32, i32)> {
        let mut width = 0;
        let mut height = 0;

        let has_content_size = unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_configuration_get_content_size,
                self.0,
                frame.0,
                &mut width,
                &mut height
            )
        };

        if has_content_size {
            Some((width, height))
        } else {
            None
        }
    }

    /// Get the [`WindowState`] for this configuration.
    ///
    /// If the configuration doesn't contain any associated window state, [`None`] is
    /// returned, and the application should assume the window state remains
    /// unchanged.
    pub fn window_state(&self) -> Option<WindowState> {
        let mut window_state: libdecor_window_state = LIBDECOR_WINDOW_STATE_NONE;

        let has_window_state = unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_configuration_get_window_state,
                self.0,
                &mut window_state
            )
        };

        if has_window_state && window_state != LIBDECOR_WINDOW_STATE_NONE {
            Some(WindowState::from_bits(window_state).unwrap())
        } else {
            None
        }
    }
}

/// An object corresponding to a configured content state.
#[derive(Debug)]
pub struct State(*mut libdecor_state);

impl State {
    /// Create a new content surface state.
    pub fn new(width: i32, height: i32) -> Self {
        let state = unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_state_new, width, height) };
        State(state)
    }
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_state_free, self.0) }
    }
}

fn invoke_frame_callback(
    frame: *mut libdecor_frame,
    user_data: *mut c_void,
    request: FrameRequest,
) {
    assert!(crate::DISPATCH_METADATA.is_set());

    let callback = unsafe { &mut *(user_data as *mut std::boxed::Box<FrameCallback>) };
    let frame_ref = FrameRef(frame);

    crate::DISPATCH_METADATA.with(|ddata| callback(frame_ref, request, ddata.get().reborrow()));
}

extern "C" fn configure_callback_trampolin(
    frame: *mut libdecor_frame,
    configuration: *mut libdecor_configuration,
    user_data: *mut c_void,
) {
    invoke_frame_callback(
        frame,
        user_data,
        FrameRequest::Configure(Configuration(configuration)),
    )
}

extern "C" fn close_callback_trampolin(frame: *mut libdecor_frame, user_data: *mut c_void) {
    invoke_frame_callback(frame, user_data, FrameRequest::Close)
}

extern "C" fn commit_callback_trampolin(frame: *mut libdecor_frame, user_data: *mut c_void) {
    invoke_frame_callback(frame, user_data, FrameRequest::Commit)
}

extern "C" fn dismiss_popup_callback_trampolin(
    frame: *mut libdecor_frame,
    seat_name: *const c_char,
    user_data: *mut c_void,
) {
    let seat_name = unsafe { CStr::from_ptr(seat_name) };

    invoke_frame_callback(
        frame,
        user_data,
        FrameRequest::DismissPopup {
            seat_name: seat_name.to_str().unwrap().to_owned(),
        },
    )
}

#[derive(Debug)]
#[non_exhaustive]
pub enum FrameRequest {
    /// A new configuration was received. An application should respond to
    /// this by creating a suitable [`State`], and apply it using
    /// [`FrameRef::commit`].
    Configure(Configuration),
    /// The window was requested to be closed by the compositor.
    Close,
    /// The window decoration asked to have the main surface to be
    /// committed. This is required when the decoration is implemented using
    /// synchronous subsurfaces.
    Commit,
    /// Any mapped popup that has a grab on the given seat should be
    /// dismissed.
    DismissPopup { seat_name: String },
}

pub(crate) static LIBDECOR_FRAME_INTERFACE: libdecor_frame_interface = libdecor_frame_interface {
    configure: configure_callback_trampolin,
    close: close_callback_trampolin,
    commit: commit_callback_trampolin,
    dismiss_popup: dismiss_popup_callback_trampolin,
    reserved0: None,
    reserved1: None,
    reserved2: None,
    reserved3: None,
    reserved4: None,
    reserved5: None,
    reserved6: None,
    reserved7: None,
    reserved8: None,
    reserved9: None,
};

/// A reference to a [`Frame`] used for decorating a Wayland surface.
#[derive(Debug)]
pub struct FrameRef(pub(crate) *mut libdecor_frame);

impl FrameRef {
    /// Close the window.
    ///
    /// Roughly translates to [`xdg_toplevel::Event::Close`].
    pub fn close(&self) {
        unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_frame_close, self.0) }
    }

    /// Map the window.
    ///
    /// This will eventually result in the initial configure event.
    pub fn map(&self) {
        unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_frame_map, self.0) }
    }

    /// Return [`true`] if the window is floating.
    ///
    /// A window is floating when it's not maximized, tiled, fullscreen, or in any
    /// similar way with a fixed size and state.
    /// Note that this function uses the "applied" configuration. If this function
    /// is used in the 'configure' callback, the provided configuration has to be
    /// applied via [`commit`](#method.commit) first, before it will reflect the current
    /// window state from the provided configuration.
    pub fn is_floating(&self) -> bool {
        unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_frame_is_floating, self.0) }
    }

    /// Set the application ID of the window.
    pub fn set_app_id<S: AsRef<str>>(&self, app_id: S) {
        let app_id = app_id.as_ref();
        let app_id = CString::new(app_id).unwrap();

        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_set_app_id,
                self.0,
                app_id.as_ptr()
            )
        }
    }

    /// Set the title of the window.
    pub fn set_title<S: AsRef<str>>(&self, title: S) {
        let title = title.as_ref();
        let title = CString::new(title).unwrap();

        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_set_title,
                self.0,
                title.as_ptr()
            )
        }
    }

    /// Get the title of the window.
    pub fn title(&self) -> String {
        let title = unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_frame_get_title, self.0) };
        let title = unsafe { CStr::from_ptr(title) };
        title.to_string_lossy().to_string()
    }

    /// Set new capabilities of the window.
    ///
    /// This determines whether e.g. a window decoration should show a maximize
    /// button, etc.
    ///
    /// Setting a capability does not implicitly unset any other.
    pub fn set_capabilities(&self, capabilities: Capabilities) {
        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_set_capabilities,
                self.0,
                capabilities.bits()
            )
        }
    }

    /// Unset capabilities of the window.
    ///
    /// The opposite of [`set_capabilities`](#method.set_capabilities).
    pub fn unset_capabilities(&self, capabilities: Capabilities) {
        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_unset_capabilities,
                self.0,
                capabilities.bits()
            )
        }
    }

    /// Check whether the window has any of the given capabilities.
    pub fn has_capability(&self, capabilities: Capabilities) -> bool {
        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_has_capability,
                self.0,
                capabilities.bits()
            )
        }
    }

    /// Show the window menu.
    pub fn show_window_menu(
        &self,
        seat: &wayland_client::protocol::wl_seat::WlSeat,
        serial: u32,
        x: i32,
        y: i32,
    ) {
        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_show_window_menu,
                self.0,
                seat.as_ref().c_ptr() as *mut _,
                serial,
                x,
                y
            )
        }
    }

    /// Issue a popup grab on the window. Call this when a [`wayland_protocols::xdg_shell::client::xdg_popup::XdgPopup`] is mapped, so
    /// that it can be properly dismissed by the decorations.
    pub fn popup_grab<S: AsRef<str>>(&self, seat_name: S) {
        let seat_name = seat_name.as_ref();
        let seat_name = CString::new(seat_name).unwrap();

        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_popup_grab,
                self.0,
                seat_name.as_ptr()
            )
        }
    }

    /// Release the popup grab. Call this when you unmap a popup.
    pub fn popup_ungrab<S: AsRef<str>>(&self, seat_name: S) {
        let seat_name = seat_name.as_ref();
        let seat_name = CString::new(seat_name).unwrap();

        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_popup_ungrab,
                self.0,
                seat_name.as_ptr()
            )
        }
    }

    /// Translate content surface local coordinates to toplevel window local
    /// coordinates.
    /// This can be used to translate surface coordinates to coordinates useful for
    /// e.g. showing the window menu, or positioning a popup.
    pub fn translate_coordinate(&self, surface_x: i32, surface_y: i32) -> (i32, i32) {
        let mut frame_x: i32 = 0;
        let mut frame_y: i32 = 0;
        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_translate_coordinate,
                self.0,
                surface_x,
                surface_y,
                &mut frame_x,
                &mut frame_y
            )
        }

        (frame_x, frame_y)
    }

    /// Set the max content size.
    ///
    /// This translates roughly to [`xdg_toplevel::XdgToplevel::set_max_size`].
    pub fn set_max_content_size(&self, width: i32, height: i32) {
        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_set_max_content_size,
                self.0,
                width,
                height
            )
        }
    }

    /// Set the min content size.
    ///
    /// This translates roughly to [`xdg_toplevel::XdgToplevel::set_min_size`].
    pub fn set_min_content_size(&self, width: i32, height: i32) {
        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_set_min_content_size,
                self.0,
                width,
                height
            )
        }
    }

    /// Initiate an interactive resize.
    ///
    /// This roughly translates to [`xdg_toplevel::XdgToplevel::resize`].
    pub fn resize(
        &self,
        seat: &wayland_client::protocol::wl_seat::WlSeat,
        serial: u32,
        edge: ResizeEdge,
    ) {
        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_resize,
                self.0,
                seat.as_ref().c_ptr() as *mut _,
                serial,
                edge.into()
            )
        }
    }

    /// Initiate an interactive move.
    ///
    /// This roughly translates to [`xdg_toplevel::XdgToplevel::_move`].
    pub fn _move(&self, seat: &wayland_client::protocol::wl_seat::WlSeat, serial: u32) {
        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_move,
                self.0,
                seat.as_ref().c_ptr() as *mut _,
                serial
            )
        }
    }

    /// Commit a new window state. This can be called on application driven resizes
    /// when the window is floating, or in response to received configurations, i.e.
    /// from e.g. interactive resizes or state changes.
    pub fn commit(&self, state: &State, configuration: Option<&Configuration>) {
        let configuration = configuration
            .map(|c| c.0)
            .unwrap_or_else(std::ptr::null_mut);
        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_commit,
                self.0,
                state.0,
                configuration
            )
        }
    }

    /// Minimize the window.
    ///
    /// Roughly translates to [`xdg_toplevel::XdgToplevel::set_minimized`].
    pub fn set_minimized(&self) {
        unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_frame_set_minimized, self.0) }
    }

    /// Maximize the window.
    ///
    /// Roughly translates to [`xdg_toplevel::XdgToplevel::set_maximized`].
    pub fn set_maximized(&self) {
        unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_frame_set_maximized, self.0) }
    }

    /// Unmaximize the window.
    ///
    /// Roughly translates to [`xdg_toplevel::XdgToplevel::unset_maximized`].
    pub fn unset_maximized(&self) {
        unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_frame_unset_maximized, self.0) }
    }

    /// Fullscreen the window.
    ///
    /// Roughly translates to [`xdg_toplevel::XdgToplevel::set_fullscreen`].
    pub fn set_fullscreen(&self, output: Option<&wayland_client::protocol::wl_output::WlOutput>) {
        let output = output
            .map(|o| o.as_ref().c_ptr())
            .unwrap_or_else(std::ptr::null_mut);
        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_set_fullscreen,
                self.0,
                output as *mut _
            )
        }
    }

    /// Unfullscreen the window.
    ///
    /// Roughly translates to [`xdg_toplevel::XdgToplevel::unset_fullscreen`].
    pub fn unset_fullscreen(&self) {
        unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_frame_unset_fullscreen, self.0) }
    }

    /// Set the visibility of the frame.
    ///
    /// If an application wants to be borderless, it can set the frame visibility to
    /// false.
    pub fn set_visibility(&self, visible: bool) {
        unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_frame_set_visibility,
                self.0,
                visible
            )
        }
    }

    /// Get the visibility of the frame.
    pub fn is_visible(&self) -> bool {
        unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_frame_is_visible, self.0) }
    }

    /// Set the parent of the window.
    ///
    /// This can be used to stack multiple toplevel windows above or under each
    /// other.
    pub fn set_parent(&self, parent: &FrameRef) {
        unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_frame_set_parent, self.0, parent.0) }
    }

    /// Get the associated [`xdg_surface::XdgSurface`] for content [`wayland_client::protocol::wl_surface::WlSurface`].
    pub fn xdg_surface(&self) -> Option<xdg_surface::XdgSurface> {
        let xdg_surface =
            unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_frame_get_xdg_surface, self.0) };

        if xdg_surface.is_null() {
            None
        } else {
            let proxy = unsafe { wayland_client::Proxy::from_c_ptr(xdg_surface as *mut _) };
            Some(proxy.into())
        }
    }

    /// Get the associated [`xdg_toplevel::XdgToplevel`] for the content [`wayland_client::protocol::wl_surface::WlSurface`].
    pub fn xdg_toplevel(&self) -> Option<xdg_toplevel::XdgToplevel> {
        let xdg_toplevel =
            unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_frame_get_xdg_toplevel, self.0) };

        if xdg_toplevel.is_null() {
            None
        } else {
            let proxy = unsafe { wayland_client::Proxy::from_c_ptr(xdg_toplevel as *mut _) };
            Some(proxy.into())
        }
    }
}

/// A frame used for decorating a Wayland surface.
#[derive(Debug)]
pub struct Frame {
    pub(crate) frame_ref: FrameRef,
    pub(crate) cb: *mut Box<FrameCallback>,
    pub(crate) _context: crate::Context,
}

impl Frame {
    pub fn dispatch<T, F, R>(&self, ddata: &mut T, f: F) -> R
    where
        T: Any,
        F: FnOnce(&FrameRef) -> R,
    {
        let ddata = unsafe { std::mem::transmute(ddata) };
        let ddata = DispatchData::wrap::<T>(ddata);
        let ddata_mut = crate::DispatchDataMut::new(ddata);
        crate::DISPATCH_METADATA.set(&ddata_mut, || f(&self.frame_ref))
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_frame_unref, self.frame_ref.0) }
        let _ = unsafe { Box::from_raw(self.cb) };
    }
}
