//! # Overview
//!
//! Wrapper for [libdecor - A client-side decorations library for Wayland client](https://gitlab.gnome.org/jadahl/libdecor)
//!
//! > libdecor is a library that can help Wayland clients draw window
//! > decorations for them. It aims to provide multiple backends that implements the
//! > decoration drawing.
//!
//! # Usage
//! 
//! ## Create a context
//! 
//! ```
//! # use libdecor::{Context, State, Request, FrameRequest};
//! # use wayland_client::Display;
//! #
//! # let display = Display::connect_to_env().unwrap();
//! # let mut event_queue = display.create_event_queue();
//! # let attached_display = (*display).clone().attach(event_queue.token());
//! # event_queue
//! #     .sync_roundtrip(&mut (), |_, _, _| unreachable!())
//! #     .unwrap();
//! # 
//! let context = Context::new(display, |request| match request {
//!     Request::Error(error) => {
//!         panic!("libdecor error: {}", error);
//!     }
//!     _ => unreachable!(),
//! });
//! # 
//! # context.dispatch(&mut (), None);
//! ```
//!
//! ## Decorate a surface
//! 
//! ```
//! # use libdecor::{Context, State, Request, FrameRequest};
//! # use wayland_client::{
//! #     protocol::wl_compositor,
//! #     Display, GlobalManager,
//! # };
//! #
//! # let display = Display::connect_to_env().unwrap();
//! # let mut event_queue = display.create_event_queue();
//! # let attached_display = (*display).clone().attach(event_queue.token());
//! # let globals = GlobalManager::new(&attached_display);
//! # event_queue
//! #     .sync_roundtrip(&mut (), |_, _, _| unreachable!())
//! #     .unwrap();
//! # 
//! # let compositor = globals
//! #     .instantiate_exact::<wl_compositor::WlCompositor>(4)
//! #     .expect("Failed to instantiate wl_compositor");
//! # 
//! # let content_surface = compositor.create_surface();
//! # content_surface.quick_assign(|_, _, _| {});
//! #
//! # let context = Context::new(display, |request| match request {
//! #     Request::Error(error) => {
//! #         panic!("libdecor error: {}", error);
//! #     }
//! #     _ => unreachable!(),
//! # });
//! # 
//! let frame = context
//!     .decorate(content_surface.detach(), move |frame, request, _| {
//!         match request {
//!             FrameRequest::Configure(configuration) => {
//!                 let content_surface_size = configuration
//!                     .content_size(&frame)
//!                     .unwrap_or((800, 600));
//!
//!                 let state = State::new(content_surface_size.0, content_surface_size.1);
//!                 frame.commit(&state, Some(configuration));
//!
//!                 // Draw surface content
//!                 # content_surface.commit();
//!             }
//!             FrameRequest::Close => { }
//!             FrameRequest::Commit => {
//!                 content_surface.commit();
//!             }
//!             FrameRequest::DismissPopup { .. } => { }
//!             _ => unreachable!(),
//!         }
//!     })
//!     .expect("Failed to create frame");
//!
//! frame.dispatch(&mut (), |f| {
//!     f.set_app_id("libdecor-rs-example");
//!     f.set_title("libdecor-rs example");
//!     f.map();
//! });
//! #
//! # context.dispatch(&mut (), None);
//! # event_queue
//! #     .dispatch(&mut (), |_, _, _| unreachable!())
//! #     .unwrap();
//! ```
//! 
//! # Example
//! 
//! For a more complete example see [demo.rs](https://github.com/cmeissl/libdecor-rs/blob/main/libdecor/examples/demo.rs)
//!
//! ```
//! use std::{
//!     time::Duration,
//!     io::{BufWriter, Write},
//!     os::unix::prelude::{AsRawFd, FromRawFd}
//! };
//! use libdecor::{Context, State, Request, FrameRequest};
//! use wayland_client::{
//!     protocol::{wl_compositor, wl_shm, wl_buffer},
//!     Display, GlobalManager,
//! };
//!
//! let display = Display::connect_to_env().unwrap();
//! let mut event_queue = display.create_event_queue();
//! let attached_display = (*display).clone().attach(event_queue.token());
//! let globals = GlobalManager::new(&attached_display);
//! event_queue
//!     .sync_roundtrip(&mut (), |_, _, _| unreachable!())
//!     .unwrap();
//!
//! let compositor = globals
//!     .instantiate_exact::<wl_compositor::WlCompositor>(4)
//!     .expect("Failed to instantiate wl_compositor");
//!
//! let content_surface = compositor.create_surface();
//! content_surface.quick_assign(|_, _, _| {});
//!
//! let shm = globals.instantiate_exact::<wl_shm::WlShm>(1)
//!     .expect("Failed to instantiate wl_shm");
//! shm.quick_assign(|_, _, _| {});
//!
//! let context = Context::new(display, |request| match request {
//!     Request::Error(error) => {
//!         panic!("libdecor error: {}", error);
//!     }
//!     _ => unreachable!(),
//! });
//!
//! let frame = context
//!     .decorate(content_surface.detach(), move |frame, request, _| {
//!         match request {
//!             FrameRequest::Configure(configuration) => {
//!                 let content_surface_size = configuration
//!                     .content_size(&frame)
//!                     .unwrap_or((800, 600));
//!
//!                 let state = State::new(content_surface_size.0, content_surface_size.1);
//!                 frame.commit(&state, Some(configuration));
//!
//!                 // Draw surface content
//!                 let mut tmp = tempfile::tempfile().expect("Unable to create a tempfile.");
//!                 {
//!                     let mut buf = BufWriter::new(&mut tmp);
//!                     for _ in 0..(content_surface_size.0 * content_surface_size.1) { buf.write_all(&0xff4455ffu32.to_ne_bytes()).unwrap(); }
//!                     buf.flush().unwrap();
//!                 }
//!                 let pool = shm.create_pool(
//!                     tmp.as_raw_fd(),
//!                     (content_surface_size.0 * content_surface_size.1 * 4) as i32);
//!                 let buffer = pool.create_buffer(
//!                     0,
//!                     content_surface_size.0 as i32,
//!                     content_surface_size.1 as i32,
//!                     (content_surface_size.0 * 4) as i32,
//!                     wl_shm::Format::Argb8888,
//!                 );
//!                 buffer.quick_assign(|_, request, _| match request {
//!                     wl_buffer::Event::Release => {}
//!                     _ => unreachable!(),
//!                 });
//!
//!                 content_surface.attach(Some(&buffer), 0, 0);
//!                 content_surface.set_buffer_scale(1);
//!                 content_surface.damage_buffer(0, 0, content_surface_size.0, content_surface_size.1);
//!                 content_surface.commit();
//!             }
//!             FrameRequest::Close => {
//!                 std::process::exit(0);
//!             }
//!             FrameRequest::Commit => {
//!                 content_surface.commit();
//!             }
//!             FrameRequest::DismissPopup { .. } => {
//!                 // ungrab and close popups
//!             }
//!             _ => unreachable!(),
//!         }
//!     })
//!     .expect("Failed to create frame");
//!
//! frame.dispatch(&mut (), |f| {
//!     f.set_app_id("libdecor-rs-example");
//!     f.set_title("libdecor-rs example");
//!     f.map();
//! });
//!
//! while context.dispatch(&mut (), Some(Duration::from_millis(16))) {
//!     event_queue
//!         .dispatch(&mut (), |_, _, _| unreachable!())
//!         .unwrap();
//! }
//! ```
//!

#![warn(missing_docs, missing_debug_implementations)]

pub use libdecor_sys as ffi;

mod context;
mod frame;

pub use context::*;
pub use frame::*;
use wayland_client::DispatchData;

scoped_tls::scoped_thread_local!(pub(crate) static DISPATCH_METADATA: DispatchDataMut);

struct DispatchDataMut<'a> {
    ddata: *mut DispatchData<'a>,
}

impl<'a> DispatchDataMut<'a> {
    fn new(ddata: DispatchData<'a>) -> Self {
        Self {
            ddata: Box::into_raw(Box::new(ddata)),
        }
    }

    #[allow(clippy::mut_from_ref)]
    fn get(&self) -> &mut DispatchData<'a> {
        unsafe { &mut *(self.ddata) }
    }
}

impl<'a> Drop for DispatchDataMut<'a> {
    fn drop(&mut self) {
        unsafe {
            let _ = Box::from_raw(self.ddata);
        }
    }
}

/// Check if the `libdecor` library is available on the system.
/// 
/// This can be used in combination with the `dlopen` feature
/// to check at runtime if the library is available.
/// 
/// Always returns [`true`] if the feature `dlopen` is not enabled.
pub fn is_lib_available() -> bool {
    ffi::is_lib_available()
}