use std::ffi::c_long;

use player_one_camera_sys::{self as poa, CameraProperties, ConfigValue, ConfigValueKind};

use crate::raw::error::PropertyError;

pub unsafe trait PropertyVal: Sized + Copy {
    unsafe fn from_value(kind: ConfigValueKind, value: ConfigValue) -> Result<Self, PropertyError>;
    unsafe fn to_value(self) -> (ConfigValueKind, ConfigValue);
}

unsafe impl PropertyVal for c_long {
    unsafe fn from_value(kind: ConfigValueKind, value: ConfigValue) -> Result<Self, PropertyError> {
        if kind != ConfigValueKind::Int {
            return Err(PropertyError::WrongType);
        }
        // SAFETY: Caller must uphold the active member being indicated by `kind`
        // (We don't just use an enum because we wanted ABI compatibility)
        unsafe { Ok(value.int_value) }
    }
    unsafe fn to_value(self) -> (ConfigValueKind, ConfigValue) {
        (ConfigValueKind::Int, ConfigValue { int_value: self })
    }
}

unsafe impl PropertyVal for bool {
    unsafe fn from_value(kind: ConfigValueKind, value: ConfigValue) -> Result<Self, PropertyError> {
        if kind != ConfigValueKind::Bool {
            return Err(PropertyError::WrongType);
        }
        // SAFETY: Caller must uphold the active member being indicated by `kind`
        // (We don't just use an enum because we wanted ABI compatibility)
        unsafe { Ok(value.bool_value.as_bool()) }
    }
    unsafe fn to_value(self) -> (ConfigValueKind, ConfigValue) {
        (
            ConfigValueKind::Bool,
            ConfigValue {
                bool_value: self.into(),
            },
        )
    }
}

unsafe impl PropertyVal for f64 {
    unsafe fn from_value(kind: ConfigValueKind, value: ConfigValue) -> Result<Self, PropertyError> {
        if kind != ConfigValueKind::Float {
            return Err(PropertyError::WrongType);
        }
        // SAFETY: Caller must uphold the active member being indicated by `kind`
        // (We don't just use an enum because we wanted ABI compatibility)
        unsafe { Ok(value.float_value) }
    }
    unsafe fn to_value(self) -> (ConfigValueKind, ConfigValue) {
        (ConfigValueKind::Float, ConfigValue { float_value: self })
    }
}
