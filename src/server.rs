use rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::time::Duration;

use crate::AnyGenCam;
#[allow(unused_imports)]
use crate::GenCam;
use crate::GenCamCtrl;
use crate::GenCamError;
use crate::GenCamRoi;
use crate::GenCamState;
use crate::Property;
use crate::PropertyValue;
use crate::Result;
use refimage::GenericImage;
use serde::{Deserialize, Serialize};

/// The result of a generic camera server call.
pub type GenCamOutput<'a> = Result<GenCamOk<'a>>;

/// The Ok variant of a generic camera server call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GenCamOk<'a> {
    /// No return value.
    Unit,
    /// A single [`PropertyValue`].
    Property(PropertyValue),
    #[serde(borrow)]
    /// A captured image from the camera.
    Image(GenericImage<'a>),
    /// Region of interest defined on the camera.
    Roi(GenCamRoi),
    /// The current state of the camera.
    State(GenCamState),
    /// A list of properties available on the camera.
    PropertyList(Vec<Property>),
}

impl<'a> From<()> for GenCamOk<'a> {
    fn from(_: ()) -> Self {
        GenCamOk::Unit
    }
}

impl<'a> From<PropertyValue> for GenCamOk<'a> {
    fn from(value: PropertyValue) -> Self {
        GenCamOk::Property(value)
    }
}

impl<'a> From<GenericImage<'a>> for GenCamOk<'a> {
    fn from(image: GenericImage<'a>) -> Self {
        GenCamOk::Image(image)
    }
}

impl<'a> From<GenCamRoi> for GenCamOk<'a> {
    fn from(roi: GenCamRoi) -> Self {
        GenCamOk::Roi(roi)
    }
}

impl<'a> From<GenCamState> for GenCamOk<'a> {
    fn from(state: GenCamState) -> Self {
        GenCamOk::State(state)
    }
}

impl<'a> From<Vec<Property>> for GenCamOk<'a> {
    fn from(properties: Vec<Property>) -> Self {
        GenCamOk::PropertyList(properties)
    }
}

/// The result of a generic camera server call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GenCamResult<'a> {
    #[serde(borrow)]
    /// A successful result.
    Ok(GenCamOk<'a>),
    /// An error occurred.
    Err(GenCamError),
}

/// The possible calls that can be made to a generic camera server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientCall {
    /// Get the vendor of the camera. Calls the [`GenCam::vendor`] method.
    Vendor,
    /// Check if the camera is ready. Calls the [`GenCam::camera_ready`] method.
    CameraReady,
    /// Get the name of the camera. Calls the [`GenCam::camera_name`] method.
    CameraName,
    /// List all properties available on the camera. Calls the [`GenCam::list_properties`] method.
    ListProperties,
    /// Get a specific property from the camera. Calls the [`GenCam::get_property`] method.
    GetProperty(GenCamCtrl),
    /// Set a specific property on the camera. Calls the [`GenCam::set_property`] method.
    SetProperty(GenCamCtrl, PropertyValue),
    /// Check if a property is set to auto. Calls the [`GenCam::get_property_auto`] method.
    CheckAuto(GenCamCtrl),
    /// Set a property to auto. Calls the [`GenCam::set_property_auto`] method.
    SetAuto(GenCamCtrl, bool),
    /// Cancel a capture in progress. Calls the [`GenCam::cancel_capture`] method.
    CancelCapture,
    /// Check if the camera is currently capturing. Calls the [`GenCam::is_capturing`] method.
    IsCapturing,
    /// Capture an image from the camera. Calls the [`GenCam::capture`] method.
    Capture,
    /// Start an exposure on the camera. Calls the [`GenCam::start_exposure`] method.
    StartExposure,
    /// Download an image from the camera. Calls the [`GenCam::download_image`] method.
    DownloadImage,
    /// Check if an image is ready to be downloaded. Calls the [`GenCam::image_ready`] method.
    ImageReady,
    /// Get the current state of the camera. Calls the [`GenCam::camera_state`] method.
    CameraState,
    /// Get the current exposure time. Calls the [`GenCam::get_exposure`] method.
    GetExposure,
    /// Set the exposure time. Calls the [`GenCam::set_exposure`] method.
    SetExposure(Duration),
    /// Set the region of interest on the camera. Calls the [`GenCam::set_roi`] method.
    SetRoi(GenCamRoi),
    /// Get the current region of interest. Calls the [`GenCam::get_roi`] method.
    GetRoi,
}

/// A generic camera server that can manage multiple cameras.
#[derive(Debug, Default)]
pub struct GenCamServer {
    cameras: HashMap<i32, AnyGenCam>,
}

impl GenCamServer {
    /// Add a camera to the server and return the camera's assigned ID.
    pub fn add_camera(&mut self, camera: AnyGenCam) -> i32 {
        let id = thread_rng().gen();
        self.cameras.insert(id, camera);
        id
    }

    /// Get a reference to a camera by its ID.
    pub fn get_camera(&self, id: i32) -> Option<&AnyGenCam> {
        self.cameras.get(&id)
    }

