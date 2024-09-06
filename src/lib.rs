#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
/*!
 * # Generic Camera Interface
 * This crate provides a generic interface for controlling cameras.
 */

pub use controls::*;
pub use refimage::GenericImage;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::sync::Arc;
use std::{fmt::Display, time::Duration};
use thiserror::Error;

pub use crate::property::{Property, PropertyType, PropertyValue};

mod controls;
mod property;
#[cfg(feature = "server")]
mod server;
#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
pub use server::*;

/// The version of the `generic_cam` crate.
pub type Result<T> = std::result::Result<T, GenCamError>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Hash)]
/// This structure defines a region of interest.
/// The region of interest is defined in the un-binned pixel space.
pub struct GenCamRoi {
    /// The minimum X coordinate (in binned pixel space).
    pub x_min: u16,
    /// The minimum Y coordinate (in binned pixel space).
    pub y_min: u16,
    /// The image width (X axis, in binned pixel space).
    pub width: u16,
    /// The image height (Y axis, in binned pixel space).
    pub height: u16,
    /// The X binning factor.
    pub bin_x: u8,
    /// The Y binning factor.
    pub bin_y: u8,
}

impl Display for GenCamRoi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ROI: Origin = ({}, {}), Image Size = ({} x {}), Bin = ({}, {})",
            self.x_min, self.y_min, self.width, self.height, self.bin_x, self.bin_y
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Defines the state of the camera.
pub enum GenCamState {
    /// Camera is idle.
    Idle,
    /// Camera is exposing.
    Exposing(Option<Duration>),
    /// Exposure finished.
    ExposureFinished,
    /// Camera is downloading image.
    Downloading(Option<u32>),
    /// Error occurred.
    Errored(GenCamError),
    /// Camera is in an unknown state.
    Unknown,
}

/// A trait object for a camera unit.
pub type AnyGenCam = Box<dyn GenCam>;
/// A trait object for a camera info.
pub type AnyGenCamInfo = Arc<Box<dyn GenCamInfo>>;

/// Trait for camera drivers. Provides functions to
/// list available devices and connect to a device.
pub trait GenCamDriver {
    /// Get the number of available devices.
    fn available_devices(&self) -> usize;
    /// List available devices.
    fn list_devices(&mut self) -> Result<Vec<GenCamDescriptor>>;
    /// Connect to a device.
    fn connect_device(&mut self, descriptor: &GenCamDescriptor) -> Result<AnyGenCam>;
    /// Connect to the first available device.
    fn connect_first_device(&mut self) -> Result<AnyGenCam>;
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
/// A structure to hold information about a camera device.
pub struct GenCamDescriptor {
    /// The camera ID.
    pub id: usize,
    /// The camera name.
    pub name: String,
    /// The camera vendor.
    pub vendor: String,
    /// The camera model.
    pub model: String,
    /// The camera serial number.
    pub serial: Option<String>,
    /// Camera description.
    pub description: Option<String>,
}

/// Trait for controlling the camera. This trait is intended to be applied to a
/// non-clonable object that is used to capture images and can not be shared across
/// threads.
pub trait GenCam: Send + std::fmt::Debug {
    /// Get the [`GenCamInfo`] object, if available.
    fn info_handle(&self) -> Option<AnyGenCamInfo>;

    /// Get the camera vendor.
    fn vendor(&self) -> &str;

    /// Check if camera is ready.
    fn camera_ready(&self) -> bool;

    /// Get the camera name.
    fn camera_name(&self) -> &str;

    /// Get optional capabilities of the camera.
    fn list_properties(&self) -> Vec<Property>;

    /// Get a property by name.
    fn get_property(&self, name: GenCamCtrl) -> Option<&PropertyValue>;

    /// Set a property by name.
    fn set_property(&mut self, name: GenCamCtrl, value: &PropertyValue) -> Result<()>;

    /// Check if a property is in auto mode.
    fn get_property_auto(&self, name: GenCamCtrl) -> Result<bool>;

    /// Set a property to auto mode.
    fn set_property_auto(&mut self, name: GenCamCtrl, auto: bool) -> Result<()>;

    /// Cancel an ongoing exposure.
    fn cancel_capture(&self) -> Result<()>;

    /// Check if the camera is currently capturing an image.
    fn is_capturing(&self) -> bool;

    /// Capture an image.
    /// This is a blocking call.
    ///
    /// Raises a `Message` with the message `"Not implemented"` if unimplemented.
    fn capture(&self) -> Result<GenericImage>;

    /// Start an exposure and return. This function does NOT block, but may not return immediately (e.g. if the camera is busy).
    fn start_exposure(&self) -> Result<()>;

    /// Download the image captured in [`GenCam::start_exposure`].
    fn download_image(&self) -> Result<GenericImage>;

    /// Get exposure status. This function is useful for checking if a
    /// non-blocking exposure has finished running.
    fn image_ready(&self) -> Result<bool>;

    /// Get the camera state.
    fn camera_state(&self) -> Result<GenCamState>;

    /// Get camera exposure.
    fn get_exposure(&self) -> Result<Duration>;

    /// Set camera exposure.
    fn set_exposure(&mut self, exposure: Duration) -> Result<Duration>;

