//! Raw FFI bindings for the ZWO [ASI Camera SDK](https://www.zwoastro.com/downloads/developers).
//!
//! This crate contains raw FFI bindings generated with bindgen for the ZWO ASI Camera SDK.
//!
//! ## Dependencies
//! This crate requires `libusb-1.0-dev` along with an
//! an installation of the [ASI Camera SDK](https://www.zwoastro.com/downloads/developers).
//!
//! ### Searching
//! This crate currently always searches system include and library paths for `libusb-1.0`.
//!
//! However, the ASI Camera SDK path can be configured with `ZWO_ASI_SDK` environment variable.
//! This variable should point to the extracted `ASI_linux_mac_SDK_*` folder that should contain
//! `lib` and `include` directories among others.
//!
//! If this variable is not set, then the crate automaticall searches for include and library paths
//! on system paths.
#![allow(
    dead_code,
    unused_imports,
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case
)]
// If we're documenting, we want to allow unresolved links / other invalid markdown since bindgen doesn't escape markdown.
//
// We have to allow all warnings since for some reason, unresolved link warnings are unsuppressable except by allowing all warnings.
// Thanks rustdoc.
#![cfg_attr(doc, allow(warnings))]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

impl Default for ASI_CAMERA_INFO {
    fn default() -> Self {
        Self {
            Name: [0; 64],
            CameraID: Default::default(),
            MaxHeight: Default::default(),
            MaxWidth: Default::default(),
            IsColorCam: Default::default(),
            BayerPattern: Default::default(),
            SupportedBins: Default::default(),
            SupportedVideoFormat: Default::default(),
            PixelSize: Default::default(),
            MechanicalShutter: Default::default(),
            ST4Port: Default::default(),
            IsCoolerCam: Default::default(),
            IsUSB3Host: Default::default(),
            IsUSB3Camera: Default::default(),
            ElecPerADU: Default::default(),
            BitDepth: Default::default(),
            IsTriggerCam: Default::default(),
            Unused: Default::default(),
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for ASI_ID {
    fn default() -> Self {
        Self {
            id: Default::default(),
        }
    }
}

impl Default for ASI_CONTROL_CAPS {
    fn default() -> Self {
        Self {
            Name: [0; 64],
            Description: [0; 128],
            MaxValue: Default::default(),
            MinValue: Default::default(),
            DefaultValue: Default::default(),
            IsAutoSupported: Default::default(),
            ControlType: Default::default(),
            IsWritable: Default::default(),
            Unused: Default::default(),
        }
    }
}
