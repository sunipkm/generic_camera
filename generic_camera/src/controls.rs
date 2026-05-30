/*!
 * # Controls
 * Defines standard controls for cameras.
 */
#[allow(unused_imports)]
use crate::PropertyType;
use documented::{Documented, DocumentedVariants};
use senti::cstring::BoundedCString;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq, PartialOrd, Ord)]
/// A custom name for a control.
///
/// This is a 16-byte array that can be used to store a custom name for a control.
/// It must be non-empty and made solely out of the ASCII alphanumerics and the
/// special characters `-_# `.
///
///
/// # Example
/// ```
/// use generic_camera::controls::CustomName;
///
/// let name: CustomName = CustomName::new("UUID").unwrap();
/// assert_eq!(name.as_str(), "UUID");
///
/// let name: Option<CustomName> = CustomName::new("My Custom Very Long Name");
/// assert_eq!(name, None);
///
/// const MY_COOL_NAME: CustomName = CustomName::new("Product ID").unwrap();
/// assert_eq!(MY_COOL_NAME.as_str(), "Product ID")
/// ```
pub struct CustomName(BoundedCString<16>);

impl CustomName {
    /// Create a new custom name, returning `None` if the string is too long, empty,
    /// or not made from ASCII alphanumerics and the characters
    /// `-_# `.
    ///
    /// <div class="warning">
    /// Currently, if the string contains a null byte, it will be truncated to before
    /// that null byte since internally, for convenience,
    /// we use a bounded C-style string that is terminated if and only if the length is not
    /// the capacity. Do not rely on this behavior though.
    /// </div>
    pub const fn new(name: &str) -> Option<Self> {
        let Some(inner) = BoundedCString::from_bytes(name.as_bytes()) else {
            return None;
        };
        // I don't like this since it has to compute the length a second time, but
        // it really doesn't matter
        let bytes = inner.to_bytes();
        if bytes.is_empty() {
            return None;
        }
        // validate
        let mut i = 0;
        while i < bytes.len() {
            if !bytes[i].is_ascii_alphanumeric() && !matches!(bytes[i], b'_' | b'-' | b' ' | b'#') {
                return None;
            }
            i += 1;
        }

        Some(Self(inner))
    }

    /// Get the custom name as a string.
    pub fn as_str(&self) -> &str {
        // SAFETY: The constructor ensures that we are valid UTF-8
        // by ensuring that the string is only made up of specific ASCII characters
        unsafe { self.0.to_str_unchecked() }
    }
}
impl Serialize for CustomName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.as_str().serialize(serializer)
    }
}
use serde::de::Error;
impl<'de> Deserialize<'de> for CustomName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let res = <&'de str>::deserialize(deserializer)?;
        if res.len() > 16 {
            return Err(D::Error::custom("Custom name must be <16 bytes"));
        }
        if res.is_empty() {
            return Err(D::Error::custom("Custom name must not be empty"));
        }
        // This is more strict than the constructor, but deserializing shouldn't
        // allow implicit truncation.
        if res.as_bytes().contains(&b'\0') {
            return Err(D::Error::custom("Custom name must not contain a null byte"));
        }
        let Some(res) = Self::new(res) else {
            return Err(D::Error::custom(
                "Custom name must only contain ASCII alphanumerics, spaces, and any of `#-_`",
            ));
        };
        Ok(res)
    }
}
/// Describes device-specific control options.
#[derive(
    Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Documented, DocumentedVariants,
)]
#[non_exhaustive]
pub enum DeviceCtrl {
    /// Query line or area scan type, usually [`PropertyType::EnumStr`]
    ScanType,
    /// Query device vendor ([`PropertyType::EnumStr`])
    VendorName,
    /// Query device model ([`PropertyType::EnumStr`])
    ModelName,
    /// Query device family ([`PropertyType::EnumStr`])
    FamilyName,
    /// Query manufacturer information ([`PropertyType::EnumStr`])
    MfgInfo,
    /// Query version ([`PropertyType::EnumStr`])
    Version,
    /// Query firmware version ([`PropertyType::EnumStr`])
    FwVersion,
    /// Query serial number ([`PropertyType::EnumStr`])
    SerialNumber,
    /// Query unique ID ([`PropertyType::EnumStr`])
    Id,
    /// Query user-set ID ([`PropertyType::EnumStr`])
    UserId,
    /// Query transport layer type ([`PropertyType::EnumStr`])
    TlType,
    /// Select device temperature source ([`PropertyType::EnumStr`])
    TemperatureSelector,
    /// Query selected temperature ([`PropertyType::Float`])
    Temperature,
    /// Reset device ([`PropertyType::Command`])
    Reset,
    /// Configure the cooler temperature ([`PropertyType::Float`])
    CoolerTemp,
    /// Configure the cooler power ([`PropertyType::Float`])
    CoolerPower,
    /// Enable or disable the cooler ([`PropertyType::Bool`])
    CoolerEnable,
    /// Configure high speed mode ([`PropertyType::Bool`])
    HighSpeedMode,
    /// Configure device fan ([`PropertyType::Bool`])
    FanToggle,
    /// A custom command
    Custom(CustomName),
}

