use std::{ffi::c_long, time::Duration};

use generic_camera::{GenCamCtrl, Property, PropertyValue, property::PropertyLims};
use player_one_camera_sys::{
    Bool, ConfigAttributes, ConfigParameter, ConfigValue, ConfigValueKind,
};
/// # Safety
/// Common sense
pub(crate) unsafe trait ConfigVal: Sized + Copy {
    const KIND: ConfigValueKind;
    unsafe fn from_value(value: ConfigValue) -> Self;
    fn to_value(self) -> ConfigValue;
}

unsafe impl ConfigVal for c_long {
    const KIND: ConfigValueKind = ConfigValueKind::Int;
    unsafe fn from_value(value: ConfigValue) -> Self {
        // SAFETY: Caller must uphold the active member being `int_value`
        unsafe { value.int_value }
    }
    fn to_value(self) -> ConfigValue {
        ConfigValue { int_value: self }
    }
}

unsafe impl ConfigVal for bool {
    const KIND: ConfigValueKind = ConfigValueKind::Bool;
    unsafe fn from_value(value: ConfigValue) -> Self {
        // SAFETY: Caller must uphold the active member being `bool_value`
        unsafe { value.bool_value.into_bool() }
    }
    fn to_value(self) -> ConfigValue {
        ConfigValue {
            bool_value: self.into(),
        }
    }
}

unsafe impl ConfigVal for f64 {
    const KIND: ConfigValueKind = ConfigValueKind::Float;
    unsafe fn from_value(value: ConfigValue) -> Self {
        // SAFETY: Caller must uphold the active member being `float_value`
        unsafe { value.float_value }
    }
    fn to_value(self) -> ConfigValue {
        ConfigValue { float_value: self }
    }
}

/// How to marshal data from POA's format and gencam's property format
/// and vice versa, including type changes and unit conversions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "We have the logic for handling the variants and might use them later"
)]
pub enum PropMarshalling {
    /// Direct type conversion
    Identity,
    /// POA expects int, gencam expects unsigned
    Int2Unsigned,
    /// POA expects int, gencam expects float, same unit, step 1
    Int2Float,
    /// POA expects int ms, gencam expects duration, step 1ms
    IntMillis,
    /// POA expects int μs, gencam expects duration, step 1μs
    IntMicros,
    /// POA expects float seconds, gencam expects duration
    FloatSeconds,
    /// POA gives in Hz, gencam expects duration
    IntHertz2Duration,
    /// POA expects bool, Gencam expects negated bool
    Negated,
}

