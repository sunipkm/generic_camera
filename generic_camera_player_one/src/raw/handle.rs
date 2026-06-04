use std::{
    collections::HashMap,
    f64,
    mem::MaybeUninit,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};

use generic_camera::{
    GenCamCtrl, GenCamPixelBpp, GenCamRoi, Property, PropertyValue,
    controls::{CustomName, DeviceCtrl, SensorCtrl},
    property::PropertyLims,
};
use player_one_camera_sys::{
    self as poa, Bool, Camera, CameraProperties, CameraState, ConfigAttributes, ConfigParameter,
    Id, ImageFormat, MaybeInvalidImageFormat, Millis,
    senti::{MaybeInvalid, bytemuck, cstring::BoundedCString, ptr::Buffer},
};
use refimage::{BayerPattern, ColorSpace, EXPOSURE_KEY, GenericImageRef, ImageRef};

use crate::{
    raw::{
        error::{CameraError, PropertyError},
        property::{ConfigVal, gencam2poa_param, poa2gencam_ctrl},
    },
    util::poa_call,
};

/// An owned handle to an open and initialized camera device
/// When this camera goes out of scope, it is closed.
#[repr(transparent)]
#[derive(Debug)]
struct OwnedCamera {
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
    fn cleanup(&mut self) -> Result<(), poa::Error> {
        unsafe {
            _ = self.set_config_unchecked(ConfigParameter::Cooler, false, false);
            _ = poa::stop_exposure(self.id);
            poa::close_camera(self.id).into_result()?;
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CaptureState {
    Idle,
    Capturing(Option<Instant>),
    Ready,
}
#[derive(Debug)]
pub struct HandleInner {
    camera: OwnedCamera,
    capture_state: CaptureState,
    gencam_props: Arc<HashMap<GenCamCtrl, Property>>,
    // read-only, except the user custom id may change if the user sets it once.
    // Maybe I should also do some layout adjustment because this struct
    // is 992 bytes
    properties: CameraProperties,
    counter: usize,
}
impl HandleInner {
    pub fn open_and_init(props: &CameraProperties) -> Result<Self, CameraError> {
        let mut owned = unsafe { OwnedCamera::new(props.camera_id)? };
        let attrs = owned.get_config_attrs()?;
        let mut prop_map = camera_properties_to_gencam(props);
        prop_map.extend(attrs.filter_map(|attrs| {
            let (ctrl, codec) = poa2gencam_ctrl(attrs.kind.get().ok()?)?;
            Some((ctrl, codec.poa2gencam_property(attrs)?))
        }));
        let capture_state = match owned.get_state()? {
            CameraState::Opened => CaptureState::Idle,
            CameraState::Exposing => CaptureState::Capturing(None),
            CameraState::Closed => return Err(CameraError::Internal(poa::Error::InvalidId)),
        };
        Ok(Self {
            camera: owned,
            properties: *props,
            gencam_props: Arc::new(prop_map),
            counter: 0,
            capture_state,
        })
    }
    fn update_capture_state(&mut self) -> Result<(), poa::Error> {
        #[allow(clippy::single_match, reason = "Might change later")]
        #[allow(
            clippy::collapsible_match,
            reason = "Guards should not have side effects, so I will not make this a guard"
        )]
        match self.capture_state {
            CaptureState::Capturing(_) => {
                if unsafe { poa_call!(poa::is_image_ready(self.camera.id) @ out) }?.into_bool() {
                    self.capture_state = CaptureState::Ready;
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub fn capture_state(&mut self) -> Result<&CaptureState, poa::Error> {
        self.update_capture_state()?;
        Ok(&self.capture_state)
    }
    fn get_special_prop(
        &mut self,
        ctrl: GenCamCtrl,
    ) -> Result<Option<(PropertyValue, bool)>, PropertyError> {
        Ok(Some((
            match ctrl {
                GenCamCtrl::Device(DeviceCtrl::UserId) => self
                    .properties
                    .user_custom_id
                    .to_str_lossy()
                    .into_owned()
                    .into(),

                GenCamCtrl::Device(DeviceCtrl::SerialNumber) => {
                    self.properties.serial.to_str_lossy().into_owned().into()
                }
                GenCamCtrl::Device(DeviceCtrl::ModelName) => self
                    .properties
                    .model_name
                    .to_str_lossy()
                    .into_owned()
                    .into(),
                GenCamCtrl::Sensor(
                    SensorCtrl::BinningBoth | SensorCtrl::BinningHorz | SensorCtrl::BinningVert,
                ) => {
                    // this should never fail since the id is valid and we're open, but handle it anyway
                    let bin_idx = unsafe {
                        poa_call!(poa::get_image_bin(self.camera.id) @ out)
                            .map_err(|_| PropertyError::Failed)?
                    };
                    // this should never be out of bounds unless the driver is bugged,
                    // but I really don't want to panic
                    let bin = self
                        .properties
                        .binning_modes
                        .to_slice()
                        .get(bin_idx as usize)
                        .ok_or(PropertyError::Failed)?;
                    let factor = bin.id() as u64;
                    factor.into()
                }
                GenCamCtrl::Sensor(SensorCtrl::PixelHeight | SensorCtrl::PixelWidth) => {
                    self.properties.pixel_size.into()
                }
                GenCamCtrl::Sensor(SensorCtrl::WidthMax) => {
                    (self.properties.max_width as u64).into()
                }
                GenCamCtrl::Sensor(SensorCtrl::HeightMax) => {
                    (self.properties.max_height as u64).into()
                }
                GenCamCtrl::Sensor(SensorCtrl::PixelFormat) => {
                    let fmt = unsafe {
                        poa_call!(poa::get_image_format(self.camera.id) @ out)
                            .map_err(|_| PropertyError::Failed)
                    }?;
                    map_pixel_format(MaybeInvalidImageFormat(fmt))
                        .unwrap_or(GenCamPixelBpp::Bpp8)
                        .into()
                }

                _ => return Ok(None),
            },
            false,
        )))
    }
    pub fn get_property(
        &mut self,
        ctrl: GenCamCtrl,
    ) -> Result<(PropertyValue, bool), PropertyError> {
        if let Some(res) = self.get_special_prop(ctrl)? {
            return Ok(res);
        };
        let (prop, marshalling) = gencam2poa_param(ctrl).ok_or(PropertyError::Unsupported)?;
        let res = unsafe { poa_call!(poa::get_config(self.camera.id, prop) @ out_value is_auto) };
        match res {
            Ok((val, auto)) => Ok((
                unsafe { marshalling.poa2gencam_quantity(prop.value_type(), val) }
                    .ok_or(PropertyError::WrongType)?,
                auto.into_bool(),
            )),
            Err(poa::Error::InvalidConfig) => Err(PropertyError::Unsupported),
            Err(_) => Err(PropertyError::Failed),
        }
    }
    fn set_flip(&mut self, flip_x: bool, flip_y: bool) -> Result<(), PropertyError> {
        // POA cameras are annoying. They have 4 linked parameters for the different combinations of flip
        // and have setters that ignore the value. It essentially acts like an enum property but it's just
        // 4 different properties.
        let param = match (flip_x, flip_y) {
            (false, false) => ConfigParameter::FlipNone,
            (true, false) => ConfigParameter::FlipHorizontal,
            (false, true) => ConfigParameter::FlipVertical,
            (true, true) => ConfigParameter::FlipBoth,
        };
        unsafe { self.camera.set_config_unchecked(param, true, false) }
            .map_err(|_| PropertyError::Failed)?;
        Ok(())
    }
    fn set_special_prop(
        &mut self,
        ctrl: GenCamCtrl,
        value: &PropertyValue,
        _: bool,
    ) -> Result<Option<()>, PropertyError> {
        #[allow(clippy::unit_arg, reason = "")]
        Ok(Some(match ctrl {
            GenCamCtrl::Device(DeviceCtrl::UserId) => {
                let prop_string: &str = value.as_enum_str().ok_or(PropertyError::WrongType)?;
                let string =
                    BoundedCString::from_str(prop_string).ok_or(PropertyError::ValueOutOfRange)?;
                unsafe {
                    poa::set_user_custom_id(
                        self.camera.id,
                        Some(&string),
                        string.to_bytes().len() as _,
                    )
                    .into_result()
                }
                .map_err(|_| PropertyError::Failed)?;
                // now we have to refresh the camera properties since the user id changed.
                // I'm not sure if we really need to refresh the whole struct since documentation
                // is unclear
                let Ok(prop) =
                    (unsafe { poa_call!(poa::get_camera_properties_by_id(self.camera.id) @ prop) })
                else {
                    return Ok(Some(()));
                };
                // assert!(self.camera.id == prop.camera_id, "the library broke their own invariants")
                self.properties = prop;
            }
            GenCamCtrl::Sensor(SensorCtrl::BinningBoth) => {
                let val: u64 = value.as_u64().ok_or(PropertyError::WrongType)?;
                let bin = self
                    .properties
                    .binning_modes
                    .to_slice()
                    .iter()
                    .find(|x| x.id() as u64 == val)
                    .ok_or(PropertyError::ValueOutOfRange)?;
                unsafe { poa::set_image_bin(self.camera.id, *bin).into_result() }
                    .map_err(|_| PropertyError::Failed)?;
            }
            GenCamCtrl::Sensor(SensorCtrl::PixelFormat) => {
                let fmt = if let Some(string) = value.as_enum_str() {
                    match string {
                        "mono" | "MONO" => ImageFormat::Mono8,
                        "raw8" | "RAW8" => ImageFormat::Raw8,
                        "raw16" | "RAW16" => ImageFormat::Raw16,
                        "rgb24" | "RGB24" => ImageFormat::Rgb24,
                        _ => return Err(PropertyError::ValueOutOfRange),
                    }
                } else if let Some(pixel) = value.as_pixel_fmt() {
                    match pixel {
                        GenCamPixelBpp::Bpp8 if self.properties.is_color_camera.into_bool() => {
                            ImageFormat::Mono8
                        }
                        GenCamPixelBpp::Bpp8 => ImageFormat::Raw8,
                        GenCamPixelBpp::Bpp16 => ImageFormat::Raw16,
                        GenCamPixelBpp::Bpp24 if self.properties.is_color_camera.into_bool() => {
                            ImageFormat::Rgb24
                        }
                        _ => return Err(PropertyError::ValueOutOfRange),
                    }
                } else {
                    return Err(PropertyError::WrongType);
                };

                if !self
                    .properties
                    .formats
                    .to_slice()
                    .contains(&MaybeInvalidImageFormat(MaybeInvalid::new(fmt)))
                {
                    return Err(PropertyError::ValueOutOfRange);
                }
                unsafe {
                    poa::set_image_format(self.camera.id, fmt)
                        .into_result()
                        .map_err(|_| PropertyError::Failed)?
                }
            }
            GenCamCtrl::Sensor(SensorCtrl::ReverseX) => {
                let to: bool = value.as_bool().ok_or(PropertyError::WrongType)?;

                let (flip_y, _) = self
                    .camera
                    .get_config::<bool>(ConfigParameter::FlipVertical)
                    .map_err(|_| PropertyError::Failed)?;
                self.set_flip(to, flip_y)?;
            }
            GenCamCtrl::Sensor(SensorCtrl::ReverseY) => {
                let to: bool = value.as_bool().ok_or(PropertyError::WrongType)?;

                let (flip_x, _) = self
                    .camera
                    .get_config::<bool>(ConfigParameter::FlipHorizontal)
                    .map_err(|_| PropertyError::Failed)?;
                self.set_flip(flip_x, to)?;
            }
            // do nothing. We have no special selectors.
            GenCamCtrl::Device(DeviceCtrl::TemperatureSelector) => {}
            _ => return Ok(None),
        }))
    }
    pub fn exposure_time(&mut self) -> Result<Duration, poa::Error> {
        let (exposure_time, _) = self
            .camera
            .get_config::<f64>(ConfigParameter::ExposureSeconds)?;
        Ok(Duration::from_secs_f64(exposure_time))
    }
    pub fn set_property(
        &mut self,
        ctrl: GenCamCtrl,
        value: &PropertyValue,
        auto: bool,
    ) -> Result<(), PropertyError> {
        if let Ok(CameraState::Exposing) = self.camera.get_state() {
            return Err(PropertyError::Exposing);
        }
        if self.set_special_prop(ctrl, value, auto)?.is_some() {
            return Ok(());
        }

        let prop = self
            .gencam_props
            .get(&ctrl)
            .ok_or(PropertyError::Unsupported)?;
        prop.validate(value)
            .map_err(|_| PropertyError::ValueOutOfRange)?;
        let (param, marshalling) = gencam2poa_param(ctrl).ok_or(PropertyError::Unsupported)?;
        let (kind, config) = marshalling
            .gencam2poa_quantity(value.clone())
            .ok_or(PropertyError::WrongType)?;
        if kind != param.value_type() {
            return Err(PropertyError::WrongType);
        }
        let res =
            unsafe { poa::set_config(self.camera.id, param, config, auto.into()).into_result() };
        match res {
            Ok(()) => Ok(()),
            Err(poa::Error::InvalidConfig) => Err(PropertyError::Unsupported),
            _ => Err(PropertyError::Failed),
        }
    }

    pub fn start_exposure(&mut self) -> Result<(), CameraError> {
        if matches!(self.capture_state()?, CaptureState::Capturing(_)) {
            return Err(CameraError::Internal(poa::Error::Exposing));
        }
        unsafe { poa::start_exposure(self.camera.id, Bool::True) }.into_result()?;
        self.capture_state = CaptureState::Capturing(Some(Instant::now()));
        Ok(())
    }
    pub fn stop_exposure(&mut self) -> Result<(), CameraError> {
        if !matches!(self.capture_state()?, CaptureState::Capturing(_)) {
            return Ok(());
        }
        unsafe { poa::stop_exposure(self.camera.id).into_result()? };
        self.capture_state = CaptureState::Idle;
        Ok(())
    }
    pub fn get_roi(&mut self) -> Result<GenCamRoi, CameraError> {
        let (w, h) = unsafe { poa_call!(poa::get_roi_size(self.camera.id) @ w h) }?;
        let (x, y) = unsafe { poa_call!(poa::get_roi_start_pos(self.camera.id) @ x y) }?;
        Ok(GenCamRoi {
            x_min: x as _,
            y_min: y as _,
            width: w as _,
            height: h as _,
        })
    }
    pub fn set_roi(&mut self, roi: GenCamRoi, to: &mut GenCamRoi) -> Result<(), CameraError> {
        let GenCamRoi {
            x_min,
            y_min,
            width,
            height,
        } = roi;

        unsafe { poa::set_roi_size(self.camera.id, width.into(), height.into()).into_result()? };
        unsafe {
            poa::set_roi_start_pos(self.camera.id, x_min.into(), y_min.into()).into_result()?
        };
        *to = self.get_roi()?;
        Ok(())
    }
    pub fn download<'buf>(
        &mut self,
        out: &'buf mut Vec<u8>,
        roi: GenCamRoi,
    ) -> Result<GenericImageRef<'buf>, CameraError> {
        if self.capture_state != CaptureState::Ready {
            return Err(CameraError::NotReady);
        }
        let image_fmt = unsafe { poa_call!(poa::get_image_format(self.camera.id) @ out) }?
            .get()
            .map_err(|_| CameraError::UnknownImageFormat)?;
        // resize the buffer to fit the image format
        out.resize(image_fmt.buffer_size(roi.width as _, roi.height as _), 0);
        let (_, res) = Buffer::with(out, |buff, len| {
            // SAFETY: `Buffer<T>` has the same layout as `NonNull<T>` and we know this operation
            // won't uninitialize the buffer
            let buf: Buffer<MaybeUninit<u8>> = unsafe { std::mem::transmute(buff) };
            unsafe { poa::get_image_data(self.camera.id, buf, len as _, Millis(0)).into_result() }
        });
        res?;
        let temp = self
            .camera
            .get_config::<f64>(ConfigParameter::Temperature)
            .ok()
            .map(|(x, _)| x)
            .unwrap_or(-273.16);
        let now = SystemTime::now();
        let bayer = match self.properties.bayer_pattern.get() {
            Ok(poa::BayerPattern::Bg) => Some(BayerPattern::Bggr),
            Ok(poa::BayerPattern::Gb) => Some(BayerPattern::Gbrg),
            Ok(poa::BayerPattern::Gr) => Some(BayerPattern::Grbg),
            Ok(poa::BayerPattern::Rg) => Some(BayerPattern::Rggb),
            _ => None,
        };
        let colorspace = match image_fmt {
            ImageFormat::Raw8 | ImageFormat::Raw16 => ColorSpace::Gray,
            // I believe mono8 needs debayering according to the docs, but
            // the docs are weird
            ImageFormat::Mono8 | ImageFormat::Rgb24 => match bayer {
                Some(bayer) => ColorSpace::Bayer(bayer),
                None => ColorSpace::Rgb,
            },
        };

        let mut img = match image_fmt {
            ImageFormat::Mono8 | ImageFormat::Raw8 | ImageFormat::Rgb24 => {
                // This cannot error. If it does error, that means there's a bug.
                let img =
                    ImageRef::new(out, roi.width.into(), roi.height.into(), colorspace.clone())
                        .unwrap();
                GenericImageRef::new(now, img.into())
            }
            ImageFormat::Raw16 => {
                let img = ImageRef::new(
                    // this shouldn't panic since any reasonable allocator will
                    // align the allocation to more than 1
                    bytemuck::cast_slice_mut::<u8, u16>(out),
                    roi.width.into(),
                    roi.height.into(),
                    colorspace.clone(),
                )
                .unwrap();
                GenericImageRef::new(now, img.into())
            }
        };
        _ = img.insert_key("IMGSER", {
            let c = self.counter;
            self.counter += 1;
            c as u32
        });
        if let Ok((time, _)) = self
            .camera
            .get_config::<f64>(ConfigParameter::ExposureSeconds)
        {
            _ = img.insert_key(
                EXPOSURE_KEY,
                (Duration::from_secs_f64(time), "Exposure Time"),
            )
        }
        if let Ok((gain, _)) = self.camera.get_config::<i64>(ConfigParameter::Gain) {
            // wait? what unit is the gain in again?. The poa docs don't say and I'll just guess and fix it later if
            // I'm wrong.
            _ = img.insert_key("GAIN", (gain as f64 * 0.1, "Gain (dB)"));
        }
        if let Ok((egain, _)) = self.camera.get_config::<i64>(ConfigParameter::EGain) {
            _ = img.insert_key("ADU2ELEC", (egain, "Electrons per ADU (Sensor Bit Depth)"));
        }
        _ = img.insert_key("SENSORBPP", (self.properties.bit_depth, "Sensor bit depth"));
        _ = img.insert_key("XOFFSET", (roi.x_min, "X offset"));
        _ = img.insert_key("YOFFSET", (roi.y_min, "Y offset"));
        'b: {
            if let Ok(bin_idx) = unsafe { poa_call!(poa::get_image_bin(self.camera.id) @ out) } {
                let Some(id) = self
                    .properties
                    .binning_modes
                    .to_slice()
                    .get(bin_idx as usize)
                else {
                    break 'b;
                };
                let bin_size = id.id();
                _ = img.insert_key("XBINNING", (bin_size, "X binning"));
                _ = img.insert_key("YBINNING", (bin_size, "Y binning"));
            }
        }

        _ = img.insert_key("CCD-TEMP", (temp, "CCD temperature (C)"));
        _ = img.insert_key(
            "CAMERA",
            (
                self.properties.model_name.to_str_lossy().into_owned(),
                "Camera name",
            ),
        );
        _ = img.insert_key(
            "SERIAL",
            (
                self.properties.serial.to_str_lossy().into_owned(),
                "Camera serial number",
            ),
        );
        if colorspace != ColorSpace::Gray {
            _ = img.insert_key("XBAYOFF", (roi.x_min % 2, "X offset of Bayer pattern"));
            _ = img.insert_key("YBAYOFF", (roi.y_min % 2, "Y offset of Bayer pattern"));
        }
        self.capture_state = CaptureState::Idle;
        Ok(img)
    }
}
fn camera_properties_to_gencam(props: &CameraProperties) -> HashMap<GenCamCtrl, Property> {
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
            DeviceCtrl::Custom(const { CustomName::new("Product ID").unwrap() }),
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
                PropertyLims::PixelFmt {
                    variants: img_formats
                        .iter()
                        .copied()
                        .filter_map(map_pixel_format)
                        .collect(),
                    default: img_formats
                        .first()
                        .copied()
                        .and_then(map_pixel_format)
                        .unwrap_or(GenCamPixelBpp::Bpp8),
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
    let props_iter = sensor_props
        .into_iter()
        .map(|(ctl, prop)| (GenCamCtrl::from(ctl), prop))
        .chain(
            device_props
                .into_iter()
                .map(|(ctl, prop)| (GenCamCtrl::from(ctl), prop)),
        );

    HashMap::from_iter(props_iter)
}
fn map_pixel_format(fmt: MaybeInvalidImageFormat) -> Option<GenCamPixelBpp> {
    Some(
        match fmt.0.get().ok()? {
            ImageFormat::Mono8 | ImageFormat::Raw8 => GenCamPixelBpp::Bpp8,
            ImageFormat::Raw16 => GenCamPixelBpp::Bpp16,
            ImageFormat::Rgb24 => GenCamPixelBpp::Bpp24,
        }
        .to_owned(),
    )
}

/// A handle to an owned camera. This is used both for the info and main camera struct
#[derive(Clone, Debug)]
pub struct Handle {
    pub(crate) inner: Arc<Mutex<HandleInner>>,
    // we need to store this here beause list_properties needs to
    // be able to return a reference
    pub(crate) gencam_props: Arc<HashMap<GenCamCtrl, Property>>,
}

impl Handle {
    pub fn open_and_init(props: &CameraProperties) -> Result<Self, CameraError> {
        let inner = HandleInner::open_and_init(props)?;
        let gencam_props = inner.gencam_props.clone();
        Ok(Self {
            inner: Arc::new(inner.into()),
            gencam_props,
        })
    }
}