/// Describes sensor-specific control options.
#[derive(
    Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Documented, DocumentedVariants,
)]
#[non_exhaustive]
pub enum SensorCtrl {
    /// Query pixel width ([`PropertyType::Float`])
    PixelWidth,
    /// Query pixel height ([`PropertyType::Float`])
    PixelHeight,
    /// Query sensor name ([`PropertyType::EnumStr`])
    Name,
    /// Query sensor shutter mode ([`PropertyType::EnumStr`])
    ShutterMode,
    /// Query sensor max width ([`PropertyType::Unsigned`])
    WidthMax,
    /// Query sensor max height ([`PropertyType::Unsigned`])
    HeightMax,
    /// Query the binning method ([`PropertyType::EnumStr`])
    BinningSelector,
    /// Query binning factor on both axes ([`PropertyType::EnumUnsigned`] or [`PropertyType::Unsigned`])
    BinningBoth,
    /// Query the horizontal binning mode ([`PropertyType::EnumStr`])
    BinningHorzlMode,
    /// Query the vertical binning mode ([`PropertyType::EnumStr`])
    BinningVertMode,
    /// Query the horizontal binning factor ([`PropertyType::Unsigned`] or [`PropertyType::EnumUnsigned`])
    BinningHorz,
    /// Query the vertical binning factor ([`PropertyType::Unsigned`] or [`PropertyType::EnumUnsigned`])
    BinningVert,
    /// Query the horizontal decimation method ([`PropertyType::EnumStr`])
    DecimationHorzMode,
    /// Query the horizontal decimation mode ([`PropertyType::EnumStr`])
    DecimationHorz,
    /// Query the vertical decimation method ([`PropertyType::EnumStr`])
    DecimationVertMode,
    /// Query the vertical decimation mode ([`PropertyType::EnumStr`])
    DecimationVert,
    /// Reverse the image about the X axis ([`PropertyType::Bool`])
    ReverseX,
    /// Reverse the image about the Y axis ([`PropertyType::Bool`])
    ReverseY,
    /// Query the pixel format ([`PropertyType::EnumStr`])
    PixelFormat,
    /// Apply a test pattern to the image ([`PropertyType::EnumStr`])
    TestPattern,
    /// A custom command
    Custom(CustomName),
}

/// Describes trigger-specific control options.
#[derive(
    Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Documented, DocumentedVariants,
)]
#[non_exhaustive]
pub enum TriggerCtrl {
    /// Select trigger line ([`PropertyType::EnumStr`])
    Sel,
    /// Get or set trigger mode on the selected trigger line ([`PropertyType::EnumStr`])
    Mod,
    /// Get or set trigger source on the selected trigger line ([`PropertyType::EnumStr`])
    Src,
    /// Get or set the type trigger overlap permitted with the previous frame or line ([`PropertyType::EnumStr`])
    Overlap,
    /// Specifies the delay in microseconds (us) to apply after the trigger reception before activating it ([`PropertyType::Float`])
    Delay,
    /// Specifies a division factor for the incoming trigger pulses ([`PropertyType::Float`])
    Divider,
    /// Specifies a multiplication factor for the incoming trigger pulses ([`PropertyType::Float`])
    Multiplier,
    /// A custom command
    Custom(CustomName),
}

/// Describes exposure control options.
#[derive(
    Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Documented, DocumentedVariants,
)]
#[non_exhaustive]
pub enum ExposureCtrl {
    /// Select exposure mode ([`PropertyType::EnumStr`])
    Mode,
    /// Select exposure time ([`PropertyType::Float`])
    ExposureTime,
    /// Select exposure auto mode ([`PropertyType::EnumStr`] or [`PropertyType::Bool`])
    Auto,
    /// Select maximum auto exposure time ([`PropertyType::Duration`])
    AutoMaxExposure,
    /// Select exposure auto target brightness ([`PropertyType::Float`])
    AutoTargetBrightness,
    /// Select maximum gain for auto exposure ([`PropertyType::Float`])
    AutoMaxGain,
    /// A custom command
    Custom(CustomName),
}

/// Describes frame rate control options.
#[derive(
    Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Documented, DocumentedVariants,
)]
#[non_exhaustive]
pub enum FrameTimeCtrl {
    /// Select frame time mode ([`PropertyType::EnumStr`])
    Mode,
    /// Select frame time ([`PropertyType::Duration`])
    FrameTime,
    /// Select frame time auto mode ([`PropertyType::EnumStr`] or [`PropertyType::Bool`])
    Auto,
    /// A custom command
    Custom(CustomName),
}

