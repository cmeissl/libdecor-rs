#![allow(non_camel_case_types)]

use std::{
    ffi::c_void,
    os::raw::{c_char, c_int, c_uint},
};

use dlib::external_library;

pub enum libdecor {}
pub enum libdecor_frame {}
pub enum libdecor_configuration {}
pub enum libdecor_state {}
pub type libdecor_error = c_uint;
pub type libdecor_window_state = c_uint;
pub type libdecor_resize_edge = c_uint;
pub type libdecor_capabilities = c_uint;
pub enum wl_seat {}
pub enum wl_display {}
pub enum wl_surface {}
pub enum wl_output {}
pub enum xdg_surface {}
pub enum xdg_toplevel {}

pub const LIBDECOR_ERROR_COMPOSITOR_INCOMPATIBLE: libdecor_error = 0;
pub const LIBDECOR_ERROR_INVALID_FRAME_CONFIGURATION: libdecor_error = 1;
pub const LIBDECOR_WINDOW_STATE_NONE: libdecor_window_state = 0;
pub const LIBDECOR_WINDOW_STATE_ACTIVE: libdecor_window_state = 1;
pub const LIBDECOR_WINDOW_STATE_MAXIMIZED: libdecor_window_state = 2;
pub const LIBDECOR_WINDOW_STATE_FULLSCREEN: libdecor_window_state = 4;
pub const LIBDECOR_WINDOW_STATE_TILED_LEFT: libdecor_window_state = 8;
pub const LIBDECOR_WINDOW_STATE_TILED_RIGHT: libdecor_window_state = 16;
pub const LIBDECOR_WINDOW_STATE_TILED_TOP: libdecor_window_state = 32;
pub const LIBDECOR_WINDOW_STATE_TILED_BOTTOM: libdecor_window_state = 64;
pub const LIBDECOR_RESIZE_EDGE_NONE: libdecor_resize_edge = 0;
pub const LIBDECOR_RESIZE_EDGE_TOP: libdecor_resize_edge = 1;
pub const LIBDECOR_RESIZE_EDGE_BOTTOM: libdecor_resize_edge = 2;
pub const LIBDECOR_RESIZE_EDGE_LEFT: libdecor_resize_edge = 3;
pub const LIBDECOR_RESIZE_EDGE_TOP_LEFT: libdecor_resize_edge = 4;
pub const LIBDECOR_RESIZE_EDGE_BOTTOM_LEFT: libdecor_resize_edge = 5;
pub const LIBDECOR_RESIZE_EDGE_RIGHT: libdecor_resize_edge = 6;
pub const LIBDECOR_RESIZE_EDGE_TOP_RIGHT: libdecor_resize_edge = 7;
pub const LIBDECOR_RESIZE_EDGE_BOTTOM_RIGHT: libdecor_resize_edge = 8;
pub const LIBDECOR_ACTION_MOVE: libdecor_capabilities = 1;
pub const LIBDECOR_ACTION_RESIZE: libdecor_capabilities = 2;
pub const LIBDECOR_ACTION_MINIMIZE: libdecor_capabilities = 4;
pub const LIBDECOR_ACTION_FULLSCREEN: libdecor_capabilities = 8;
pub const LIBDECOR_ACTION_CLOSE: libdecor_capabilities = 16;

pub type libdecor_error_callback =
    unsafe extern "C" fn(context: *mut libdecor, error: libdecor_error, message: *const c_char);

type libdecor_reserver_callback = std::option::Option<unsafe extern "C" fn()>;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct libdecor_interface {
    /// An error event
    pub error: libdecor_error_callback,
    pub reserved0: libdecor_reserver_callback,
    pub reserved1: libdecor_reserver_callback,
    pub reserved2: libdecor_reserver_callback,
    pub reserved3: libdecor_reserver_callback,
    pub reserved4: libdecor_reserver_callback,
    pub reserved5: libdecor_reserver_callback,
    pub reserved6: libdecor_reserver_callback,
    pub reserved7: libdecor_reserver_callback,
    pub reserved8: libdecor_reserver_callback,
    pub reserved9: libdecor_reserver_callback,
}

