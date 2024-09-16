#[allow(unused_imports)]
use crate::PropertyType;
use documented::{Documented, DocumentedVariants};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Hash, Eq)]
/// A custom name for a control.
///
/// This is a 32-byte array that can be used to store a custom name for a control.
///
/// # Note
/// The name is trimmed to 32 bytes, so it is possible that the name is truncated.
///
/// # Example
/// ```
/// use generic_camera::CustomName;
///
/// let name: CustomName = "UUID".into();
/// assert_eq!(name.as_str(), "UUID");
///
/// let name: CustomName = "My Custom Very Long Name".into();
/// assert_eq!(name.as_str(), "My Custom Very L");
/// ```
pub struct CustomName([u8; 16]);

impl CustomName {
    /// Create a new custom name.
    fn new(name: &str) -> Self {
        let mut bytes = [0; 16];
        let len = name.len().min(16);
        bytes[..len].copy_from_slice(&name.as_bytes()[..len]);
        Self(bytes)
    }

    /// Get the custom name as a string.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0)
            .unwrap() // This is safe because the array is always valid UTF-8
            .trim_end_matches(char::from(0))
    }
}

impl<'a, T: Into<&'a str>> From<T> for CustomName {
    fn from(name: T) -> Self {
        Self::new(name.into())
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
