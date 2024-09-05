#![deny(missing_docs)]
/*!

# cameraunit

`cameraunit` provides a well-defined and ergonomic API to write interfaces to capture frames from CCD/CMOS based
detectors through Rust traits `cameraunit::CameraUnit` and `cameraunit::CameraInfo`. The library additionally
provides the `cameraunit::ImageData` struct to obtain images with extensive metadata.

You can use `cameraunit` to:
 - Write user-friendly interfaces to C APIs to access different kinds of cameras in a uniform fashion,
 - Acquire images from these cameras in different pixel formats (using the [`image`](https://crates.io/crates/image) crate as a backend),
 - Save these images to `FITS` files (requires the `cfitsio` C library, and uses the [`fitsio`](https://crates.io/crates/fitsio) crate) with extensive metadata,
 - Alternatively, use the internal [`serialimage::DynamicSerialImage`](https://docs.rs/crate/serialimage/latest/) object to obtain `JPEG`, `PNG`, `BMP` etc.

## Usage
Add this to your `Cargo.toml`:
```toml
[dependencies]
cameraunit = "6.0"
```
and this to your source code:
```no_run
use cameraunit::{CameraDriver, CameraUnit, CameraInfo, Error, DynamicSerialImage, OptimumExposureBuilder, SerialImageBuffer};
```

## Example
Since this library is mostly trait-only, refer to projects (such as [`cameraunit_asi`](https://crates.io/crates/cameraunit_asi)) to see it in action.

## Notes
The interface provides three traits:
 1. `CameraDriver`: This trait provides the API to list available cameras, connect to a camera, and connect to the first available camera.
 2. `CameraUnit`: This trait supports extensive access to the camera, and provides the API for mutating the camera
    state, such as changing the exposure, region of interest on the detector, etc. The object implementing this trait
    must not derive from the `Clone` trait, since ideally image capture should happen in a single thread.
 3. `CameraInfo`: This trait supports limited access to the camera, and provides the API for obtaining housekeeping
    data such as temperatures, gain etc., while allowing limited mutation of the camera state, such as changing the
    detector temperature set point, turning cooler on and off, etc.

Ideally, the crate implementing the camera interface should
 1. Implement the `CameraUnit` and `CameraInfo` for a `struct` that does not allow cloning, and implement a second,
    smaller structure that allows clone and implement only `CameraInfo` for that struct.
 2. Provide functions to get the number of available cameras, a form of unique identification for the cameras,
    and to open a camera using the unique identification. Additionally, a function to open the first available camera
    may be provided.
 3. Upon opening a camera successfully, a tuple of two objects - one implementing the `CameraUnit` trait and
    another implementing the `CameraInfo` trait, should be returned. The second object should be clonable to be
    handed off to some threads if required to handle housekeeping functions.

*/

use property::{GenCamCtrl, GenericProperty};
pub use refimage::GenericImage;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{fmt::Display, time::Duration};
use thiserror::Error;

pub use crate::property::{Property, PropertyName, PropertyType, PropertyValue};

mod property;

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
#[must_use]
pub trait GenCamDriver {
    /// Get the number of available devices.
    fn available_devices(&self) -> usize;
    /// List available devices.
    fn list_devices(&mut self) -> Result<Vec<GenCamDescriptor>>;
    /// Connect to a device.
    fn connect_device(
        &mut self,
        descriptor: &GenCamDescriptor,
    ) -> Result<(AnyGenCam, AnyGenCamInfo)>;
    /// Connect to the first available device.
    fn connect_first_device(&mut self) -> Result<(AnyGenCam, AnyGenCamInfo)>;
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

/// Trait for obtaining camera information and cancelling any ongoing image capture.
/// This trait is intended to be exclusively applied to a clonable object that can
/// be passed to other threads for housekeeping purposes.
#[must_use]
pub trait GenCamInfo: Send + Sync {
    /// Check if camera is ready.
    fn camera_ready(&self) -> bool;

    /// Get the camera name.
    fn camera_name(&self) -> &str;

    /// Cancel an ongoing exposure.
    fn cancel_capture(&self) -> Result<()>;

    /// Check if the camera is currently capturing an image.
    fn is_capturing(&self) -> bool;

    /// Get optional capabilities of the camera.
    fn get_properties(&self) -> Vec<Property>;

    /// Get a property by name.
    /// TODO: Replace name with a type that implements a trait for property names.
    fn get_property(&self, name: GenCamCtrl) -> Option<&PropertyValue>;

    /// Set a property by name.
    /// TODO: Replace name with a type that implements a trait for property names.
    fn set_property(&mut self, name: GenCamCtrl, value: &PropertyValue) -> Result<()>;
}

/// Trait for controlling the camera. This trait is intended to be applied to a
/// non-clonable object that is used to capture images and can not be shared across
/// threads.
#[must_use]
pub trait GenCam: Send {
    /// Get the camera vendor.
    fn vendor(&self) -> &str;

    /// Check if camera is ready.
    fn camera_ready(&self) -> bool;

    /// Get the camera name.
    fn camera_name(&self) -> &str;

    /// Get optional capabilities of the camera.
    fn get_properties(&self) -> Vec<Property>;

    /// Get a property by name.
    /// TODO: Replace name with a type that implements a trait for property names.
    fn get_property(&self, name: GenCamCtrl) -> Option<&PropertyValue>;

    /// Set a property by name.
    /// TODO: Replace name with a type that implements a trait for property names.
    fn set_property(&mut self, name: GenCamCtrl, value: &PropertyValue) -> Result<()>;

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

    /// Download the image captured in [`CameraUnit::start_exposure`].
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

    /// Flip the image along X and/or Y axes.
    ///
    /// Raises a `Message` with the message `"Not implemented"` if unimplemented.
    fn set_flip(&mut self, _x: bool, _y: bool) -> Result<()> {
        Err(GenCamError::Message("Not implemented".to_string()))
    }

    /// Check if the image is flipped along X and/or Y axes.
    ///
    /// Defaults to `(false, false)` if unimplemented.
    fn get_flip(&self) -> (bool, bool) {
        (false, false)
    }

    /// Get the detector size (width, height) in pixels.
    fn get_sensor_size(&self) -> (u16, u16);

    /// Get the detector pixel size (x, y) in microns.
    ///
    /// Defaults to `None` if unimplemented.
    fn get_pixel_size(&self) -> Option<(f32, f32)> {
        None
    }
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
    /// Convert from `u32` to [`cameraunit::PixelBpp`].
    ///
    /// # Arguments
    /// - `value` - The value to convert.
    ///   Note: If the value is not one of the known values, `Bpp8` is returned.
    ///
    /// # Returns
    /// The corresponding [`cameraunit::PixelBpp`] value.
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
