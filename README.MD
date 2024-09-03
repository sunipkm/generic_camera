# generic_camera

`generic_camera` provides a well-defined and ergonomic API to write interfaces to capture frames from CCD/CMOS based
detectors through Rust traits `generic_camera::GenericCam` and `generic_camera::GenericCamInfo`. The library uses the
[`refimage::GenericImage`](https://crates.io/crates/refimage) object to encapsulate the captured image and necessary 
metadata.

You can use `generic_camera` to:
 - Write user-friendly interfaces to C APIs to access different kinds of cameras in a uniform fashion,
 - Acquire images from these cameras in different pixel formats (using the [`refimage`](https://crates.io/crates/refimage) crate as a backend),
 - Save these images to `FITS` files (requires the `cfitsio` C library, and uses the [`fitsio`](https://crates.io/crates/fitsio) crate) with extensive metadata,
 - Save the images in the [`bincode`](https://crates.io/crates/bincode),
 - Alternatively, use the [`image`](https://crates.io/crates/image) feature to obtain `JPEG`, `PNG`, `BMP` etc.

## Usage
Add this to your `Cargo.toml`:
```toml
[dependencies]
generic_camera = "0.0"
```
and this to your source code:
```rs
use generic_camera::{GenCamDriver, GenCam, GenCamInfo, Error, DynamicSerialImage, OptimumExposureBuilder, SerialImageBuffer};
```

## Example
Since this library is mostly trait-only, refer to projects (such as [`cameraunit_asi`](https://crates.io/crates/cameraunit_asi)) to see it in action.

## Notes
The interface provides two traits:
 1. `CameraUnit`: This trait supports extensive access to the camera, and provides the API for mutating the camera
 state, such as changing the exposure, region of interest on the detector, etc. The object implementing this trait
 should not derive from the `Clone` trait, since ideally image capture should happen in a single thread.
 2. `CameraInfo`: This trait supports limited access to the camera, and provides the API for obtaining housekeeping
 data such as temperatures, gain etc., while allowing limited mutation of the camera state, such as changing the
 detector temperature set point, turning cooler on and off, etc.

Ideally, the crate implementing the camera interface should
 1. Implement the `CameraUnit` for a `struct` that does not allow cloning, and implement a second,
 smaller structure that allows clone and implement only `CameraInfo` for that struct.
 2. Provide functions to get the number of available cameras, a form of unique identification for the cameras,
 and to open a camera using the unique identification. Additionally, a function to open the first available camera
 may be provided.
 3. Upon opening a camera successfully, a tuple of two objects - one implementing the `CameraUnit` trait and
 another implementing the `CameraInfo` trait, should be returned. The second object should be clonable to be
 handed off to some threads if required to handle housekeeping functions.