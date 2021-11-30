use std::{
    cell::RefCell,
    ffi::CStr,
    os::{
        raw::{c_char, c_int},
        unix::prelude::RawFd,
    },
    rc::Rc,
    time::Duration,
};
use wayland_client::{protocol::wl_surface::WlSurface, Display};

use crate::{frame::LIBDECOR_FRAME_INTERFACE, FrameCallback, FrameRef};
use libdecor_sys::*;

use crate::{Frame, FrameRequest};

/// Kind of error in a [`Context`]
#[derive(Debug, Clone, Copy)]
pub enum ErrorKind {
    /// The compositor is incompatible with [`libdecor`]
    ///
    /// The most likely reason is that `xdg_wm_base` is missing
    CompositorIncompatible,
    /// The frame configuration is invalid
    ///
    /// E.g.: The min_size is greater as the max_size
    InvalidFrameConfiguration,
}

/// An error that has occurred in a [`Context`]
#[derive(Debug, Clone)]
pub struct Error {
    message: String,
    kind: ErrorKind,
}

impl Error {
    /// Returns the corresponding [`ErrorKind`] for this error.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<libdecor_error> for ErrorKind {
    fn from(error: libdecor_error) -> Self {
        match error {
            LIBDECOR_ERROR_COMPOSITOR_INCOMPATIBLE => Self::CompositorIncompatible,
            LIBDECOR_ERROR_INVALID_FRAME_CONFIGURATION => Self::InvalidFrameConfiguration,
            _ => unreachable!(),
        }
    }
}

extern "C" fn error_callback_trampolin(
    context: *mut libdecor,
    error: libdecor_error,
    message: *const c_char,
) {
    let message = unsafe { CStr::from_ptr(message) };

    let error = Error {
        message: message.to_string_lossy().to_string(),
        kind: ErrorKind::from(error),
    };

    LIBDECOR_CALLBACK_REGISTRATIONS.with(|c| {
        let mut registrations = c.borrow_mut();
        let registration = registrations.iter_mut().find(|r| r.context == context);

        if let Some(registration) = registration {
            (*registration.cb)(Request::Error(error));
        }
    });
}

static LIBDECOR_INTERFACE: libdecor_interface = libdecor_interface {
    error: error_callback_trampolin,
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

struct ContextCallbackRegistration {
    context: *mut libdecor,
    cb: Box<dyn FnMut(Request)>,
}

thread_local! {
    static LIBDECOR_CALLBACK_REGISTRATIONS: RefCell<Vec<ContextCallbackRegistration>> = RefCell::new(Vec::new());
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Request {
    /// An error event
    Error(Error),
}

#[derive(Debug)]
struct InnerContext(*mut libdecor);

impl InnerContext {
    fn new<C>(display: Display, cb: C) -> Self
    where
        C: FnMut(Request) + 'static,
    {
        let context = unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_new,
                display.get_display_ptr() as *mut _,
                &LIBDECOR_INTERFACE as *const _ as *mut _
            )
        };

        let callback_registration = ContextCallbackRegistration {
            context,
            cb: Box::new(cb),
        };

        LIBDECOR_CALLBACK_REGISTRATIONS.with(|c| {
            let mut registrations = c.borrow_mut();
            registrations.push(callback_registration);
        });

        Self(context)
    }
}

impl Drop for InnerContext {
    fn drop(&mut self) {
        unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_unref, self.0) }

        LIBDECOR_CALLBACK_REGISTRATIONS.with(|c| {
            let mut registrations = c.borrow_mut();
            registrations.retain(|r| r.context != self.0);
        });
    }
}

/// A libdecor context instance.
#[derive(Debug, Clone)]
pub struct Context {
    inner: Rc<InnerContext>,
}

impl Context {
    /// Create a new libdecor context for the given [`Display`].
    pub fn new<C>(display: Display, cb: C) -> Self
    where
        C: FnMut(Request) + 'static,
    {
        Self {
            inner: Rc::new(InnerContext::new(display, cb)),
        }
    }

    /// Decorate the given content [`WlSurface`].
    ///
    /// This will create an [`wayland_protocols::xdg_shell::client::xdg_surface::XdgSurface`]
    /// and an [`wayland_protocols::xdg_shell::client::xdg_toplevel::XdgToplevel`], and integrate it
    /// properly with the windowing system, including creating appropriate
    /// decorations when needed, as well as handle windowing integration events such
    /// as resizing, moving, maximizing, etc.
    ///
    /// The passed [`WlSurface`] should only contain actual application content,
    /// without any window decoration.
    pub fn decorate<C>(&self, surface: WlSurface, cb: C) -> Option<Frame>
    where
        C: FnMut(FrameRef, FrameRequest) + 'static,
    {
        let cb: Box<Box<FrameCallback>> = Box::new(Box::new(cb));
        let cb = Box::into_raw(cb);

        let frame = unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_decorate,
                self.inner.0,
                surface.as_ref().c_ptr() as *mut _,
                &LIBDECOR_FRAME_INTERFACE as *const _ as *mut _,
                cb as *mut _
            )
        };

        if frame.is_null() {
            None
        } else {
            Some(Frame {
                frame_ref: FrameRef(frame),
                cb,
                context: self.clone(),
            })
        }
    }

    /// Get the file descriptor used by libdecor. This is similar to
    /// wl_display_get_fd(), thus should be polled, and when data is available,
    /// [`dispatch`](#method.dispatch) should be called.
    pub fn fd(&self) -> RawFd {
        unsafe { ffi_dispatch!(LIBDECOR_HANDLE, libdecor_get_fd, self.inner.0) }
    }

    /// Dispatch events. This function should be called when data is available on
    /// the file descriptor returned by [`fd`](#method.fd). If timeout is [`None`], this
    /// function will never block.
    pub fn dispatch(&self, timeout: Option<Duration>) -> bool {
        let result = unsafe {
            ffi_dispatch!(
                LIBDECOR_HANDLE,
                libdecor_dispatch,
                self.inner.0,
                timeout.map(|t| t.as_millis() as c_int).unwrap_or(-1)
            )
        };

        result >= 0
    }
}
