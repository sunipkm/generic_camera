use rand::{thread_rng, Rng};
use std::collections::HashMap;

use crate::AnyGenCam;
#[allow(unused_imports)]
use crate::GenCam;
use crate::GenCamCtrl;
use crate::GenCamError;
use crate::GenCamResult;
use crate::GenCamRoi;
use crate::GenCamState;
use crate::Property;
use crate::PropertyValue;
use refimage::GenericImage;
use serde::{Deserialize, Serialize};

/// The result of a generic camera server call.
pub type GenSrvOutput<'a> = GenCamResult<GenSrvValue<'a>>;

/// The Ok variant of a generic camera server call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GenSrvValue<'a> {
    /// No return value.
    Unit,
    /// A single [`PropertyValue`].
    Property {
        /// The value of the property.
        value: PropertyValue,
        /// The auto setting of the property, if applicable.
        auto: Option<bool>,
    },
    #[serde(borrow)]
    /// A captured image from the camera.
    Image(GenericImage<'a>),
    /// Region of interest defined on the camera.
    Roi(GenCamRoi),
    /// The current state of the camera.
    State(GenCamState),
    /// A list of properties available on the camera.
    PropertyList(HashMap<GenCamCtrl, Property>),
}

impl<'a> From<()> for GenSrvValue<'a> {
    fn from(_: ()) -> Self {
        GenSrvValue::Unit
    }
}

impl<'a> From<(PropertyValue, bool)> for GenSrvValue<'a> {
    fn from(value: (PropertyValue, bool)) -> Self {
        let (value, auto) = value;
        GenSrvValue::Property {
            value,
            auto: Some(auto),
        }
    }
}

impl<'a> From<(&PropertyValue, bool)> for GenSrvValue<'a> {
    fn from(value: (&PropertyValue, bool)) -> Self {
        let (value, auto) = value;
        GenSrvValue::Property {
            value: value.clone(),
            auto: Some(auto),
        }
    }
}

impl<'a> From<PropertyValue> for GenSrvValue<'a> {
    fn from(value: PropertyValue) -> Self {
        GenSrvValue::Property { value, auto: None }
    }
}

impl<'a> From<GenericImage<'a>> for GenSrvValue<'a> {
    fn from(image: GenericImage<'a>) -> Self {
        GenSrvValue::Image(image)
    }
}

impl<'a> From<GenCamRoi> for GenSrvValue<'a> {
    fn from(roi: GenCamRoi) -> Self {
        GenSrvValue::Roi(roi)
    }
}

impl<'a> From<GenCamState> for GenSrvValue<'a> {
    fn from(state: GenCamState) -> Self {
        GenSrvValue::State(state)
    }
}

impl<'a> From<HashMap<GenCamCtrl, Property>> for GenSrvValue<'a> {
    fn from(properties: HashMap<GenCamCtrl, Property>) -> Self {
        GenSrvValue::PropertyList(properties)
    }
}

/// The result of a generic camera server call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GenSrvResult<'a> {
    #[serde(borrow)]
    /// A successful result.
    Ok(GenSrvValue<'a>),
    /// An error occurred.
    Err(GenCamError),
}

impl <'a>From<GenSrvResult<'a>> for GenCamResult<GenSrvValue<'a>> {
    fn from(result: GenSrvResult<'a>) -> Self {
        match result {
            GenSrvResult::Ok(ok) => Ok(ok),
            GenSrvResult::Err(e) => Err(e),
        }
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
/// ```no_run
/// use generic_camera::GenCamServer;
/// use generic_camera::GenCam;
/// 
/// let mut server = GenCamServer::default();
/// let id = server.add_camera(...);
/// ```
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
    pub fn execute_fn(&mut self, id: i32, sig: GenSrvCmd) -> GenCamResult<GenSrvValue> {
        let Some(camera) = self.get_camera_mut(id) else {
            return Err(GenCamError::InvalidId(id));
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
            ListProperties => {
                let properties = camera.list_properties();
                GenSrvValue::PropertyList(properties.clone())
            }
            GetProperty(ctrl) => {
                camera.get_property(ctrl)?.into()
            }
            SetProperty(ctrl, value, auto) => {
               camera.set_property(ctrl, &value, auto)?.into()
            }
            CancelCapture => {
                camera.cancel_capture()?.into()

                
            }
            IsCapturing => {
                PropertyValue::Bool(camera.is_capturing()).into()
            }
            Capture => {
                camera.capture()?.into()
            }
            StartExposure => {
                camera.start_exposure()?.into()
                
            }
            DownloadImage => {
                camera.download_image()?.into()
                
            }
            ImageReady => {
                PropertyValue::Bool(camera.image_ready()?).into()
            }
            CameraState => {
                camera.camera_state()?.into()
            }
            SetRoi(roi) => {
                (*camera.set_roi(&roi)?).into()
            }
            GetRoi => {
                (*camera.get_roi()).into()
            }
        };
        Ok(res)
    }
}