pub type libdecor_configure_callback = unsafe extern "C" fn(
    frame: *mut libdecor_frame,
    configuration: *mut libdecor_configuration,
    user_data: *mut c_void,
);

pub type libdecor_close_callback =
    unsafe extern "C" fn(frame: *mut libdecor_frame, user_data: *mut c_void);

pub type libdecor_commit_callback =
    unsafe extern "C" fn(frame: *mut libdecor_frame, user_data: *mut c_void);

pub type libdecor_dismiss_popup_callback = unsafe extern "C" fn(
    frame: *mut libdecor_frame,
    seat_name: *const c_char,
    user_data: *mut c_void,
);

/// Interface for integrating a Wayland surface with libdecor.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct libdecor_frame_interface {
    /// A new configuration was received. An application should respond to
    /// this by creating a suitable libdecor_state, and apply it using
    /// libdecor_frame_commit.
    pub configure: libdecor_configure_callback,
    /// The window was requested to be closed by the compositor.
    pub close: libdecor_close_callback,
    /// The window decoration asked to have the main surface to be
    /// committed. This is required when the decoration is implemented using
    /// synchronous subsurfaces.
    pub commit: libdecor_commit_callback,
    /// Any mapped popup that has a grab on the given seat should be
    /// dismissed.
    pub dismiss_popup: libdecor_dismiss_popup_callback,
    pub reserved0: libdecor_reserver_callback,
    pub reserved1: libdecor_reserver_callback,
    pub reserved2: libdecor_reserver_callback,
    pub reserved3: libdecor_reserver_callback,
    pub reserved4: libdecor_reserver_callback,
    pub reserved5: libdecor_reserver_callback,
    pub reserved6: libdecor_reserver_callback,
    pub reserved7: libdecor_reserver_callback,
    pub reserved8: libdecor_reserver_callback,
    pub reserved9: libdecor_reserver_callback,
}