    /// Set the image region of interest (ROI).
    ///
    /// # Arguments
    /// - `roi` - The region of interest.
    ///
    /// Note:
    /// - The region of interest is defined in the binned pixel space.
    /// - Setting all values to `0` will set the ROI to the full detector size.
    ///
    ///
    /// # Returns
    /// The region of interest that was set, or error.
    fn set_roi(&mut self, roi: &GenCamRoi) -> Result<&GenCamRoi>;

    /// Get the region of interest.
    ///
    /// # Returns
    /// - The region of interest.
    fn get_roi(&self) -> &GenCamRoi;
}

/// Trait for obtaining camera information and cancelling any ongoing image capture.
/// This trait is intended to be exclusively applied to a clonable object that can
/// be passed to other threads for housekeeping purposes.
pub trait GenCamInfo: Send + Sync + std::fmt::Debug {
    /// Check if camera is ready.
    fn camera_ready(&self) -> bool;

    /// Get the camera name.
    fn camera_name(&self) -> &str;

    /// Cancel an ongoing exposure.
    fn cancel_capture(&self) -> Result<()>;

    /// Check if the camera is currently capturing an image.
    fn is_capturing(&self) -> bool;

    /// Get optional capabilities of the camera.
    fn list_properties(&self) -> Vec<Property>;

    /// Get a property by name.
    fn get_property(&self, name: GenCamCtrl) -> Option<&PropertyValue>;

    /// Set a property by name.
    fn set_property(&mut self, name: GenCamCtrl, value: &PropertyValue) -> Result<()>;

    /// Check if a property is in auto mode.
    fn get_property_auto(&self, name: GenCamCtrl) -> Result<bool>;

    /// Set a property to auto mode.
    fn set_property_auto(&mut self, name: GenCamCtrl, auto: bool) -> Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
/// Pixel bit depth.
pub enum GenCamPixelBpp {
    /// 8 bits per pixel. This is the default.
    Bpp8 = 8,
    /// 10 bits per pixel.
    Bpp10 = 10,
    /// 12 bits per pixel.
    Bpp12 = 12,
    /// 14 bits per pixel.
    Bpp16 = 16,
    /// 16 bits per pixel.
    Bpp24 = 24,
    /// 32 bits per pixel.
    Bpp32 = 32,
}

impl From<u32> for GenCamPixelBpp {
    /// Convert from `u32` to [`GenCamPixelBpp`].
    ///
    /// # Arguments
    /// - `value` - The value to convert.
    ///   Note: If the value is not one of the known values, `Bpp8` is returned.
    ///
    /// # Returns
    /// The corresponding [`GenCamPixelBpp`] value.
    fn from(value: u32) -> Self {
        match value {
            8 => GenCamPixelBpp::Bpp8,
            10 => GenCamPixelBpp::Bpp10,
            12 => GenCamPixelBpp::Bpp12,
            16 => GenCamPixelBpp::Bpp16,
            24 => GenCamPixelBpp::Bpp24,
            32 => GenCamPixelBpp::Bpp32,
            _ => GenCamPixelBpp::Bpp8,
        }
    }
}

#[derive(Error, Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Errors returned by camera operations.
pub enum GenCamError {
    /// Error message.
    #[error("Error: {0}")]
    Message(String),
    /// Invalid index.
    #[error("Invalid index: {0}")]
    InvalidIndex(i32),
    /// Invalid ID.
    #[error("Invalid ID: {0}")]
    InvalidId(i32),
    /// Invalid control type.
    #[error("Invalid control type: {0}")]
    InvalidControlType(String),
    /// No cameras available.
    #[error("No cameras available")]
    NoCamerasAvailable,
    /// Camera not open for access.
    #[error("Camera not open for access")]
    CameraClosed,
    /// Camera already removed.
    #[error("Camera already removed")]
    CameraRemoved,
    /// Invalid path.
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    /// Invalid format.
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    /// Invalid size.
    #[error("Invalid size: {0}")]
    InvalidSize(usize),
    /// Invalid image type.
    #[error("Invalid image type: {0}")]
    InvalidImageType(String),
    /// Operation timed out.
    #[error("Operation timed out")]
    TimedOut,
    /// Invalid sequence.
    #[error("Invalid sequence")]
    InvalidSequence,
    /// Buffer too small.
    #[error("Buffer too small: {0}")]
    BufferTooSmall(usize),
    /// Exposure in progress.
    #[error("Exposure already in progress")]
    ExposureInProgress,
    /// General error.
    #[error("General error: {0}")]
    GeneralError(String),
    /// Invalid mode.
    #[error("Invalid mode: {0}")]
    InvalidMode(String),
    /// Exposure failed.
    #[error("Exposure failed: {0}")]
    ExposureFailed(String),
    /// Invalid value.
    #[error("Invalid value: {0}")]
    InvalidValue(String),
    /// Out of bounds.
    #[error("Out of bounds: {0}")]
    OutOfBounds(String),
    /// Exposure not started.
    #[error("Exposure not started.")]
    ExposureNotStarted,
    /// Property not found.
    #[error("Property not found: {0}")]
    PropertyNotFound(String),
    /// Read only property.
    #[error("Property is read only")]
    ReadOnly,
    /// Property not an enum.
    #[error("Property is not an enum")]
    PropertyNotEnum,
    /// Property is not a number.
    #[error("Property is not a number")]
    PropertyNotNumber,
    /// Property is an enum, hence does not support min/max.
    #[error("Property is an enum")]
    PropertyIsEnum,
    /// Auto mode not supported.
    #[error("Auto mode not supported")]
    AutoNotSupported,
}
