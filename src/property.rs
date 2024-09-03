use serde::{Deserialize, Serialize};
use crate::{Result,  GenCamError};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Property {
    name: [u8; 32],
    desc: String,
    prop: PropertyStor
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PropertyStor {
    Bool(bool),
    Int(PropertyConcrete<i64>),
    Float(PropertyConcrete<f64>),
    Unsigned(PropertyConcrete<u64>),
    Str(String),
    EnumString(PropertyEnum<String>),
    EnumInt(PropertyEnum<i64>),
    EnumUnsigned(PropertyEnum<u64>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PropertyValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Unsigned(u64),
    Str(String),
    EnumString(String),
    EnumInt(i64),
    EnumUnsigned(u64),
}

impl From<PropertyStor> for PropertyValue {
    fn from(prop: PropertyStor) -> Self {
        match prop {
            PropertyStor::Bool(b) => PropertyValue::Bool(b),
            PropertyStor::Int(i) => PropertyValue::Int(i.get_value()),
            PropertyStor::Float(f) => PropertyValue::Float(f.get_value()),
            PropertyStor::Unsigned(u) => PropertyValue::Unsigned(u.get_value()),
            PropertyStor::Str(s) => PropertyValue::Str(s),
            PropertyStor::EnumString(e) => PropertyValue::EnumString(e.get_value()),
            PropertyStor::EnumInt(e) => PropertyValue::EnumInt(e.get_value()),
            PropertyStor::EnumUnsigned(e) => PropertyValue::EnumUnsigned(e.get_value()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PropertyType {
    Bool,
    Int,
    Float,
    Unsigned,
    Str,
    EnumString,
    EnumInt,
    EnumUnsigned,
}

impl From<PropertyStor> for PropertyType {
    fn from(prop: PropertyStor) -> Self {
        match prop {
            PropertyStor::Bool(_) => PropertyType::Bool,
            PropertyStor::Int(_) => PropertyType::Int,
            PropertyStor::Float(_) => PropertyType::Float,
            PropertyStor::Unsigned(_) => PropertyType::Unsigned,
            PropertyStor::Str(_) => PropertyType::Str,
            PropertyStor::EnumString(_) => PropertyType::EnumString,
            PropertyStor::EnumInt(_) => PropertyType::EnumInt,
            PropertyStor::EnumUnsigned(_) => PropertyType::EnumUnsigned,
        }
    }
}

pub trait PropertyFunctions<T: Sized>{
    fn get_value(&self) -> T;
    fn get_type(&self) -> PropertyType;
    fn set_value(&mut self, value: T) -> Result<T>;
    fn get_min(&self) -> Result<T>;
    fn get_max(&self) -> Result<T>;
    fn get_step(&self) -> Result<T>;
    fn get_default(&self) -> Result<T>;
    fn get_variants(&self) -> Result<Vec<T>>;
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropertyConcrete<T: Sized + Default> {
    value: T,
    min: T,
    max: T,
    step: T,
    rdonly: bool,
    default: T,
}

macro_rules! prop_num_for_prop_conc {
    ($t:ty, $b: path) => {
        impl PropertyFunctions<$t> for PropertyConcrete<$t> {
            fn get_value(&self) -> $t {
                self.value
            }
            fn set_value(&mut self, value: $t) -> Result<$t> {
                if self.rdonly {
                    return Err(GenCamError::ReadOnly);
                }
                if value < self.min || value > self.max {
                    return Err(GenCamError::InvalidValue(format!("Value out of range: Valid range [{}, {}]", self.min, self.max)));
                }
                self.value = value;
                Ok(value)
            }
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
            fn get_type(&self) -> PropertyType {
                $b
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

impl PropertyFunctions<bool> for PropertyConcrete<bool> {
    fn get_value(&self) -> bool {
        self.value
    }
    fn set_value(&mut self, value: bool) -> Result<bool> {
        if self.rdonly {
            return Err(GenCamError::ReadOnly);
        }
        self.value = value;
        Ok(value)
    }
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
    fn get_type(&self) -> PropertyType {
        PropertyType::Bool
    }
    fn get_variants(&self) -> Result<Vec<bool>> {
        Err(GenCamError::PropertyNotEnum)
    }
}

impl PropertyFunctions<String> for PropertyConcrete<String> {
    fn get_value(&self) -> String {
        self.value.clone()
    }
    fn set_value(&mut self, value: String) -> Result<String> {
        if self.rdonly {
            return Err(GenCamError::ReadOnly);
        }
        self.value = value;
        Ok(self.value.clone())
    }
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
    fn get_type(&self) -> PropertyType {
        PropertyType::Str
    }
    fn get_variants(&self) -> Result<Vec<String>> {
        Err(GenCamError::PropertyNotEnum)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropertyEnum<T: Sized> {
    value: usize, // index of the value in the variants
    variants: Vec<T>,
    rdonly: bool,
    default: usize,
}

macro_rules! impl_propfn_for_propenum {
    ($t:ty) => {
        impl PropertyFunctions<$t> for PropertyEnum<$t> {
            fn get_value(&self) -> $t {
                self.variants[self.value].clone()
            }
            fn set_value(&mut self, value: $t) -> Result<$t> {
                if self.rdonly {
                    return Err(GenCamError::ReadOnly);
                }
                let index = self.variants.iter().position(|x| *x == value);
                match index {
                    Some(i) => {
                        self.value = i;
                        Ok(value)
                    }
                    None => Err(GenCamError::InvalidValue(format!("Value not in variants: {:?}", self.variants))),
                }
            }
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
            fn get_type(&self) -> PropertyType {
                <$t>::get_enum()
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
        PropertyType::EnumString
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