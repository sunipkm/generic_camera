use generic_camera::{GenCamDescriptor, PropertyValue};
use player_one_camera_sys::{self as poa, BayerPattern, Camera, CameraProperties, Id};
use std::{collections::HashMap, ffi::c_int, marker::PhantomData};

use crate::{
    raw::{error::CameraError, handle::Handle},
    util::poa_call,
};

struct NotSendSync(PhantomData<*const ()>);
/// A proxy for driver-global state
pub struct Driver {
    // We can be sent across threads, but we don't have synchronized access.
    // If the constructor's invariants are upheld, then we can be sent across threads
    // without any problem, but we just don't have synchronized access
    _not_sync: NotSendSync,
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
        Driver {
            _not_sync: NotSendSync(PhantomData),
        }
    }
    /// Gets the number of available devices.
    pub fn num_devices(&self) -> usize {
        // SAFETY: The constructor of the driver requires upholding the fact these methods
        // require not being called concurrently
        unsafe { poa::get_camera_count() }
            .try_into()
            .unwrap_or_default()
    }

    fn list_raw(&self) -> impl Iterator<Item = CameraProperties> + use<'_> {
        (0..self.num_devices())
            .map(|idx| unsafe { poa_call!(poa::get_camera_properties(idx as c_int) @ out) })
            .filter_map(Result::ok)
    }
    /// Gets the [`GenCamDescriptor`]s of the available devices, discarding any errors in getting device info
    ///
    /// # Custom properties
    /// - `Serial Number` (`EnumString`) => The serial number of the camera
    /// - `Sensor Model Name` (`EnumString`) => The sensor model name
    /// - `Local Path` (`EnumString`) => The path of the camera in the computer host
    /// - `Sensor Width`/`Sensor Height` (`Unsigned`) => The sensor width/height
    /// - `Color Sensor` (`Bool`) => Whether the camera is a color camera
    /// - `Pixel Size` (`Float`) => The pixel size in um
    /// - `ST4 Port` (`Bool`) => Whether the camera has an ST4 port
    /// - `Cooler` (`Bool`) => Whether the camera has a cooler assembly
    /// - `Bit Depth` (`Unsigned`) => The bit depth of the sensor
    /// - `USB3 Speed` (`Bool`) => Whether the camera is connected via a USB-3.0 speed connection
    /// - `Hardware Bin` (`Bool`) => Whether the camera supports hardware binning
    /// - `Bayer Pattern` (`EnumString`) => (If is a color camera) The Bayer pattern
    pub fn list_devices(&self) -> impl Iterator<Item = GenCamDescriptor> + use<'_> {
        self.list_raw().map(|props| GenCamDescriptor {
            id: props.camera_id.id() as usize,
            name: props.model_name.to_str_lossy().into_owned(),
            vendor: "POA".to_owned(),
            info: make_info(&props),
        })
    }
    pub fn connect(
        &self,
        descriptor: &GenCamDescriptor,
    ) -> Result<(Handle, CameraProperties), CameraError> {
        // SAFETY: `Id` is repr(transparent) and wraps a c_int. This id came from the driver
        let desc: Id<Camera> = unsafe { std::mem::transmute(descriptor.id as c_int) };
        let props = unsafe { poa_call!(poa::get_camera_properties_by_id(desc) @ out) }?;
        Ok((Handle::open_and_init(&props)?, props))
    }
}

fn make_info(props: &CameraProperties) -> HashMap<String, PropertyValue> {
    let mut data = HashMap::from_iter(
        [
            (
                "Serial Number",
                props.serial.to_str_lossy().into_owned().into(),
            ),
            (
                "Sensor Name",
                props.sensor_model_name.to_str_lossy().into_owned().into(),
            ),
            (
                "Local Path",
                props.local_path.to_str_lossy().into_owned().into(),
            ),
            ("Sensor Width", (props.max_width as i64 as u64).into()),
            ("Sensor Height", (props.max_height as i64 as u64).into()),
            ("Color Sensor", props.is_color_camera.into_bool().into()),
            ("ST4 Port", props.has_st4_port.into_bool().into()),
            ("Pixel Size", props.pixel_size.into()),
            ("Cooler", props.has_cooler.into_bool().into()),
            ("Bit Depth", (props.bit_depth as u64).into()),
            ("USB3 Speed", props.is_usb3_speed.into_bool().into()),
            (
                "Hardware Bin",
                props.harware_bin_supported.into_bool().into(),
            ),
        ]
        .map(|(k, v)| (k.to_owned(), v)),
    );
    if props.is_color_camera.into_bool()
        && let Ok(bayer) = props.bayer_pattern.get()
    {
        let string = match bayer {
            BayerPattern::Bg => "BG",
            BayerPattern::Gr => "GR",
            BayerPattern::Gb => "GB",
            BayerPattern::Rg => "RG",
            // I don't think this should happen, but just in case...
            BayerPattern::Mono => "NONE",
        };
        data.insert("Bayer Pattern".to_owned(), string.to_owned().into());
    }
    data
}
