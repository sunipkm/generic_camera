use std::time::Duration;

use crate::GenCamPixelBpp;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result type for property operations
pub type PropertyResult<T> = Result<T, PropertyError>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// A property
pub struct Property {
    auto: bool,
    rdonly: bool,
    prop: PropertyLims,
}

impl Property {
    /// Create a new property
    pub fn new(prop: PropertyLims, auto: bool, rdonly: bool) -> Self {
        Property { auto, rdonly, prop }
    }

    /// Get the type of the property
    pub fn get_type(&self) -> PropertyType {
        (&self.prop).into()
    }

    /// Check if the property supports auto mode
    pub fn supports_auto(&self) -> bool {
        self.auto
    }

    /// Validate a property value
    pub fn validate(&self, value: &PropertyValue) -> PropertyResult<()> {
        // 1. Check if value in enum
        match self.prop {
            PropertyLims::EnumStr { ref variants, .. } => {
                if let PropertyValue::EnumStr(ref val) = value {
                    if variants.contains(val) {
                        return Ok(());
                    } else {
                        return Err(PropertyError::ValueNotSupported);
                    }
                } else {
                    return Err(PropertyError::InvalidControlType {
                        expected: PropertyType::EnumStr,
                        received: value.get_type(),
                    });
                }
            }
            PropertyLims::EnumInt { ref variants, .. } => {
                if let PropertyValue::Int(ref val) = value {
                    if variants.contains(val) {
                        return Ok(());
                    } else {
                        return Err(PropertyError::ValueNotSupported);
                    }
                } else {
                    return Err(PropertyError::InvalidControlType {
                        expected: PropertyType::EnumInt,
                        received: value.get_type(),
                    });
                }
            }
            PropertyLims::EnumUnsigned { ref variants, .. } => {
                if let PropertyValue::Unsigned(ref val) = value {
                    if variants.contains(val) {
                        return Ok(());
                    } else {
                        return Err(PropertyError::ValueNotSupported);
                    }
                } else {
                    return Err(PropertyError::InvalidControlType {
                        expected: PropertyType::EnumUnsigned,
                        received: value.get_type(),
                    });
                }
            }
            PropertyLims::Duration { .. } => {
                if value.get_type() != PropertyType::Duration {
                    return Err(PropertyError::InvalidControlType {
                        expected: PropertyType::Duration,
                        received: value.get_type(),
                    });
                }
            }
            PropertyLims::Bool { .. } => {
                if value.get_type() != PropertyType::Bool {
                    return Err(PropertyError::InvalidControlType {
                        expected: PropertyType::Bool,
                        received: value.get_type(),
                    });
                }
            }
            PropertyLims::Int { .. } => {
                if value.get_type() != PropertyType::Int {
                    return Err(PropertyError::InvalidControlType {
                        expected: PropertyType::Int,
                        received: value.get_type(),
                    });
                }
            }
            PropertyLims::Float { .. } => {
                if value.get_type() != PropertyType::Float {
                    return Err(PropertyError::InvalidControlType {
                        expected: PropertyType::Float,
                        received: value.get_type(),
                    });
                }
            }
            PropertyLims::Unsigned { .. } => {
                if value.get_type() != PropertyType::Unsigned {
                    return Err(PropertyError::InvalidControlType {
                        expected: PropertyType::Unsigned,
                        received: value.get_type(),
                    });
                }
            }
            PropertyLims::PixelFmt { .. } => {
                if value.get_type() != PropertyType::PixelFmt {
                    return Err(PropertyError::InvalidControlType {
                        expected: PropertyType::PixelFmt,
                        received: value.get_type(),
                    });
                }
            }
        }
        // 2. Check if value is within limits
        match self.get_type() {
            PropertyType::Int
            | PropertyType::Unsigned
            | PropertyType::Float
            | PropertyType::Duration => {
                if &self.get_min()? <= value && value <= &self.get_max()? {
                    Ok(())
                } else {
                    Err(PropertyError::ValueOutOfRange {
                        value: value.clone(),
                        min: self.get_min().unwrap(), // safety: checked above
                        max: self.get_max().unwrap(), // safety: checked above
                    })
                }
            }
            PropertyType::Bool => Ok(()),
            PropertyType::Command
            | PropertyType::PixelFmt
            | PropertyType::EnumStr
            | PropertyType::EnumInt
            | PropertyType::EnumUnsigned => Err(PropertyError::NotNumber),
        }
    }

