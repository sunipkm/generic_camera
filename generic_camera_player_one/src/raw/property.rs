use std::ffi::c_long;

use player_one_camera_sys::{self as poa, CameraProperties, ConfigValue, ConfigValueKind};

pub unsafe trait ConfigVal: Sized + Copy {
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
