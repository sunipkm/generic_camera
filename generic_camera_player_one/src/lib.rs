//! [`generic_camera`] implementation for the [Player One Astronomy Camera Driver](https://player-one-astronomy.com/service/software/).
#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
mod raw;
pub(crate) mod util;
use player_one_camera_sys::{self as poa};
use std::{
    sync::{MutexGuard, PoisonError},
    time::Duration,
};

use generic_camera::{
    GenCam, GenCamCtrl, GenCamDescriptor, GenCamDriver, GenCamError, GenCamInfo, GenCamRoi,
    GenCamState, PollExposure, PropertyValue,
};
pub use player_one_camera_sys::Id;
use raw::driver::Driver as RawDriver;

use crate::raw::{
    error::{CameraError, PropertyError},
    handle::{CaptureState, Handle, HandleInner},
};

/// An player one astronomy camera.
/// When cloned, it makes another handle to the same camera
/// except with a different image buffer
#[derive(Debug, Clone)]
pub struct Camera {
    handle: Handle,
    roi: GenCamRoi,
    image_data: Vec<u8>,
    name: String,
    desc: GenCamDescriptor,
}
impl Camera {
    fn inner(&self) -> MutexGuard<'_, HandleInner> {
        self.handle
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

fn get_state(handle: &Handle) -> generic_camera::GenCamResult<generic_camera::GenCamState> {
    Ok(
        match handle
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .capture_state()
        {
            Ok(CaptureState::Capturing(time)) => GenCamState::Exposing(time.map(|x| x.elapsed())),
            Ok(CaptureState::Idle) => GenCamState::Idle,
            Ok(CaptureState::Ready) => GenCamState::ExposureFinished,
            Err(e) => return Err(poa2gencam(e)),
        },
    )
}
fn poa2gencam(error: poa::Error) -> GenCamError {
    match error {
        poa::Error::Failed => GenCamError::GeneralError("Operation failed".into()),
        poa::Error::Exposing => GenCamError::ExposureInProgress,
        poa::Error::ExposureFailed => GenCamError::ExposureFailed("unknown reason".into()),
        poa::Error::AccessDenied => GenCamError::AccessViolation,
        poa::Error::DeviceNotFound | poa::Error::InvalidId => GenCamError::CameraRemoved,
        e @ poa::Error::OutOfLimit => GenCamError::OutOfBounds(e.message().into()),
        e => GenCamError::Message(e.message().into()),
    }
}
fn cameraerror2gencam(err: CameraError) -> GenCamError {
    match err {
        CameraError::Internal(e) => poa2gencam(e),
        e @ CameraError::NotReady => GenCamError::GeneralError(e.to_string()),
        e @ CameraError::UnknownCameraState(_) => GenCamError::GeneralError(e.to_string()),
        e @ CameraError::UnknownImageFormat => GenCamError::InvalidImageType(e.to_string()),
    }
}
fn propertyerror2gencam(
    err: PropertyError,
    prop: GenCamCtrl,
    value: Option<&PropertyValue>,
) -> GenCamError {
    match err {
        PropertyError::Exposing => GenCamError::ExposureInProgress,
        PropertyError::Unsupported => GenCamError::PropertyError {
            control: prop,
            error: generic_camera::PropertyError::NotNumber,
        },
        e @ PropertyError::ValueOutOfRange => {
            if let Some(value) = value {
                GenCamError::PropertyError {
                    control: prop,
                    error: generic_camera::PropertyError::ValueOutOfRange {
                        min: PropertyValue::EnumStr("insert min here".to_owned()),
                        max: PropertyValue::EnumStr("insert max here".to_owned()),
                        value: value.clone(),
                    },
                }
            } else {
                GenCamError::GeneralError(e.to_string())
            }
        }
        e => GenCamError::GeneralError(e.to_string()),
    }
}
impl GenCam for Camera {
    fn camera_name(&self) -> &str {
        &self.name
    }
    fn camera_ready(&self) -> bool {
        true
    }
    fn camera_state(&self) -> generic_camera::GenCamResult<generic_camera::GenCamState> {
        get_state(&self.handle)
    }
    fn cancel_capture(&self) -> generic_camera::GenCamResult<()> {
        self.inner().stop_exposure().map_err(cameraerror2gencam)
    }
    fn get_property(
        &self,
        name: generic_camera::GenCamCtrl,
    ) -> generic_camera::GenCamResult<(generic_camera::PropertyValue, bool)> {
        self.inner()
            .get_property(name)
            .map_err(|e| propertyerror2gencam(e, name, None))
    }
    fn set_property(
        &mut self,
        name: GenCamCtrl,
        value: &PropertyValue,
    ) -> generic_camera::GenCamResult<()> {
        self.inner()
            .set_property(name, value, false)
            .map_err(|e| propertyerror2gencam(e, name, Some(value)))
    }
    fn set_property_auto(
        &mut self,
        name: GenCamCtrl,
        value: &PropertyValue,
    ) -> generic_camera::GenCamResult<()> {
        self.inner()
            .set_property(name, value, true)
            .map_err(|e| propertyerror2gencam(e, name, Some(value)))
    }
    fn get_roi(&self) -> &GenCamRoi {
        &self.roi
    }
    fn set_roi(&mut self, roi: &GenCamRoi) -> generic_camera::GenCamResult<&GenCamRoi> {
        self.handle
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .set_roi(*roi, &mut self.roi)
            .map_err(cameraerror2gencam)?;
        Ok(&self.roi)
    }
    fn info(&self) -> generic_camera::GenCamResult<&generic_camera::GenCamDescriptor> {
        Ok(&self.desc)
    }
    fn info_handle(&self) -> Option<generic_camera::AnyGenCamInfo> {
        Some(Box::new(Info {
            handle: self.handle.clone(),
            name: self.name.clone(),
        }))
    }
    fn is_capturing(&self) -> bool {
        self.inner()
            .capture_state()
            .map(|e| matches!(e, CaptureState::Capturing(_)))
            .unwrap_or_default()
    }
    fn list_properties(&self) -> &std::collections::HashMap<GenCamCtrl, generic_camera::Property> {
        &self.handle.gencam_props
    }
    fn start_exposure(&mut self) -> generic_camera::GenCamResult<()> {
        self.inner().start_exposure().map_err(cameraerror2gencam)?;
        Ok(())
    }
    fn vendor(&self) -> &str {
        "POA"
    }
    fn poll_exposure(&mut self) -> generic_camera::PollExposure<'_> {
        let mut inner_guard = self
            .handle
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // inner_guard.camera
        match inner_guard.capture_state().copied().map_err(poa2gencam) {
            Ok(CaptureState::Idle) => PollExposure::Ready(Err(GenCamError::ExposureNotStarted)),
            Ok(CaptureState::Ready) => match inner_guard.download(&mut self.image_data, self.roi) {
                Ok(img) => PollExposure::Ready(Ok(img)),
                Err(e) => PollExposure::Ready(Err(cameraerror2gencam(e))),
            },
            Ok(CaptureState::Capturing(started)) => {
                if let Some(started) = started {
                    let exposure_time = inner_guard
                        .exposure_time()
                        .unwrap_or(Duration::from_millis(100));
                    let elapsed = started.elapsed();
                    if elapsed > exposure_time {
                        PollExposure::Wait(Duration::from_millis(20))
                    } else {
                        PollExposure::Wait(exposure_time - elapsed)
                    }
                } else {
                    // this happens if we are initialized in the capturing state.
                    // No sane user would let this happen, but let's just say we wait 100 millis before trying again
                    PollExposure::Wait(Duration::from_millis(100))
                }
            }
            Err(e) => PollExposure::Ready(Err(e)),
        }
    }
}