#[derive(
    Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Documented, DocumentedVariants,
)]
#[non_exhaustive]
/// Describes analog control options.
pub enum AnalogCtrl {
    /// Select which gain to control ([`PropertyType::EnumStr`])
    GainSelector,
    /// Select gain value ([`PropertyType::Float`])
    Gain,
    /// Select gain auto mode ([`PropertyType::EnumStr`] or [`PropertyType::Bool`])
    GainAuto,
    /// Select gain auto balance ([`PropertyType::Float`])
    GainAutoBalance,
    /// Select which black level to control ([`PropertyType::EnumStr`])
    BlackLevelSel,
    /// Select black level value ([`PropertyType::Float`])
    BlackLevel,
    /// Select black level auto mode ([`PropertyType::EnumStr`] or [`PropertyType::Bool`])
    BlackLevelAuto,
    /// Select black level auto balance ([`PropertyType::Float`])
    BlackLevelAutoBalance,
    /// Select which white clip to control ([`PropertyType::EnumStr`])
    WhiteClipSel,
    /// Select white clip value ([`PropertyType::Float`])
    WhiteClip,
    /// Select white balance ratio mode ([`PropertyType::EnumStr`])
    BalanceRatioSel,
    /// Configure white balance ratio value ([`PropertyType::Float`])
    BalanceRatio,
    /// Configure white balance ratio auto mode ([`PropertyType::EnumStr`] or [`PropertyType::Bool`])
    BalanceWhiteAuto,
    /// Configure gamma value ([`PropertyType::Float`])
    Gamma,
    /// A custom command
    Custom(CustomName),
}

#[derive(
    Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Documented, DocumentedVariants,
)]
#[non_exhaustive]
/// Describes digital I/O control options.
pub enum DigitalIoCtrl {
    /// Select which line to control ([`PropertyType::EnumStr`])
    LineSel,
    /// Select the line mode ([`PropertyType::EnumStr`])
    LineMod,
    /// Line I/O inversion ([`PropertyType::Bool`] or [`PropertyType::EnumStr`])
    LineInvert,
    /// Query line status ([`PropertyType::EnumStr`])
    LineStat,
    /// Configure the line signal source ([`PropertyType::EnumStr`])
    LineSrc,
    /// Configure as user output selector ([`PropertyType::EnumStr`] or [`PropertyType::Bool`])
    UserOutSel,
    /// Configure as user output value ([`PropertyType::Float`] or [`PropertyType::Bool`])
    UserOutVal,
    /// A custom command
    Custom(CustomName),
}

#[derive(
    Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Documented, DocumentedVariants,
)]
#[non_exhaustive]
/// Describes the general camera control zones.
pub enum GenCamCtrl {
    /// Device-specific control options.
    Device(DeviceCtrl),
    /// Sensor-specific control options.
    Sensor(SensorCtrl),
    /// Trigger-specific control options.
    Trigger(TriggerCtrl),
    /// Exposure-specific control options.
    Exposure(ExposureCtrl),
    /// Frame rate-specific control options.
    FrameTime(FrameTimeCtrl),
    /// Analog-specific control options.
    Analog(AnalogCtrl),
    /// Digital I/O-specific control options.
    DigitalIo(DigitalIoCtrl),
}

macro_rules! impl_from_ctrl {
    ($ctrl:ident, $variant:ident) => {
        impl From<$ctrl> for GenCamCtrl {
            fn from(ctrl: $ctrl) -> Self {
                GenCamCtrl::$variant(ctrl)
            }
        }
    };
}

impl_from_ctrl!(DeviceCtrl, Device);
impl_from_ctrl!(SensorCtrl, Sensor);
impl_from_ctrl!(TriggerCtrl, Trigger);
impl_from_ctrl!(ExposureCtrl, Exposure);
impl_from_ctrl!(FrameTimeCtrl, FrameTime);
impl_from_ctrl!(AnalogCtrl, Analog);
impl_from_ctrl!(DigitalIoCtrl, DigitalIo);

/// Trait for controls that have a tooltip.
pub trait ToolTip {
    /// The tooltip for this control.
    fn tooltip(&self) -> &'static str;
}

macro_rules! impl_tooltip {
    ($ctrl:ident) => {
        impl ToolTip for $ctrl {
            fn tooltip(&self) -> &'static str {
                self.get_variant_docs().unwrap()
            }
        }
    };
}

impl_tooltip!(DeviceCtrl);
impl_tooltip!(SensorCtrl);
impl_tooltip!(TriggerCtrl);
impl_tooltip!(ExposureCtrl);
impl_tooltip!(FrameTimeCtrl);
impl_tooltip!(AnalogCtrl);
impl_tooltip!(DigitalIoCtrl);
impl_tooltip!(GenCamCtrl);
