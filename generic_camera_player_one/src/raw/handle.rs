use std::{
    marker::{PhantomData, PhantomPinned},
    mem::MaybeUninit,
    ops::Deref,
};

use player_one_camera_sys::{
    self as poa, Camera, CameraProperties, CameraState, Id, PoaResult, close_camera,
    ffi_util::{MaybeInvalid, ValidationError},
    get_camera_count, get_state,
};

use crate::util::poa_call;
error_set::error_set! {
    Error := {
        UnknownCameraState(ValidationError<CameraState>),
        (poa::Error)
    }
}
/// An owned handle to an open and initialized camera device
/// When this camera goes out of scope, it is closed.
struct OwnedCamera {
    id: Id<Camera>,
}
impl OwnedCamera {
    /// Opens and initializes a camera
    ///
    /// # Safety
    /// - The camera with this id must not be open or initialized
    /// - This camera must be the sole owner of the camera id.
    ///   When this camera goes out of scope, it is closed.
    /// - While this [`OwnedCamera`] is alive, it must not be closed during any method calls
    /// - The id must not be shared between threads without synchronization
    pub unsafe fn new(id: Id<Camera>) -> Result<Self, Error> {
        unsafe {
            poa::open_camera(id).into_result()?;
            poa::init_camera(id).into_result()?;
        };
        Ok(Self { id })
    }

    fn get_state(&mut self) -> Result<CameraState, Error> {
        // SAFETY: We have exclusive access to the camera
        let res = unsafe {
            poa_call!(
                poa::get_state(self.id) @ out
            )?
        };
        Ok(res.get()?)
    }
}

impl Drop for OwnedCamera {
    fn drop(&mut self) {
        unsafe {
            match self.get_state() {
                Ok(CameraState::Exposing) => {
                    _ = poa::stop_exposure(self.id);
                    _ = poa::close_camera(self.id);
                }
                Ok(CameraState::Opened) => _ = poa::close_camera(self.id),
                // this should not happen, but might as well handle it
                Ok(CameraState::Closed) | Err(_) => {}
            }
        }
        // close_camera(self.id)
    }
}