/// Info for a [`Camera`]
#[derive(Debug, Clone)]
pub struct Info {
    handle: Handle,
    name: String,
}
impl Info {
    fn inner(&self) -> MutexGuard<'_, HandleInner> {
        self.handle
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}
impl GenCamInfo for Info {
    fn camera_name(&self) -> &str {
        &self.name
    }
    fn camera_ready(&self) -> bool {
        true
    }
    fn camera_state(&self) -> generic_camera::GenCamResult<GenCamState> {
        get_state(&self.handle)
    }
    fn cancel_capture(&self) -> generic_camera::GenCamResult<()> {
        self.inner().stop_exposure().map_err(cameraerror2gencam)
    }
    fn is_capturing(&self) -> bool {
        self.inner()
            .capture_state()
            .map(|e| matches!(e, CaptureState::Capturing(_)))
            .unwrap_or_default()
    }
    fn get_property(
        &self,
        name: generic_camera::GenCamCtrl,
    ) -> generic_camera::GenCamResult<(generic_camera::PropertyValue, bool)> {
        self.inner()
            .get_property(name)
            .map_err(|e| propertyerror2gencam(e, name, None))
    }
    fn set_property(
        &mut self,
        name: GenCamCtrl,
        value: &PropertyValue,
    ) -> generic_camera::GenCamResult<()> {
        self.inner()
            .set_property(name, value, false)
            .map_err(|e| propertyerror2gencam(e, name, Some(value)))
    }
    fn set_property_auto(
        &mut self,
        name: GenCamCtrl,
        value: &PropertyValue,
    ) -> generic_camera::GenCamResult<()> {
        self.inner()
            .set_property(name, value, true)
            .map_err(|e| propertyerror2gencam(e, name, Some(value)))
    }
    fn list_properties(&self) -> &std::collections::HashMap<GenCamCtrl, generic_camera::Property> {
        &self.handle.gencam_props
    }
}