    /// Get the minimum value of the property
    pub fn get_min(&self) -> PropertyResult<PropertyValue> {
        use PropertyLims::*;
        match &self.prop {
            Bool { .. } => Err(PropertyError::NotNumber),
            Int { min, .. } => Ok((*min).into()),
            Float { min, .. } => Ok((*min).into()),
            Unsigned { min, .. } => Ok((*min).into()),
            Duration { min, .. } => Ok((*min).into()),
            PixelFmt { variants, .. } => {
                Ok((*variants.iter().min().ok_or(PropertyError::EmptyEnumList)?).into())
            }
            EnumStr { .. } => Err(PropertyError::NotNumber),
            EnumInt { variants, .. } => {
                Ok((*variants.iter().min().ok_or(PropertyError::EmptyEnumList)?).into())
            }
            EnumUnsigned { variants, .. } => {
                Ok((*variants.iter().min().ok_or(PropertyError::EmptyEnumList)?).into())
            }
        }
    }

    /// Get the maximum value of the property
    pub fn get_max(&self) -> PropertyResult<PropertyValue> {
        use PropertyLims::*;
        match &self.prop {
            Bool { .. } => Err(PropertyError::NotNumber),
            Int { max, .. } => Ok((*max).into()),
            Float { max, .. } => Ok((*max).into()),
            Unsigned { max, .. } => Ok((*max).into()),
            Duration { max, .. } => Ok((*max).into()),
            PixelFmt { variants, .. } => {
                Ok((*variants.iter().max().ok_or(PropertyError::EmptyEnumList)?).into())
            }
            EnumStr { .. } => Err(PropertyError::NotNumber),
            EnumInt { variants, .. } => {
                Ok((*variants.iter().max().ok_or(PropertyError::EmptyEnumList)?).into())
            }
            EnumUnsigned { variants, .. } => {
                Ok((*variants.iter().max().ok_or(PropertyError::EmptyEnumList)?).into())
            }
        }
    }

    /// Get the step value of the property
    pub fn get_step(&self) -> PropertyResult<PropertyValue> {
        use PropertyLims::*;
        match &self.prop {
            Bool { .. } => Err(PropertyError::NotNumber),
            Int { step, .. } => Ok((*step).into()),
            Float { step, .. } => Ok((*step).into()),
            Unsigned { step, .. } => Ok((*step).into()),
            Duration { step, .. } => Ok((*step).into()),
            PixelFmt { .. } => Err(PropertyError::IsEnum),
            EnumStr { .. } => Err(PropertyError::NotNumber),
            EnumInt { .. } => Err(PropertyError::IsEnum),
            EnumUnsigned { .. } => Err(PropertyError::IsEnum),
        }
    }

    /// Get the default value of the property
    pub fn get_default(&self) -> PropertyResult<PropertyValue> {
        use PropertyLims::*;
        match self.prop.clone() {
            Bool { default } => Ok(default.into()),
            Int { default, .. } => Ok(default.into()),
            Float { default, .. } => Ok(default.into()),
            Unsigned { default, .. } => Ok(default.into()),
            Duration { default, .. } => Ok(default.into()),
            PixelFmt { default, .. } => Ok(default.into()),
            EnumStr { default, .. } => Ok(default.into()),
            EnumInt { default, .. } => Ok(default.into()),
            EnumUnsigned { default, .. } => Ok(default.into()),
        }
    }

