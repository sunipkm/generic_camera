use std::time::Duration;

use crate::{GenCamError, Result};
use serde::{Deserialize, Serialize};

/// Describes device-specific control options.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DeviceCtrl {
    /// Query line or area scan type, usually [`PropertyType::EnumString`]
    ScanType,
    /// Query device vendor ([`PropertyType::Str`])
    VendorName,
    /// Query device model ([`PropertyType::Str`])
    ModelName,
    /// Query device family ([`PropertyType::Str`])
    FamilyName,
    /// Query manufacturer information ([`PropertyType::Str`])
    MfgInfo,
    /// Query version ([`PropertyType::Str`])
    Version,
    /// Query firmware version ([`PropertyType::Str`])
    FwVersion,
    /// Query serial number ([`PropertyType::Str`])
    SerialNumber,
    /// Query unique ID ([`PropertyType::Str`])
    Id,
    /// Query user-set ID ([`PropertyType::Str`])
    UserId,
    /// Query transport layer type ([`PropertyType::Str`])
    TlType,
    /// Select device temperature source ([`PropertyType::EnumString`])
    TemperatureSelector,
    /// Query selected temperature ([`PropertyType::Float`])
    Temperature,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SensorCtrl {
    /// Query pixel width ([`PropertyType::Float`])
    PixelWidth,
    /// Query pixel height ([`PropertyType::Float`])
    PixelHeight,
    /// Query sensor name ([`PropertyType::Str`])
    Name,
    /// Query sensor shutter mode ([`PropertyType::EnumStr`])
    ShutterMode,
    /// Query sensor max width ([`PropertyType::Unsigned`])
    WidthMax,
    /// Query sensor max height ([`PropertyType::Unsigned`])
    HeightMax,
    BinningSelector,
    BinningHorzlMode,
    BinningVertMode,
    BinningHorz,
    BinningVert,
    DecimationHorzMode,
    DecimationHorz,
    DecimationVertMode,
    DecimationVert,
    ReverseX,
    ReverseY,
    PixelFormat,
    TestPattern,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TriggerCtrl {
    Sel,
    Mod,
    Src,
    Act,
    Overlap,
    Delay,
    Divider,
    Multiplier,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ExposureCtrl {
    Mode,
    TimeMode,
    TimeSelector,
    ExposureTime,
    Auto,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AnalogCtrl {
    GainSelector,
    Gain,
    GainAuto,
    GainAutoBalance,
    BlackLevelSel,
    BlackLevel,
    BlackLevelAuto,
    BlackLevelAutoBalance,
    WhiteClipSel,
    WhiteClip,
    BalanceRatioSel,
    BalanceRatio,
    BalanceWhiteAuto,
    Gamma,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DigitalIoCtrl {
    LineSel,
    LineMod,
    LineInvert,
    LineStat,
    LineSrc,
    UserOutSel,
    UserOutVal,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GenCamCtrl {
    Device(DeviceCtrl),
    Sensor(SensorCtrl),
    Trigger(TriggerCtrl),
    Analog(AnalogCtrl),
    DigitalIo(DigitalIoCtrl),
}


/// A generic property trait that abstracts the different types of properties
/// TODO: Derive macro for this trait to reduce boilerplate. Read: [argh](https://github.com/google/argh), [thiserror](https://github.com/dtolnay/thiserror)
pub trait GenericProperty {
    /// Get the name of the property
    fn get_name(&self) -> &str;
    /// Get the description of the property
    fn get_desc(&self) -> &str;
    /// Get the property from a string
    fn get_property(name: &str) -> Result<Self>
    where
        Self: Sized;
    /// Get the property name as a [`PropertyName`]
    fn get_propname(&self) -> PropertyName {
        PropertyName::from_str(self.get_name())
    }
    /// Get the property type as a [`PropertyType`]
    fn get_proptype(&self) -> PropertyType;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Hash)]
pub(crate) enum PrivPropertyName {
    Stack([u8; 32]),
    Heap(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Hash)]
/// A property name
pub struct PropertyName(PrivPropertyName); // private

impl PropertyName {
    /// Get the name as a string
    pub fn as_str(&self) -> &str {
        match &self.0 {
            // Safety: The name is constructed from a string, so it is guaranteed to be valid utf8
            PrivPropertyName::Stack(name) => std::str::from_utf8(name)
                .expect("Error unpacking string in PrivPropertyName::Stack")
                .trim_matches(char::from(0)),
            PrivPropertyName::Heap(name) => name.as_str(),
        }
    }
    /// Create a new property name from a string
    fn from_str(name: &str) -> Self {
        if name.len() <= 32 {
            let mut name_arr = [0; 32];
            name_arr[..name.len()].copy_from_slice(name.as_bytes());
            PropertyName(PrivPropertyName::Stack(name_arr))
        } else {
            PropertyName(PrivPropertyName::Heap(name.to_string()))
        }
    }
}

impl<'a, T: Into<&'a str>> From<T> for PropertyName {
    fn from(name: T) -> Self {
        PropertyName::from_str(name.into())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// A property
pub struct Property {
    name: PropertyName,
    prop: PropertyStor,
}

impl Property {
    /// Create a new property
    pub fn new(name: &str, prop: PropertyStor) -> Self {
        Property {
            name: PropertyName::from_str(name),
            prop,
        }
    }

    /// Get the name of the property
    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }

    /// Get the type of the property
    pub fn get_type(&self) -> PropertyType {
        (&self.prop).into()
    }

    /// Get the minimum value of the property
    pub fn get_min(&self) -> Result<PropertyValue> {
        match &self.prop {
            PropertyStor::Bool(_) => Err(GenCamError::PropertyNotNumber),
            PropertyStor::Int(prop) => Ok(prop.get_min()?.into()),
            PropertyStor::Float(prop) => Ok(prop.get_min()?.into()),
            PropertyStor::Unsigned(prop) => Ok(prop.get_min()?.into()),
            PropertyStor::Duration(prop) => Ok(prop.get_min()?.into()),
            PropertyStor::Str(_) => Err(GenCamError::PropertyNotNumber),
            PropertyStor::EnumStr(_) => Err(GenCamError::PropertyNotNumber),
            PropertyStor::EnumInt(prop) => Ok(prop.get_min()?.into()),
            PropertyStor::EnumUnsigned(prop) => Ok(prop.get_min()?.into()),
        }
    }

    /// Get the maximum value of the property
    pub fn get_max(&self) -> Result<PropertyValue> {
        match &self.prop {
            PropertyStor::Bool(_) => Err(GenCamError::PropertyNotNumber),
            PropertyStor::Int(prop) => Ok(prop.get_max()?.into()),
            PropertyStor::Float(prop) => Ok(prop.get_max()?.into()),
            PropertyStor::Unsigned(prop) => Ok(prop.get_max()?.into()),
            PropertyStor::Duration(prop) => Ok(prop.get_max()?.into()),
            PropertyStor::Str(_) => Err(GenCamError::PropertyNotNumber),
            PropertyStor::EnumStr(_) => Err(GenCamError::PropertyNotNumber),
            PropertyStor::EnumInt(_) => Err(GenCamError::PropertyIsEnum),
            PropertyStor::EnumUnsigned(_) => Err(GenCamError::PropertyIsEnum),
        }
    }

    /// Get the step value of the property
    pub fn get_step(&self) -> Result<PropertyValue> {
        match &self.prop {
            PropertyStor::Bool(_) => Err(GenCamError::PropertyNotNumber),
            PropertyStor::Int(prop) => Ok(prop.get_step()?.into()),
            PropertyStor::Float(prop) => Ok(prop.get_step()?.into()),
            PropertyStor::Unsigned(prop) => Ok(prop.get_step()?.into()),
            PropertyStor::Duration(prop) => Ok(prop.get_step()?.into()),
            PropertyStor::Str(_) => Err(GenCamError::PropertyNotNumber),
            PropertyStor::EnumStr(_) => Err(GenCamError::PropertyNotNumber),
            PropertyStor::EnumInt(_) => Err(GenCamError::PropertyIsEnum),
            PropertyStor::EnumUnsigned(_) => Err(GenCamError::PropertyIsEnum),
        }
    }

    /// Get the default value of the property
    pub fn get_default(&self) -> Result<PropertyValue> {
        match &self.prop {
            PropertyStor::Bool(prop) => Ok(prop.get_default()?.into()),
            PropertyStor::Int(prop) => Ok(prop.get_default()?.into()),
            PropertyStor::Float(prop) => Ok(prop.get_default()?.into()),
            PropertyStor::Unsigned(prop) => Ok(prop.get_default()?.into()),
            PropertyStor::Duration(prop) => Ok(prop.get_default()?.into()),
            PropertyStor::Str(prop) => Ok(prop.get_default()?.into()),
            PropertyStor::EnumStr(prop) => Ok(prop.get_default()?.into()),
            PropertyStor::EnumInt(prop) => Ok(prop.get_default()?.into()),
            PropertyStor::EnumUnsigned(prop) => Ok(prop.get_default()?.into()),
        }
    }

    /// Get the variants of the property
    pub fn get_variants(&self) -> Result<Vec<PropertyValue>> {
        match &self.prop {
            PropertyStor::Bool(_) => Err(GenCamError::PropertyNotEnum),
            PropertyStor::Int(_) => Err(GenCamError::PropertyNotEnum),
            PropertyStor::Float(_) => Err(GenCamError::PropertyNotEnum),
            PropertyStor::Unsigned(_) => Err(GenCamError::PropertyNotEnum),
            PropertyStor::Duration(_) => Err(GenCamError::PropertyNotEnum),
            PropertyStor::Str(_) => Err(GenCamError::PropertyNotEnum),
            PropertyStor::EnumStr(prop) => {
                Ok(prop.get_variants()?.into_iter().map(|x| x.into()).collect())
            }
            PropertyStor::EnumInt(prop) => {
                Ok(prop.get_variants()?.into_iter().map(|x| x.into()).collect())
            }
            PropertyStor::EnumUnsigned(prop) => {
                Ok(prop.get_variants()?.into_iter().map(|x| x.into()).collect())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PropertyStor {
    Bool(PropertyConcrete<bool>),
    Int(PropertyConcrete<i64>),
    Float(PropertyConcrete<f64>),
    Unsigned(PropertyConcrete<u64>),
    Duration(PropertyConcrete<Duration>),
    Str(PropertyConcrete<String>),
    EnumStr(PropertyEnum<String>),
    EnumInt(PropertyEnum<i64>),
    EnumUnsigned(PropertyEnum<u64>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
/// A property value
pub enum PropertyValue {
    /// A boolean value
    Bool(bool),
    /// An integer value
    Int(i64),
    /// A floating point value
    Float(f64),
    /// An unsigned integer value
    Unsigned(u64),
    /// A duration value
    Duration(Duration),
    /// A string value
    Str(String),
    /// An enum string value
    EnumStr(String),
    /// An enum integer value
    EnumInt(i64),
    /// An enum unsigned integer value
    EnumUnsigned(u64),
}

impl From<i64> for PropertyValue {
    fn from(val: i64) -> Self {
        PropertyValue::Int(val)
    }
}

impl From<f64> for PropertyValue {
    fn from(val: f64) -> Self {
        PropertyValue::Float(val)
    }
}

impl From<u64> for PropertyValue {
    fn from(val: u64) -> Self {
        PropertyValue::Unsigned(val)
    }
}

impl From<Duration> for PropertyValue {
    fn from(val: Duration) -> Self {
        PropertyValue::Duration(val)
    }
}

impl From<String> for PropertyValue {
    fn from(val: String) -> Self {
        PropertyValue::Str(val)
    }
}

impl From<&str> for PropertyValue {
    fn from(val: &str) -> Self {
        PropertyValue::Str(val.to_owned())
    }
}

impl From<bool> for PropertyValue {
    fn from(val: bool) -> Self {
        PropertyValue::Bool(val)
    }
}

impl From<&PropertyValue> for PropertyType {
    fn from(prop: &PropertyValue) -> Self {
        use PropertyValue::*;
        match prop {
            Bool(_) => PropertyType::Bool,
            Int(_) => PropertyType::Int,
            Float(_) => PropertyType::Float,
            Unsigned(_) => PropertyType::Unsigned,
            Duration(_) => PropertyType::Duration,
            Str(_) => PropertyType::Str,
            EnumStr(_) => PropertyType::EnumStr,
            EnumInt(_) => PropertyType::EnumInt,
            EnumUnsigned(_) => PropertyType::EnumUnsigned,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
/// The type of a property
pub enum PropertyType {
    /// A boolean property
    Bool,
    /// An integer property ([`i64`])
    Int,
    /// A floating point property ([`f64`])
    Float,
    /// An unsigned integer property ([`u64`])
    Unsigned,
    /// A duration property ([`Duration`])
    Duration,
    /// A string property ([`String`])
    Str,
    /// An enum string property ([`String`])
    EnumStr,
    /// An enum integer property ([`i64`])
    EnumInt,
    /// An enum unsigned integer property ([`u64`])
    EnumUnsigned,
}

impl From<&PropertyStor> for PropertyType {
    fn from(prop: &PropertyStor) -> Self {
        use PropertyStor::*;
        match prop {
            Bool(_) => PropertyType::Bool,
            Int(_) => PropertyType::Int,
            Float(_) => PropertyType::Float,
            Unsigned(_) => PropertyType::Unsigned,
            Duration(_) => PropertyType::Duration,
            Str(_) => PropertyType::Str,
            EnumStr(_) => PropertyType::EnumStr,
            EnumInt(_) => PropertyType::EnumInt,
            EnumUnsigned(_) => PropertyType::EnumUnsigned,
        }
    }
}

pub trait PropertyFunctions<T: Sized> {
    fn get_min(&self) -> Result<T>;
    fn get_max(&self) -> Result<T>;
    fn get_step(&self) -> Result<T>;
    fn get_default(&self) -> Result<T>;
    fn get_variants(&self) -> Result<Vec<T>>;
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropertyConcrete<T: Sized> {
    value: T,
    min: T,
    max: T,
    step: T,
    rdonly: bool,
    default: T,
}

impl<T: Sized> PropertyConcrete<T> {
    pub fn new(value: T, min: T, max: T, step: T, rdonly: bool, default: T) -> Self {
        PropertyConcrete {
            value,
            min,
            max,
            step,
            rdonly,
            default,
        }
    }
}

macro_rules! prop_num_for_prop_conc {
    ($t:ty, $b: path) => {
        impl PropertyFunctions<$t> for PropertyConcrete<$t> {
            fn get_min(&self) -> Result<$t> {
                Ok(self.min)
            }
            fn get_max(&self) -> Result<$t> {
                Ok(self.max)
            }
            fn get_step(&self) -> Result<$t> {
                Ok(self.step)
            }
            fn get_default(&self) -> Result<$t> {
                Ok(self.default)
            }
            fn get_variants(&self) -> Result<Vec<$t>> {
                Err(GenCamError::PropertyNotEnum)
            }
        }
    };
}

prop_num_for_prop_conc!(i64, PropertyType::Int);
prop_num_for_prop_conc!(f64, PropertyType::Float);
prop_num_for_prop_conc!(u64, PropertyType::Unsigned);
prop_num_for_prop_conc!(Duration, PropertyType::Duration);

impl PropertyFunctions<bool> for PropertyConcrete<bool> {
    fn get_min(&self) -> Result<bool> {
        Err(GenCamError::PropertyNotNumber)
    }
    fn get_max(&self) -> Result<bool> {
        Err(GenCamError::PropertyNotNumber)
    }
    fn get_step(&self) -> Result<bool> {
        Err(GenCamError::PropertyNotNumber)
    }
    fn get_default(&self) -> Result<bool> {
        Ok(self.default)
    }
    fn get_variants(&self) -> Result<Vec<bool>> {
        Err(GenCamError::PropertyNotEnum)
    }
}

impl PropertyFunctions<String> for PropertyConcrete<String> {
    fn get_min(&self) -> Result<String> {
        Err(GenCamError::PropertyNotNumber)
    }
    fn get_max(&self) -> Result<String> {
        Err(GenCamError::PropertyNotNumber)
    }
    fn get_step(&self) -> Result<String> {
        Err(GenCamError::PropertyNotNumber)
    }
    fn get_default(&self) -> Result<String> {
        Ok(self.default.clone())
    }
    fn get_variants(&self) -> Result<Vec<String>> {
        Err(GenCamError::PropertyNotEnum)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropertyEnum<T: Sized + PartialEq> {
    value: usize, // index of the value in the variants
    variants: Vec<T>,
    rdonly: bool,
    default: usize,
}

impl<T: Sized + PartialEq> PropertyEnum<T> {
    pub fn new(value: T, variants: Vec<T>, rdonly: bool, default: T) -> Self {
        let default = variants.iter().position(|x| x == &default).unwrap();
        let value = variants.iter().position(|x| x == &value).unwrap();
        PropertyEnum {
            value,
            variants,
            rdonly,
            default,
        }
    }
}

macro_rules! impl_propfn_for_propenum {
    ($t:ty) => {
        impl PropertyFunctions<$t> for PropertyEnum<$t> {
            fn get_min(&self) -> Result<$t> {
                Err(GenCamError::PropertyNotNumber)
            }
            fn get_max(&self) -> Result<$t> {
                Err(GenCamError::PropertyNotNumber)
            }
            fn get_step(&self) -> Result<$t> {
                Err(GenCamError::PropertyNotNumber)
            }
            fn get_default(&self) -> Result<$t> {
                Ok(self.variants[self.default].clone())
            }
            fn get_variants(&self) -> Result<Vec<$t>> {
                Ok(self.variants.clone())
            }
        }
    };
}

impl_propfn_for_propenum!(String);
impl_propfn_for_propenum!(i64);
impl_propfn_for_propenum!(u64);

trait EnumType {
    fn get_enum() -> PropertyType;
}

impl EnumType for String {
    fn get_enum() -> PropertyType {
        PropertyType::EnumStr
    }
}

impl EnumType for i64 {
    fn get_enum() -> PropertyType {
        PropertyType::EnumInt
    }
}

impl EnumType for u64 {
    fn get_enum() -> PropertyType {
        PropertyType::EnumUnsigned
    }
}
