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
