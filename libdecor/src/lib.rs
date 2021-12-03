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
