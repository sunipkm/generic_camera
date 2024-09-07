use std::time::Duration;

use crate::{GenCamError, GenCamPixelBpp, Result};
use serde::{Deserialize, Serialize};

use crate::controls::GenCamCtrl;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// A property
pub struct Property {
    name: GenCamCtrl,
    auto: bool,
    prop: PropertyLims,
}

impl Property {
    /// Create a new property
    pub fn new<T>(name: T, prop: PropertyLims, auto: bool) -> Self
    where
        T: Into<GenCamCtrl>,
    {
        Property {
            name: name.into(),
            auto,
            prop,
        }
    }

    /// Get the name of the property
    pub fn get_name(&self) -> GenCamCtrl {
        self.name
    }

    /// Get the type of the property
    pub fn get_type(&self) -> PropertyType {
        (&self.prop).into()
    }

    /// Check if the property supports auto mode
    pub fn supports_auto(&self) -> bool {
        self.auto
    }

    /// Get the minimum value of the property
    pub fn get_min(&self) -> Result<PropertyValue> {
        match &self.prop {
            PropertyLims::Bool(_) => Err(GenCamError::PropertyNotNumber),
            PropertyLims::Int(prop) => Ok(prop.get_min()?.into()),
            PropertyLims::Float(prop) => Ok(prop.get_min()?.into()),
            PropertyLims::Unsigned(prop) => Ok(prop.get_min()?.into()),
            PropertyLims::PixelFmt(prop) => Ok(prop.get_min()?.into()),
            PropertyLims::Duration(prop) => Ok(prop.get_min()?.into()),
            PropertyLims::EnumStr(_) => Err(GenCamError::PropertyNotNumber),
            PropertyLims::EnumInt(prop) => Ok(prop.get_min()?.into()),
            PropertyLims::EnumUnsigned(prop) => Ok(prop.get_min()?.into()),
        }
    }

    /// Get the maximum value of the property
    pub fn get_max(&self) -> Result<PropertyValue> {
        match &self.prop {
            PropertyLims::Bool(_) => Err(GenCamError::PropertyNotNumber),
            PropertyLims::Int(prop) => Ok(prop.get_max()?.into()),
            PropertyLims::Float(prop) => Ok(prop.get_max()?.into()),
            PropertyLims::Unsigned(prop) => Ok(prop.get_max()?.into()),
            PropertyLims::PixelFmt(prop) => Ok(prop.get_max()?.into()),
            PropertyLims::Duration(prop) => Ok(prop.get_max()?.into()),
            PropertyLims::EnumStr(_) => Err(GenCamError::PropertyNotNumber),
            PropertyLims::EnumInt(prop) => Ok(prop.get_max()?.into()),
            PropertyLims::EnumUnsigned(prop) => Ok(prop.get_max()?.into()),
        }
    }

    /// Get the step value of the property
    pub fn get_step(&self) -> Result<PropertyValue> {
        match &self.prop {
            PropertyLims::Bool(_) => Err(GenCamError::PropertyNotNumber),
            PropertyLims::Int(prop) => Ok(prop.get_step()?.into()),
            PropertyLims::Float(prop) => Ok(prop.get_step()?.into()),
            PropertyLims::Unsigned(prop) => Ok(prop.get_step()?.into()),
            PropertyLims::PixelFmt(_) => Err(GenCamError::PropertyIsEnum),
            PropertyLims::Duration(prop) => Ok(prop.get_step()?.into()),
            PropertyLims::EnumStr(_) => Err(GenCamError::PropertyNotNumber),
            PropertyLims::EnumInt(_) => Err(GenCamError::PropertyIsEnum),
            PropertyLims::EnumUnsigned(_) => Err(GenCamError::PropertyIsEnum),
        }
    }

    /// Get the default value of the property
    pub fn get_default(&self) -> Result<PropertyValue> {
        match &self.prop {
            PropertyLims::Bool(prop) => Ok(prop.get_default()?.into()),
            PropertyLims::Int(prop) => Ok(prop.get_default()?.into()),
            PropertyLims::Float(prop) => Ok(prop.get_default()?.into()),
            PropertyLims::Unsigned(prop) => Ok(prop.get_default()?.into()),
            PropertyLims::PixelFmt(prop) => Ok(prop.get_default()?.into()),
            PropertyLims::Duration(prop) => Ok(prop.get_default()?.into()),
            PropertyLims::EnumStr(prop) => Ok(prop.get_default()?.into()),
            PropertyLims::EnumInt(prop) => Ok(prop.get_default()?.into()),
            PropertyLims::EnumUnsigned(prop) => Ok(prop.get_default()?.into()),
        }
    }