    /// Get a mutable reference to a camera by its ID.
    pub fn get_camera_mut(&mut self, id: i32) -> Option<&mut AnyGenCam> {
        self.cameras.get_mut(&id)
    }

    /// Remove a camera from the server by its ID.
    pub fn remove_camera(&mut self, id: i32) -> Option<AnyGenCam> {
        self.cameras.remove(&id)
    }

    /// Get the number of cameras currently connected to the server.
    pub fn num_cameras(&self) -> usize {
        self.cameras.len()
    }

    /// Execute a client call on a camera by its ID.
    pub fn execute_fn(&mut self, id: i32, sig: ClientCall) -> GenCamResult {
        let Some(camera) = self.get_camera_mut(id) else {
            return GenCamResult::Err(GenCamError::InvalidId(id));
        };
        use ClientCall::*;
        match sig {
            Vendor => {
                let vendor = camera.vendor();
                GenCamResult::Ok(GenCamOk::Property(PropertyValue::EnumStr(
                    vendor.to_string(),
                )))
            }
            CameraReady => {
                let ready = camera.camera_ready();
                GenCamResult::Ok(GenCamOk::Property(PropertyValue::Bool(ready)))
            }
            CameraName => {
                let name = camera.camera_name();
                GenCamResult::Ok(GenCamOk::Property(PropertyValue::EnumStr(name.to_string())))
            }
            ListProperties => {
                let properties = camera.list_properties();
                GenCamResult::Ok(GenCamOk::PropertyList(properties))
            }
            GetProperty(ctrl) => {
                let prop = camera.get_property(ctrl);
                match prop {
                    Some(p) => GenCamResult::Ok(GenCamOk::Property(p.clone())),
                    None => GenCamResult::Err(GenCamError::PropertyNotFound(format!("{:?}", ctrl))),
                }
            }
            SetProperty(ctrl, value) => {
                let result = camera.set_property(ctrl, &value);
                match result {
                    Ok(_) => GenCamResult::Ok(GenCamOk::Unit),
                    Err(e) => GenCamResult::Err(e),
                }
            }
            CheckAuto(ctrl) => {
                let auto = camera.get_property_auto(ctrl);
                match auto {
                    Ok(b) => GenCamResult::Ok(GenCamOk::Property(PropertyValue::Bool(b))),
                    Err(e) => GenCamResult::Err(e),
                }
            }
            SetAuto(ctrl, b) => {
                let result = camera.set_property_auto(ctrl, b);
                match result {
                    Ok(_) => GenCamResult::Ok(GenCamOk::Unit),
                    Err(e) => GenCamResult::Err(e),
                }
            }
            CancelCapture => {
                let result = camera.cancel_capture();
                match result {
                    Ok(_) => GenCamResult::Ok(GenCamOk::Unit),
                    Err(e) => GenCamResult::Err(e),
                }
            }
            IsCapturing => {
                let capturing = camera.is_capturing();
                GenCamResult::Ok(GenCamOk::Property(PropertyValue::Bool(capturing)))
            }
            Capture => {
                let result = camera.capture();
                match result {
                    Ok(image) => GenCamResult::Ok(GenCamOk::Image(image)),
                    Err(e) => GenCamResult::Err(e),
                }
            }
            StartExposure => {
                let result = camera.start_exposure();
                match result {
                    Ok(_) => GenCamResult::Ok(GenCamOk::Unit),
                    Err(e) => GenCamResult::Err(e),
                }
            }
            DownloadImage => {
                let result = camera.download_image();
                match result {
                    Ok(image) => GenCamResult::Ok(GenCamOk::Image(image)),
                    Err(e) => GenCamResult::Err(e),
                }
            }
            ImageReady => {
                let ready = camera.image_ready();
                match ready {
                    Ok(b) => GenCamResult::Ok(GenCamOk::Property(PropertyValue::Bool(b))),
                    Err(e) => GenCamResult::Err(e),
                }
            }
            CameraState => {
                let state = camera.camera_state();
                match state {
                    Ok(s) => GenCamResult::Ok(GenCamOk::State(s)),
                    Err(e) => GenCamResult::Err(e),
                }
            }
            GetExposure => {
                let exposure = camera.get_exposure();
                match exposure {
                    Ok(e) => GenCamResult::Ok(GenCamOk::Property(PropertyValue::Duration(e))),
                    Err(e) => GenCamResult::Err(e),
                }
            }
            SetExposure(e) => {
                let result = camera.set_exposure(e);
                match result {
                    Ok(e) => GenCamResult::Ok(GenCamOk::Property(PropertyValue::Duration(e))),
                    Err(e) => GenCamResult::Err(e),
                }
            }
            SetRoi(roi) => {
                let result = camera.set_roi(&roi);
                match result {
                    Ok(r) => GenCamResult::Ok(GenCamOk::Roi(*r)),
                    Err(e) => GenCamResult::Err(e),
                }
            }
            GetRoi => {
                let roi = camera.get_roi();
                GenCamResult::Ok(GenCamOk::Roi(*roi))
            }
        }
    }
}