impl PropMarshalling {
    pub unsafe fn poa2gencam_quantity(
        self,
        kind: ConfigValueKind,
        val: ConfigValue,
    ) -> Option<PropertyValue> {
        unsafe {
            Some(match (self, kind) {
                (Self::Negated, ConfigValueKind::Bool) => (!(val.bool_value.into_bool())).into(),
                (_, ConfigValueKind::Bool) => val.bool_value.into_bool().into(),
                (Self::Int2Float, ConfigValueKind::Int) => (val.int_value as f64).into(),
                (Self::IntMicros, ConfigValueKind::Int) => {
                    Duration::from_micros(val.int_value as _).into()
                }
                (Self::IntMillis, ConfigValueKind::Int) => {
                    Duration::from_millis(val.int_value as _).into()
                }
                (Self::FloatSeconds, ConfigValueKind::Float) => {
                    Duration::from_secs_f64(val.float_value).into()
                }
                (Self::Int2Unsigned, ConfigValueKind::Int) => (val.int_value as u64).into(),
                // You can't really put infinity on a slider and it wouldn't make sense
                // so map 0 to no limit instead.
                (Self::IntHertz2Duration, ConfigValueKind::Int) => if val.int_value == 0 {
                    Duration::ZERO
                } else {
                    Duration::from_secs_f64((val.int_value as f64).recip())
                }
                .into(),
                // (Self::Identity, ConfigValueKind::Bool) => val.bool_value.into_bool().into(),
                (Self::Identity, ConfigValueKind::Int) => val.int_value.into(),
                (Self::Identity, ConfigValueKind::Float) => val.float_value.into(),
                _ => return None,
            })
        }
    }
    pub fn gencam2poa_quantity(self, val: PropertyValue) -> Option<(ConfigValueKind, ConfigValue)> {
        Some(match (self, val) {
            (Self::Negated, PropertyValue::Bool(val)) => (
                ConfigValueKind::Bool,
                ConfigValue {
                    bool_value: Bool::new(!val),
                },
            ),
            (_, PropertyValue::Bool(val)) => (
                ConfigValueKind::Bool,
                ConfigValue {
                    bool_value: val.into(),
                },
            ),
            (Self::Int2Float, PropertyValue::Float(v)) => {
                (ConfigValueKind::Int, ConfigValue { int_value: v as _ })
            }
            (Self::IntMicros, PropertyValue::Duration(v)) => (
                ConfigValueKind::Int,
                ConfigValue {
                    int_value: v.as_micros().try_into().ok()?,
                },
            ),
            (Self::IntMillis, PropertyValue::Duration(v)) => (
                ConfigValueKind::Int,
                ConfigValue {
                    int_value: v.as_millis().try_into().ok()?,
                },
            ),
            (Self::FloatSeconds, PropertyValue::Duration(v)) => (
                ConfigValueKind::Float,
                ConfigValue {
                    float_value: v.as_secs_f64(),
                },
            ),
            (Self::Int2Unsigned, PropertyValue::Unsigned(v)) => (
                ConfigValueKind::Int,
                ConfigValue {
                    int_value: v.try_into().ok()?,
                },
            ),
            (Self::Int2Unsigned, PropertyValue::Int(v)) => {
                (ConfigValueKind::Int, ConfigValue { int_value: v })
            }
            (Self::Identity, PropertyValue::Int(v)) => {
                (ConfigValueKind::Int, ConfigValue { int_value: v })
            }
            (Self::IntHertz2Duration, PropertyValue::Duration(d)) => {
                if d == Duration::ZERO {
                    (ConfigValueKind::Int, ConfigValue { int_value: 0 })
                } else {
                    (
                        ConfigValueKind::Int,
                        ConfigValue {
                            int_value: d.as_secs_f64().recip() as i64,
                        },
                    )
                }
            }
            (Self::Identity, PropertyValue::Float(f)) => {
                (ConfigValueKind::Float, ConfigValue { float_value: f })
            }
            // (Self::Identity, PropertyValue::Bool(b)) => (
            //     ConfigValueKind::Bool,
            //     ConfigValue {
            //         bool_value: b.into(),
            //     },
            // ),
            _ => return None,
        })
    }
    pub fn step(self, ty: ConfigValueKind) -> Option<PropertyValue> {
        Some(match self {
            Self::FloatSeconds => Duration::from_micros(1).into(),
            Self::Int2Float => 1i64.into(),
            Self::Identity => match ty {
                ConfigValueKind::Bool => return None,
                ConfigValueKind::Float => 1e-6f64.into(),
                ConfigValueKind::Int => 1i64.into(),
            },
            Self::Int2Unsigned => 1u64.into(),
            Self::IntMicros => Duration::from_micros(1).into(),
            Self::IntMillis => Duration::from_millis(1).into(),
            // this step is non-linear, but gotta do our best.
            // The only use of this has a range of 2000
            Self::IntHertz2Duration => Duration::from_secs_f64(2000.0f64.recip()).into(),
            Self::Negated => return None,
        })
    }
    pub unsafe fn poa2gencam_lims(
        self,
        ty: ConfigValueKind,
        default: ConfigValue,
        min: ConfigValue,
        max: ConfigValue,
    ) -> Option<PropertyLims> {
        // these must be functions since if the property is a Bool, we cannot guarantee that
        // the read value is valid for a `Bool` since it might be an arbitrary value
        let min = move || unsafe { self.poa2gencam_quantity(ty, min) };
        let max = move || unsafe { self.poa2gencam_quantity(ty, max) };
        let default = unsafe { self.poa2gencam_quantity(ty, default) }?;
        Some(match default {
            PropertyValue::Bool(b) => PropertyLims::Bool { default: b },
            PropertyValue::Duration(d) => {
                let mut min = min()?.try_into().ok()?;
                let mut max = max()?.try_into().ok()?;
                if self == PropMarshalling::IntHertz2Duration {
                    (min, max) = (Duration::ZERO, min)
                }
                PropertyLims::Duration {
                    min,
                    max,
                    step: self.step(ty)?.try_into().ok()?,
                    default: d,
                }
            }
            PropertyValue::Float(f) => PropertyLims::Float {
                min: min()?.try_into().ok()?,
                max: max()?.try_into().ok()?,
                step: self.step(ty)?.try_into().ok()?,
                default: f,
            },
            PropertyValue::Int(i) => PropertyLims::Int {
                min: min()?.try_into().ok()?,
                max: max()?.try_into().ok()?,
                step: self.step(ty)?.try_into().ok()?,
                default: i,
            },
            PropertyValue::Unsigned(u) => PropertyLims::Unsigned {
                min: min()?.try_into().ok()?,
                max: max()?.try_into().ok()?,
                step: self.step(ty)?.try_into().ok()?,
                default: u,
            },
            _ => return None,
        })
    }
    pub fn poa2gencam_property(self, attrs: ConfigAttributes) -> Option<Property> {
        let ty = attrs.value_type.get().ok()?;
        let lims = unsafe {
            self.poa2gencam_lims(ty, attrs.default_value, attrs.min_value, attrs.max_value)?
        };
        let mut prop = Property::new(
            lims,
            attrs.supports_auto.into_bool(),
            (!attrs.writable.into_bool()) & attrs.readable.into_bool(),
        );

        prop.set_doc(attrs.description.to_str_lossy());
        Some(prop)
    }
}

