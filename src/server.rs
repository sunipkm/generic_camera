/*!
 * # Generic Camera Server
 * This module contains the implementation of a generic camera server that can manage multiple cameras.
 */
use rand::{thread_rng, Rng};
use refimage::GenericImageOwned;
use std::collections::HashMap;

use crate::AnyGenCam;
#[allow(unused_imports)]
use crate::GenCam;
use crate::GenCamCtrl;
use crate::GenCamDescriptor;
use crate::GenCamError;
use crate::GenCamResult;
use crate::GenCamRoi;
use crate::GenCamState;
use crate::Property;
use crate::PropertyValue;
use serde::{Deserialize, Serialize};

/// The result of a generic camera server call.
pub type GenSrvOutput = GenCamResult<GenSrvValue>;

/// The Ok variant of a generic camera server call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GenSrvValue {
    /// No return value.
    Unit,
    /// Camera information in a [`GenCamDescriptor`].
    Info(GenCamDescriptor),
    /// A single [`PropertyValue`].
    Property {
        /// The value of the property.
        value: PropertyValue,
        /// The auto setting of the property, if applicable.
        auto: Option<bool>,
    },
    /// A captured image from the camera.
    Image(GenericImageOwned),
    /// Region of interest defined on the camera.
    Roi(GenCamRoi),
    /// The current state of the camera.
    State(GenCamState),
    /// A list of properties available on the camera.
    PropertyList(HashMap<GenCamCtrl, Property>),
}

impl From<()> for GenSrvValue {
    fn from(_: ()) -> Self {
        GenSrvValue::Unit
    }
}

impl From<&GenCamDescriptor> for GenSrvValue {
    fn from(info: &GenCamDescriptor) -> Self {
        GenSrvValue::Info(info.clone())
    }
}

impl From<GenCamDescriptor> for GenSrvValue {
    fn from(info: GenCamDescriptor) -> Self {
        GenSrvValue::Info(info)
    }
}

impl From<(PropertyValue, bool)> for GenSrvValue {
    fn from(value: (PropertyValue, bool)) -> Self {
        let (value, auto) = value;
        GenSrvValue::Property {
            value,
            auto: Some(auto),
        }
    }
}

impl From<(&PropertyValue, bool)> for GenSrvValue {
    fn from(value: (&PropertyValue, bool)) -> Self {
        let (value, auto) = value;
        GenSrvValue::Property {
            value: value.clone(),
            auto: Some(auto),
        }
    }
}

impl From<PropertyValue> for GenSrvValue {
    fn from(value: PropertyValue) -> Self {
        GenSrvValue::Property { value, auto: None }
    }
}

impl From<GenericImageOwned> for GenSrvValue {
    fn from(image: GenericImageOwned) -> Self {
        GenSrvValue::Image(image)
    }
}

impl From<GenCamRoi> for GenSrvValue {
    fn from(roi: GenCamRoi) -> Self {
        GenSrvValue::Roi(roi)
    }
}

impl From<GenCamState> for GenSrvValue {
    fn from(state: GenCamState) -> Self {
        GenSrvValue::State(state)
    }
}

impl From<HashMap<GenCamCtrl, Property>> for GenSrvValue {
    fn from(properties: HashMap<GenCamCtrl, Property>) -> Self {
        GenSrvValue::PropertyList(properties)
    }
}

/// The possible calls that can be made to a generic camera server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GenSrvCmd {
    /// Get the vendor of the camera. Calls the [`GenCam::vendor`] method.
    Vendor,
    /// Check if the camera is ready. Calls the [`GenCam::camera_ready`] method.
    CameraReady,
    /// Get the name of the camera. Calls the [`GenCam::camera_name`] method.
    CameraName,
    /// Get the camera info. Calls the [`GenCam::info`] method.
    Info,
    /// List all properties available on the camera. Calls the [`GenCam::list_properties`] method.
    ListProperties,
    /// Get a specific property from the camera. Calls the [`GenCam::get_property`] method.
    GetProperty(GenCamCtrl),
    /// Set a specific property on the camera. Calls the [`GenCam::set_property`] method.
    SetProperty(GenCamCtrl, PropertyValue, bool),
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
    /// Set the region of interest on the camera. Calls the [`GenCam::set_roi`] method.
    SetRoi(GenCamRoi),
    /// Get the current region of interest. Calls the [`GenCam::get_roi`] method.
    GetRoi,
}

/// A generic camera server that can manage multiple cameras.
///
/// Once a camera is added to the server, it can be accessed by its assigned ID.
///
/// # Examples
/// ```rust,ignore
/// use generic_camera::server::GenCamServer;
/// use generic_camera::GenCam;
///
/// let mut server = GenCamServer::default();
/// let id = server.add_camera(...);
/// ```
#[derive(Debug, Default)]
pub struct GenCamServer {
    cameras: HashMap<u32, AnyGenCam>,
}

impl GenCamServer {
    /// Add a camera to the server and return the camera's assigned ID.
    pub fn add_camera(&mut self, camera: AnyGenCam) -> u32 {
        let id = thread_rng().gen();
        self.cameras.insert(id, camera);
        id
    }

    /// Get a reference to a camera by its ID.
    pub fn get_camera(&self, id: u32) -> Option<&AnyGenCam> {
        self.cameras.get(&id)
    }

    /// Get a mutable reference to a camera by its ID.
    pub fn get_camera_mut(&mut self, id: u32) -> Option<&mut AnyGenCam> {
        self.cameras.get_mut(&id)
    }

    /// Remove a camera from the server by its ID.
    pub fn remove_camera(&mut self, id: u32) -> Option<AnyGenCam> {
        self.cameras.remove(&id)
    }

    /// Get the number of cameras currently connected to the server.
    pub fn num_cameras(&self) -> usize {
        self.cameras.len()
    }

    /// Execute a client call on a camera by its ID.
    pub fn execute_fn(&mut self, id: u32, sig: GenSrvCmd) -> GenCamResult<GenSrvValue> {
        let Some(camera) = self.get_camera_mut(id) else {
            return Err(GenCamError::InvalidId(id as _));
        };
        use GenSrvCmd::*;
        let res = match sig {
            Vendor => {
                let vendor = camera.vendor();
                PropertyValue::EnumStr(vendor.to_string()).into()
            }
            CameraReady => {
                let ready = camera.camera_ready();
                PropertyValue::Bool(ready).into()
            }
            CameraName => {
                let name = camera.camera_name();
                PropertyValue::EnumStr(name.to_string()).into()
            }
            Info => {
                let info = camera.info()?.clone();
                info.into()
            }
            ListProperties => {
                let properties = camera.list_properties();
                GenSrvValue::PropertyList(properties.clone())
            }
            GetProperty(ctrl) => camera.get_property(ctrl)?.into(),
            SetProperty(ctrl, value, auto) => camera.set_property(ctrl, &value, auto)?.into(),
            CancelCapture => camera.cancel_capture()?.into(),
            IsCapturing => PropertyValue::Bool(camera.is_capturing()).into(),
            Capture => GenSrvValue::Image(camera.capture()?.into()),
            StartExposure => camera.start_exposure()?.into(),
            DownloadImage => GenSrvValue::Image(camera.download_image()?.into()),
            ImageReady => PropertyValue::Bool(camera.image_ready()?).into(),
            CameraState => camera.camera_state()?.into(),
            SetRoi(roi) => (*camera.set_roi(&roi)?).into(),
            GetRoi => (*camera.get_roi()).into(),
        };
        Ok(res)
    }
}
