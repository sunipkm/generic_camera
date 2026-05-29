use std::{
    cell::{LazyCell, OnceCell},
    collections::HashMap,
    f64,
    marker::{PhantomData, PhantomPinned},
    mem::MaybeUninit,
    ops::{Deref, DerefMut, Range, RangeInclusive},
    sync::{Arc, LazyLock, Mutex},
};

use generic_camera::{
    GenCamCtrl, Property,
    controls::{CustomName, DeviceCtrl, SensorCtrl},
    property::PropertyLims,
};
use player_one_camera_sys::{
    self as poa, Camera, CameraProperties, CameraState, ConfigAttributes, ConfigParameter,
    ConfigValue, ConfigValueKind, Id, ImageFormat, PoaResult, close_camera,
    ffi_util::{MaybeInvalid, ValidationError},
    get_camera_count, get_state,
};

use crate::{
    raw::{
        error::{CameraError, RawError},
        property::ConfigVal,
    },
    util::poa_call,
};

/// An owned handle to an open and initialized camera device
/// When this camera goes out of scope, it is closed.
#[repr(transparent)]
pub struct OwnedCamera {
    id: Id<Camera>,
}
impl OwnedCamera {
    /// Opens and initializes a camera
    ///
    /// # Safety
    /// - This camera must be the sole owner of the camera id.
    ///   When this camera goes out of scope, it is closed.
    /// - While this [`OwnedCamera`] is alive, it must not be closed during any method calls
    /// - The id must not be shared between threads without synchronization
    pub unsafe fn new(id: Id<Camera>) -> Result<Self, CameraError> {
        // we first need to get the state before doing anything so we make sure that we don't double init
        let state = unsafe { poa_call!(poa::get_state(id) @ out) }?;
        match state.get()? {
            // If we are open, we don't want to open again. Just try to init, there's no harm (I think. The docs are very unclear).
            // If the camera state is unknown, we are probably open, so just try to reinit anyway
            CameraState::Opened => unsafe {
                poa::init_camera(id).into_result()?;
            },
            // If we are closed, we need to open + init
            CameraState::Closed => {
                unsafe {
                    poa::open_camera(id).into_result()?;
                    poa::init_camera(id).into_result()?;
                };
            }
            // If we are exposing, we don't have to do anything
            CameraState::Exposing => {}
        }
        unsafe {
            poa::open_camera(id).into_result()?;
            poa::init_camera(id).into_result()?;
        };
        Ok(Self { id })
    }
    fn get_config_attrs(
        &mut self,
    ) -> Result<impl Iterator<Item = ConfigAttributes> + '_, CameraError> {
        let count = unsafe { poa_call!(poa::get_config_count(self.id) @ out) }?;
        Ok((0..count)
            .into_iter()
            // it should be impossible for this to error since we have a valid index and open camera
            // config attributes for a camera do not change, but if it does error... just discard the result and
            // continue like nothing happened
            .filter_map(|x| unsafe {
                poa_call!(poa::get_config_attributes(self.id, x) @ out).ok()
            }))
    }
    fn get_state(&mut self) -> Result<CameraState, CameraError> {
        // SAFETY: We have exclusive access to the camera
        let res = unsafe {
            poa_call!(
                poa::get_state(self.id) @ out
            )?
        };
        Ok(res.get()?)
    }
    fn get_config<T: ConfigVal>(&mut self, prop: ConfigParameter) -> Result<(T, bool), poa::Error> {
        let ty = prop.value_type();
        if ty != T::KIND {
            return Err(poa::Error::InvalidConfig);
        }
        let (value, is_auto) =
            unsafe { poa_call!(poa::get_config(self.id, prop) @ out_prop out_auto) }?;
        // SAFETY: We know that `ty` should match the active variant for the values of `prop`
        // and we know that `ty == T::KIND`, therefore by transitive property we
        // know that the active variant should be `T::KIND`.
        let res = unsafe { T::from_value(value) };
        Ok((res, is_auto.into_bool()))
    }
    /// # Safety
    /// The caller must ensure that the value is in range
    unsafe fn set_config_unchecked<T: ConfigVal>(
        &mut self,
        prop: ConfigParameter,
        value: T,
        auto: bool,
    ) -> Result<(), poa::Error> {
        let ty = prop.value_type();
        if ty != T::KIND {
            return Err(poa::Error::InvalidConfig);
        }
        let value = value.to_value();
        // SAFETY: We just checked that the config value is the right type and the caller needs to assert that it
        // is in range
        unsafe { poa::set_config(self.id, prop, value, auto.into()).into_result() }
    }
    fn cleanup(&mut self) -> Result<(), RawError> {
        unsafe {
            _ = self.set_config_unchecked(ConfigParameter::Cooler, false, false);
            _ = poa::stop_exposure(self.id);
            _ = poa::close_camera(self.id)
                .into_result()
                .map_err(|x| RawError::Camera(CameraError::Internal(x)))?;
        }
        Ok(())
    }
}

impl Drop for OwnedCamera {
    fn drop(&mut self) {
        _ = self.cleanup();
        // close_camera(self.id)
    }
}
const READABLE: u8 = 0b1;
const WRITABLE: u8 = 0b10;
const SUPPORTS_AUTO: u8 = 0b100;