    /// Get the variants of the property
    pub fn get_variants(&self) -> Result<Vec<PropertyValue>> {
        match &self.prop {
            PropertyLims::Bool(_) => Err(GenCamError::PropertyNotEnum),
            PropertyLims::Int(_) => Err(GenCamError::PropertyNotEnum),
            PropertyLims::Float(_) => Err(GenCamError::PropertyNotEnum),
            PropertyLims::Unsigned(_) => Err(GenCamError::PropertyNotEnum),
            PropertyLims::Duration(_) => Err(GenCamError::PropertyNotEnum),
            PropertyLims::PixelFmt(prop) => {
                Ok(prop.get_variants()?.into_iter().map(|x| x.into()).collect())
            }
            PropertyLims::EnumStr(prop) => {
                Ok(prop.get_variants()?.into_iter().map(|x| x.into()).collect())
            }
            PropertyLims::EnumInt(prop) => {
                Ok(prop.get_variants()?.into_iter().map(|x| x.into()).collect())
            }
            PropertyLims::EnumUnsigned(prop) => {
                Ok(prop.get_variants()?.into_iter().map(|x| x.into()).collect())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PropertyLims {
    Bool(PropertyConcrete<bool>),
    Int(PropertyConcrete<i64>),
    Float(PropertyConcrete<f64>),
    Unsigned(PropertyConcrete<u64>),
    Duration(PropertyConcrete<Duration>),
    PixelFmt(PropertyEnum<GenCamPixelBpp>),
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
    /// A pixel format value
    PixelFmt(GenCamPixelBpp),
    /// A duration value
    Duration(Duration),
    /// An enum string value
    EnumStr(String),
}

impl From<i64> for PropertyValue {
    fn from(val: i64) -> Self {
        PropertyValue::Int(val)
    }
}

impl From<u64> for PropertyValue {
    fn from(val: u64) -> Self {
        PropertyValue::Unsigned(val)
    }
}

impl From<f64> for PropertyValue {
    fn from(val: f64) -> Self {
        PropertyValue::Float(val)
    }
}

impl From<Duration> for PropertyValue {
    fn from(val: Duration) -> Self {
        PropertyValue::Duration(val)
    }
}

impl From<String> for PropertyValue {
    fn from(val: String) -> Self {
        PropertyValue::EnumStr(val)
    }
}

impl From<&str> for PropertyValue {
    fn from(val: &str) -> Self {
        PropertyValue::EnumStr(val.to_owned())
    }
}

impl From<bool> for PropertyValue {
    fn from(val: bool) -> Self {
        PropertyValue::Bool(val)
    }
}

impl From<GenCamPixelBpp> for PropertyValue {
    fn from(val: GenCamPixelBpp) -> Self {
        PropertyValue::PixelFmt(val)
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
            PixelFmt(_) => PropertyType::PixelFmt,
            Duration(_) => PropertyType::Duration,
            EnumStr(_) => PropertyType::EnumStr,
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
    /// A pixel format property ([`GenCamPixelBpp`])
    PixelFmt,
    /// A duration property ([`Duration`])
    Duration,
    /// An enum string property ([`String`])
    EnumStr,
    /// An enum integer property ([`i64`])
    EnumInt,
    /// An enum unsigned integer property ([`u64`])
    EnumUnsigned,
}

impl From<&PropertyLims> for PropertyType {
    fn from(prop: &PropertyLims) -> Self {
        use PropertyLims::*;
        match prop {
            Bool(_) => PropertyType::Bool,
            Int(_) => PropertyType::Int,
            Float(_) => PropertyType::Float,
            Unsigned(_) => PropertyType::Unsigned,
            Duration(_) => PropertyType::Duration,
            PixelFmt(_) => PropertyType::PixelFmt,
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
    min: T,
    max: T,
    step: T,
    rdonly: bool,
    default: T,
}

impl<T: Sized> PropertyConcrete<T> {
    pub fn new(min: T, max: T, step: T, rdonly: bool, default: T) -> Self {
        PropertyConcrete {
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
impl_propfn_for_propenum!(GenCamPixelBpp);

// trait EnumType {
//     fn get_enum() -> PropertyType;
// }

// impl EnumType for String {
//     fn get_enum() -> PropertyType {
//         PropertyType::EnumStr
//     }
// }

// impl EnumType for i64 {
//     fn get_enum() -> PropertyType {
//         PropertyType::EnumInt
//     }
// }

// impl EnumType for u64 {
//     fn get_enum() -> PropertyType {
//         PropertyType::EnumUnsigned
//     }
// }
