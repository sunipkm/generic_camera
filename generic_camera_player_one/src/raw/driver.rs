use generic_camera::{GenCamDescriptor, PropertyValue};
use player_one_camera_sys::{self as poa, CameraProperties};
use std::{collections::HashMap, ffi::c_int, marker::PhantomData, rc::Rc, sync::Arc};

use crate::util::poa_call;

struct NotSync(PhantomData<Rc<()>>);
/// A proxy for driver-global state
pub struct Driver {
    // We can be sent across threads, but we don't have synchronized access.
    // If the constructor's invariants are upheld, then we can be sent across threads
    // without any problem, but we just don't have synchronized access
    _not_sync: NotSync,
}
impl Driver {
    /// Creates a new [`Driver`] proxy
    ///
    /// # Safety
    /// Since the [`Driver`] is just a proxy for global state, you must ensure the following:
    ///
    /// - [`Driver`] methods are not called concurrently from multiple threads, even by different
    ///   [`Driver`] instances
    /// - If there are multiple active instances of the [`Driver`], you must ensure that the same
    ///   camera isn't opened or closed twice
    /// - You must ensure that none of these methods are called concurrently with any write to camera
    ///   state of cameras that come from this driver
    /// - All of the other conditions of global state apply
    ///
    /// The intended usage pattern ensures all of these conditions automatically. You should basically
    /// only ever have a single [`Driver`] instance per program or at the very least only create a
    /// new [`Driver`] after all cameras closed.
    pub unsafe fn new() -> Self {
        Driver {
            _not_sync: NotSync(PhantomData),
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
    /// - `Max Width`/`Max Height` (`Unsigned`) => The max width / height of the ROI in pixels
    pub fn list_devices(&self) -> impl Iterator<Item = GenCamDescriptor> + use<'_> {
        self.list_raw().map(|props| GenCamDescriptor {
            id: props.camera_id.id() as usize,
            name: props.model_name.to_str_lossy().into_owned(),
            vendor: "POA".to_owned(),
            info: make_info(&props),
        })
    }
    /// Returns an iterator over all of the cameras currently available
    ///
    /// # Safety
    /// - This function must not be called from multiple threads concurrently even from different driver
    ///   objects since it accesses global state
    /// - This function must not be called concurrently with functions that write to any camera's state
    pub fn get_raw_cameras(
        &self,
    ) -> impl Iterator<Item = Result<CameraProperties, poa::Error>> + '_ {
        let num_cams = unsafe { poa::get_camera_count() };
        // SAFETY:
        (0..num_cams).map(|cam_idx| unsafe { poa_call!(poa::get_camera_properties(cam_idx) @ out) })
    }
}

fn make_info(props: &CameraProperties) -> HashMap<String, PropertyValue> {
    HashMap::from_iter([
        (
            "Serial Number".to_owned(),
            props.serial.to_str_lossy().into_owned().into(),
        ),
        (
            "Max Width".to_owned(),
            (props.max_width as i64 as u64).into(),
        ),
        (
            "Max Height".to_owned(),
            (props.max_height as i64 as u64).into(),
        ),
        (),
    ])
}