struct Attributes {
    param: ConfigParameter,
    min: ConfigValue,
    max: ConfigValue,
    default: ConfigValue,
    name: Box<str>,
    description: Box<str>,
    // we store the flags in a u8 so we don't waste 12 bytes doing literally nothing
    flags: u8,
}

impl Attributes {
    pub fn new(attrs: ConfigAttributes) -> Option<Self> {
        // If the library reports a parameter we don't know about, just ignore it
        let kind = attrs.kind.get().ok()?;
        let min = attrs.min_value;
        let max = attrs.max_value;
        let default = attrs.default_value;
    }
}

struct HandleInner {
    camera: OwnedCamera,
    // read-only, except the user custom id may change if the user sets it once.
    // Maybe I should also do some layout adjustment because this struct
    // is 992 bytes
    properties: CameraProperties,
    // read-only, sorted list of config parameter attributes by parameter.
    param_to_attrs: Box<[Attributes]>,
}
impl HandleInner {
    pub fn open_and_init(props: &CameraProperties) -> Result<Self, CameraError> {
        let mut owned = unsafe { OwnedCamera::new(props.camera_id)? };
        let props = owned.get_config_attrs()?;
    }
    fn get_param_attr(&self, param: ConfigParameter) -> Option<&Attributes> {
        let idx = self
            .param_to_attrs
            .binary_search_by_key(&param, |attr| attr.param)
            .ok()?;
        Some(self.param_to_attrs.get(idx)?)
    }
}
fn camera_properties_to_gencam(props: &CameraProperties) -> HashMap<GenCamCtrl, Property> {
    // DeviceCtrl
    let any_string = PropertyLims::EnumStr {
        variants: vec![],
        default: String::new(),
    };
    let ro_string = Property::new(any_string.clone(), false, true);
    let mut product_id_prop = Property::new(
        PropertyLims::EnumStr {
            variants: vec![],
            default: "A0A0".to_owned(),
        },
        false,
        true,
    );
    product_id_prop.set_doc("The camera's product ID. The vID of PlayerOne is `0xA0A0`.");
    let device_props: [(_, Property); _] = [
        (
            DeviceCtrl::UserId,
            Property::new(any_string.clone(), false, false),
        ),
        (DeviceCtrl::SerialNumber, ro_string.clone()),
        (DeviceCtrl::VendorName, ro_string.clone()),
        (DeviceCtrl::ModelName, ro_string.clone()),
        (
            DeviceCtrl::Custom(CustomName::from("Product ID")),
            product_id_prop,
        ), // (DeviceCtrl::CoolerPower)
    ];
    let bin_mode_slice = props.binning_modes.to_slice();
    let bin_lims = PropertyLims::EnumInt {
        variants: bin_mode_slice.iter().map(|x| x.id() as i64).collect(),
        default: bin_mode_slice.first().map(|x| x.id() as i64).unwrap_or(1),
    };
    let pixel_props = Property::new(
        PropertyLims::Float {
            min: 0.0,
            max: 1.0e6, // me when the pixels are a meter wide
            step: 1.0,
            default: 0.0,
        },
        false,
        true,
    );
    // let ro_int =
    let img_formats = props.formats.to_slice();

    let sensor_props: [(_, Property); _] = [
        (SensorCtrl::Name, ro_string.clone()),
        (
            SensorCtrl::BinningBoth,
            Property::new(bin_lims.clone(), false, false),
        ),
        // POA cameras only support setting both axes at the same time
        (
            SensorCtrl::BinningHorz,
            Property::new(bin_lims.clone(), false, true),
        ),
        (
            SensorCtrl::BinningVert,
            Property::new(bin_lims.clone(), false, true),
        ),
        (SensorCtrl::PixelWidth, pixel_props.clone()),
        (SensorCtrl::PixelHeight, pixel_props.clone()),
        (
            SensorCtrl::PixelFormat,
            Property::new(
                PropertyLims::EnumStr {
                    variants: img_formats
                        .iter()
                        .copied()
                        .filter_map(map_pixel_format)
                        .collect(),
                    default: img_formats
                        .first()
                        .copied()
                        .and_then(map_pixel_format)
                        .unwrap_or("RAW8".to_owned()),
                },
                false,
                false,
            ),
        ),
        (
            SensorCtrl::HeightMax,
            Property::new(
                PropertyLims::Unsigned {
                    min: 0,
                    max: u16::MAX as _,
                    step: 1,
                    default: props.max_height.try_into().unwrap_or_default(),
                },
                false,
                true,
            ),
        ),
        (
            SensorCtrl::WidthMax,
            Property::new(
                PropertyLims::Unsigned {
                    min: 0,
                    max: u16::MAX as _,
                    step: 1,
                    default: props.max_width.try_into().unwrap_or_default(),
                },
                false,
                true,
            ),
        ),
    ];
    let mut map = HashMap::from_iter([]);
    map
}
fn map_pixel_format(fmt: MaybeInvalid<ImageFormat>) -> Option<String> {
    Some(match fmt.get().ok()? {
        ImageFormat::Mono8 => "MONO",
        ImageFormat::Raw8 => "RAW8",
        ImageFormat::Raw16 => "RAW16",
        ImageFormat::Rgb24 => "RGB24",
    })
    .map(<_>::to_owned)
}
/// A handle to an owned camera
pub struct Handle {
    camera: Arc<Mutex<HandleInner>>,
}