    /// Get the variants of the property
    pub fn get_variants(&self) -> PropertyResult<Vec<PropertyValue>> {
        use PropertyLims::*;
        match &self.prop {
            Bool { .. } | Int { .. } | Float { .. } | Unsigned { .. } | Duration { .. } => {
                Err(PropertyError::NotEnum)
            }
            PixelFmt { variants, .. } => Ok(variants.iter().map(|x| (*x).into()).collect()),
            EnumStr { variants, .. } => Ok(variants.iter().map(|x| x.clone().into()).collect()),
            EnumInt { variants, .. } => Ok(variants.iter().map(|x| (*x).into()).collect()),
            EnumUnsigned { variants, .. } => Ok(variants.iter().map(|x| (*x).into()).collect()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
/// A property with limits
pub enum PropertyLims {
    /// A boolean property
    Bool {
        /// The default value
        default: bool,
    },
    /// An integer property
    Int {
        /// The minimum value
        min: i64,
        /// The maximum value
        max: i64,
        /// The step size
        step: i64,
        /// The default value
        default: i64,
    },
    /// A floating point property
    Float {
        /// The minimum value
        min: f64,
        /// The maximum value
        max: f64,
        /// The step size
        step: f64,
        /// The default value
        default: f64,
    },
    /// An unsigned integer property
    Unsigned {
        /// The minimum value
        min: u64,
        /// The maximum value
        max: u64,
        /// The step size
        step: u64,
        /// The default value
        default: u64,
    },
    /// A duration property
    Duration {
        /// The minimum value
        min: Duration,
        /// The maximum value
        max: Duration,
        /// The step size
        step: Duration,
        /// The default value
        default: Duration,
    },
    /// A pixel format property
    PixelFmt {
        /// The variants of the property
        variants: Vec<GenCamPixelBpp>,
        /// The default value
        default: GenCamPixelBpp,
    },
    /// An enum string property
    EnumStr {
        /// The variants of the property
        variants: Vec<String>,
        /// The default value
        default: String,
    },
    /// An enum integer property
    EnumInt {
        /// The variants of the property
        variants: Vec<i64>,
        /// The default value
        default: i64,
    },
    /// An enum unsigned integer property
    EnumUnsigned {
        /// The variants of the property
        variants: Vec<u64>,
        /// The default value
        default: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, PartialOrd)]
#[non_exhaustive]
/// A property value
pub enum PropertyValue {
    /// A command
    Command,
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

impl PropertyValue {
    /// Get the type of the property value
    pub fn get_type(&self) -> PropertyType {
        self.into()
    }
}

impl From<()> for PropertyValue {
    fn from(_: ()) -> Self {
        PropertyValue::Command
    }
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
            Command => PropertyType::Command,
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
    /// A command property
    Command,
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
            Bool { .. } => PropertyType::Bool,
            Int { .. } => PropertyType::Int,
            Float { .. } => PropertyType::Float,
            Unsigned { .. } => PropertyType::Unsigned,
            Duration { .. } => PropertyType::Duration,
            PixelFmt { .. } => PropertyType::PixelFmt,
            EnumStr { .. } => PropertyType::EnumStr,
            EnumInt { .. } => PropertyType::EnumInt,
            EnumUnsigned { .. } => PropertyType::EnumUnsigned,
        }
    }
}

#[derive(Error, Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Property value error
pub enum PropertyError {
    /// Property not found.
    #[error("Property not found")]
    NotFound,
    /// Read only property.
    #[error("Property is read only")]
    ReadOnly,
    /// Property not an enum.
    #[error("Property is not an enum")]
    NotEnum,
    /// Property is not a number.
    #[error("Property is not a number")]
    NotNumber,
    #[error("Value out of range")]
    /// Value out of range.
    ValueOutOfRange {
        /// The minimum value.
        min: PropertyValue,
        /// The maximum value.
        max: PropertyValue,
        /// The supplied value.
        value: PropertyValue,
    },
    #[error("Value not supported")]
    /// Value not contained in the enum list.
    ValueNotSupported,
    /// Property is an enum, hence does not support min/max.
    #[error("Property is an enum")]
    IsEnum,
    /// Auto mode not supported.
    #[error("Auto mode not supported")]
    AutoNotSupported,
    #[error("Invalid control type: {expected:?} != {received:?}")]
    /// Invalid control type.
    InvalidControlType {
        /// The expected type.
        expected: PropertyType,
        /// The received type.
        received: PropertyType,
    },
    #[error("Empty enum list")]
    /// Empty enum list.
    EmptyEnumList,
}