/// A proxy for the POA camera driver's global state.
pub struct Driver {
    inner: RawDriver,
}

impl Driver {
    /// Creates a new [`Driver`] proxy
    ///
    /// # Safety
    /// Since the [`Driver`] is just a proxy for global state, you must ensure the following:
    ///
    /// - [`Driver`] methods are not called concurrently from multiple threads, even by different
    ///   [`Driver`] instances
    ///
    /// - If you obtain multiple owned handles to a camera with the same ID, even across different [`Driver`] instances,
    ///   you must not perform *ANY* operation on the camera without synchronization including `Drop`ping the camera.
    ///
    /// - You must ensure that none of the [`Driver`] methods are called concurrently with any write to
    ///   the custom user ID of any camera (don't ask why)
    ///
    /// - All of the typical conditions of global state apply
    ///
    /// Essentially, you must uphold the soundness conditions of the `Send` implementation for the [`GenCam`]
    /// since there is no way to ensure uniqueness at the driver level.
    ///
    /// The intended usage pattern ensures all of these conditions automatically. You should basically
    /// only ever have a single [`Driver`] instance per program or at the very least only create a
    /// new [`Driver`] after all cameras closed. You should generally not need to
    pub unsafe fn new() -> Self {
        Self {
            inner: unsafe { RawDriver::new() },
        }
    }
}

impl GenCamDriver for Driver {
    fn available_devices(&self) -> usize {
        self.inner.num_devices()
    }
    fn connect_device(
        &mut self,
        descriptor: &GenCamDescriptor,
    ) -> generic_camera::GenCamResult<generic_camera::AnyGenCam> {
        let (handle, props) = self.inner.connect(descriptor).map_err(cameraerror2gencam)?;
        let name = props.model_name.to_str_lossy().into_owned();
        let desc = descriptor.clone();
        let roi = handle
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_roi()
            .map_err(cameraerror2gencam)?;
        let cam = Camera {
            handle,
            roi,
            // this will get resized automatically
            image_data: vec![],
            name,
            desc,
        };
        Ok(Box::new(cam))
    }
    fn list_devices(&mut self) -> generic_camera::GenCamResult<Vec<GenCamDescriptor>> {
        Ok(self.inner.list_devices().collect())
    }
    fn connect_first_device(&mut self) -> generic_camera::GenCamResult<generic_camera::AnyGenCam> {
        let Some(descriptor) = self.inner.list_devices().next() else {
            return Err(GenCamError::NoCamerasAvailable);
        };
        self.connect_device(&descriptor)
    }
}