macro_rules! define_marshalling {
    (
        $(
            $poa_param:ident <=> $gencam_ctrl_kind:ident($gencam_ctrl:expr) with $marshalling:ident
        ),*
    ) => {
        pub const fn poa2gencam_ctrl(poa: ConfigParameter) -> Option<(GenCamCtrl, PropMarshalling)> {
            Some(match poa {
                $(
                    ConfigParameter::$poa_param => (const { GenCamCtrl::$gencam_ctrl_kind($gencam_ctrl) }, PropMarshalling::$marshalling),
                )*
                _ => return None
            })
        }
        pub fn gencam2poa_param(gencam: GenCamCtrl) -> Option<(ConfigParameter, PropMarshalling)> {
            Some(match gencam {
                $(
                   GenCamCtrl::$gencam_ctrl_kind(e) if  const { $gencam_ctrl } == e => (ConfigParameter::$poa_param, PropMarshalling::$marshalling),)*
                _ => return None
            })
        }
    };
}

use generic_camera::controls::*;
define_marshalling! {
    ExposureMicros <=> Exposure(ExposureCtrl::ExposureTime) with IntMicros,
    ExposureSeconds <=> Exposure(ExposureCtrl::ExposureTime) with FloatSeconds,
    AutoExposureMaxExposure <=> Exposure(ExposureCtrl::AutoMaxExposure) with IntMillis,
    AutoExposureBrightness <=> Exposure(ExposureCtrl::AutoTargetBrightness) with Int2Float,
    AutoExposureMaxGain <=> Exposure(ExposureCtrl::AutoMaxGain) with Int2Float,
    // NOTE: these flip attributes need special handling in
    // the setter cases because it is super weird
    FlipHorizontal <=> Sensor(SensorCtrl::ReverseX) with Identity,
    FlipVertical <=> Sensor(SensorCtrl::ReverseY) with Identity,
    // NOTE: there is only a single temperature source.
    // Need also to make the corresponding TemperatureSelector command
    Temperature <=> Device(DeviceCtrl::Temperature) with Identity,
    Cooler <=> Device(DeviceCtrl::CoolerEnable) with Identity,
    CoolerPower <=> Device(DeviceCtrl::CoolerPower) with Int2Float,
    HeaterPower <=> Device(DeviceCtrl::Custom(CustomName::new("Heater Power").unwrap())) with Int2Float,
    FanPower <=> Device(DeviceCtrl::Custom(CustomName::new("Fan Power").unwrap())) with Int2Float,
    TargetTemp <=> Device(DeviceCtrl::Custom(CustomName::new("Target Temp").unwrap())) with Int2Float,

    Gain <=> Analog(AnalogCtrl::Gain) with Int2Float,
    FrameLimit <=> FrameTime(FrameTimeCtrl::FrameTime) with IntHertz2Duration,
    Hqi <=> Sensor(SensorCtrl::Custom(CustomName::new("HQI").unwrap())) with Identity,
    HardwareBin <=> Sensor(SensorCtrl::Custom(CustomName::new("Hard Bin").unwrap())) with Identity
}