external_library!(Libdecor, "decor-0",
    functions:
        fn libdecor_new(*mut wl_display, *mut libdecor_interface) -> *mut libdecor,
        fn libdecor_unref(*mut libdecor) -> (),
        fn libdecor_get_fd(*mut libdecor) -> c_int,
        fn libdecor_dispatch(
            *mut libdecor,
            c_int
        ) -> c_int,
        fn libdecor_decorate(
            *mut libdecor,
            *mut wl_surface,
            *mut libdecor_frame_interface,
            *mut c_void
        ) -> *mut libdecor_frame,
        fn libdecor_frame_ref(*mut libdecor_frame) -> (),
        fn libdecor_frame_unref(*mut libdecor_frame) -> (),
        fn libdecor_frame_set_visibility(*mut libdecor_frame, bool) -> (),
        fn libdecor_frame_is_visible(*mut libdecor_frame) -> bool,
        fn libdecor_frame_set_parent(*mut libdecor_frame, *mut libdecor_frame) -> (),
        fn libdecor_frame_set_title(
            *mut libdecor_frame,
            *const c_char
        ) -> (),
        fn libdecor_frame_get_title(*mut libdecor_frame) -> *const c_char,
        fn libdecor_frame_set_app_id(
            *mut libdecor_frame,
            *const c_char
        ) -> (),
        fn libdecor_frame_set_capabilities(
            *mut libdecor_frame,
            libdecor_capabilities
        ) -> (),
        fn libdecor_frame_unset_capabilities(
            *mut libdecor_frame,
            libdecor_capabilities
        ) -> (),
        fn libdecor_frame_has_capability(
            *mut libdecor_frame,
            libdecor_capabilities
        ) -> bool,
        fn libdecor_frame_show_window_menu(
            *mut libdecor_frame,
            *mut wl_seat,
            u32,
            c_int,
            c_int
        ) -> (),
        fn libdecor_frame_popup_grab(
            *mut libdecor_frame,
            *const c_char
        ) -> (),
        fn libdecor_frame_popup_ungrab(
            *mut libdecor_frame,
            *const c_char
        ) -> (),
        fn libdecor_frame_translate_coordinate(
            *mut libdecor_frame,
            c_int,
            c_int,
            *mut c_int,
            *mut c_int
        ) -> (),
        fn libdecor_frame_set_max_content_size(
            *mut libdecor_frame,
            c_int,
            c_int
        ) -> (),
        fn libdecor_frame_set_min_content_size(
            *mut libdecor_frame,
            c_int,
            c_int
        ) -> (),
        fn libdecor_frame_resize(
            *mut libdecor_frame,
            *mut wl_seat,
            u32,
            libdecor_resize_edge
        ) -> (),
        fn libdecor_frame_move(*mut libdecor_frame, *mut wl_seat, u32) -> (),
        fn libdecor_frame_commit(
            *mut libdecor_frame,
            *mut libdecor_state,
            *mut libdecor_configuration
        ) -> (),
        fn libdecor_frame_set_minimized(*mut libdecor_frame) -> (),
        fn libdecor_frame_set_maximized(*mut libdecor_frame) -> (),
        fn libdecor_frame_unset_maximized(*mut libdecor_frame) -> (),
        fn libdecor_frame_set_fullscreen(*mut libdecor_frame, *mut wl_output) -> (),
        fn libdecor_frame_unset_fullscreen(*mut libdecor_frame) -> (),
        fn libdecor_frame_is_floating(*mut libdecor_frame) -> bool,
        fn libdecor_frame_close(*mut libdecor_frame) -> (),
        fn libdecor_frame_map(*mut libdecor_frame) -> (),
        fn libdecor_frame_get_xdg_surface(*mut libdecor_frame) -> *mut xdg_surface,
        fn libdecor_frame_get_xdg_toplevel(*mut libdecor_frame) -> *mut xdg_toplevel,
        fn libdecor_state_new(
            c_int,
            c_int
        ) -> *mut libdecor_state,
        fn libdecor_state_free(*mut libdecor_state) -> (),
        fn libdecor_configuration_get_content_size(
            *mut libdecor_configuration,
            *mut libdecor_frame,
            *mut c_int,
            *mut c_int
        ) -> bool,
        fn libdecor_configuration_get_window_state(
            *mut libdecor_configuration,
            *mut libdecor_window_state
        ) -> bool,
);

#[cfg(feature = "dlopen")]
lazy_static::lazy_static!(
    pub static ref LIBDECOR_OPTION: Option<Libdecor> = {
        let library_filename = libloading::library_filename("decor-0");
        match unsafe { Libdecor::open(library_filename.to_str().unwrap()) } {
            Ok(lib) => Some(lib),
            Err(err) => {
                eprintln!("Failed to load {}: {}", library_filename.to_str().unwrap(), err);
                None
            },
        }
    };
    pub static ref LIBDECOR_HANDLE: &'static Libdecor = {
        LIBDECOR_OPTION.as_ref().expect("Library decor-0 could not be loaded.")
    };
);

#[cfg(not(feature = "dlopen"))]
pub fn is_lib_available() -> bool {
    true
}
#[cfg(feature = "dlopen")]
pub fn is_lib_available() -> bool {
    LIBDECOR_OPTION.is_some()
}

#[cfg(feature = "dlopen")]
#[macro_export]
macro_rules! ffi_dispatch(
    ($handle: ident, $func: ident, $($arg: expr),*) => (
        ($handle.$func)($($arg),*)
    )
);

#[cfg(not(feature = "dlopen"))]
#[macro_export]
macro_rules! ffi_dispatch(
    ($handle: ident, $func: ident, $($arg: expr),*) => (
        $func($($arg),*)
    )
);
