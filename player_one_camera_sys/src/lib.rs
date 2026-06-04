//! Raw (but sanitized) FFI bindings for the [Player One Camera SDK](https://player-one-astronomy.com/service/software/).
//!
//! This crate contains raw (but sanitized) hand-written FFI bindings for the Player One Camera SDK
//!
//! ## Documentation
//! Most documentation here is paraphrased from the original `PlayerOneCamera.h` file to be reworded for clarity
//! and to reference items in this crate instead of the original C code. In cases where I could not immediately make sense
//! of the documentation, the original is mostly taken word-for-word.
//!
//! Types and values are renamed to fit Rust's naming conventions and personal preferences.
//! The original names for types and functions are also added as doc aliases. Some helper methods are also provided
//! to do basic operations. Some helper structs such as [`BoundedCString`]
//! are also used to ensure certain safety conditions are met and limit the amount of unsafety despite basically just being raw bindings.
//!
//! ## Notes
//! None of the functions in this library are guaranteed to be thread-safe as they may mutate global state. Even
//! "getter" functions may mutate global state.
//!
//! ## Dependencies
//! This crate requires `libusb-1.0-dev` along with an
//! an installation of the [Player One Camera SDK](https://player-one-astronomy.com/service/software/).
//! The include directory need not be installed since we hand-write all of the definitions.
//!
//! ### Searching
//! This crate currently always searches system library paths for `libusb-1.0`.
//!
//! However, the Player One Camera SDK path can be configured with `PLAYER_ONE_CAMERA_SDK` environment variable.
//! This variable should point to the extracted `PlayerOne_Camera_SDK.*` folder that should contain
//! `lib` and `include` directories among others.
//!
//! If this variable is not set, then the crate automatically searches for the library path
//! on system paths.

pub use senti;
use std::{
    cell::Cell,
    ffi::{c_int, c_long},
    marker::PhantomData,
    mem::MaybeUninit,
};

use senti::{
    MaybeInvalid, Reserved,
    bytemuck::{self, NoUninit},
    c_enum,
    cstring::BoundedCString,
    ptr::Buffer,
    senti::{Senti, Terminated},
};

/// A boolean value represented as a C enum, using 4x as much storage as necessary
#[doc(alias = "POABool")]
#[repr(C)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Bool {
    #[default]
    False = 0,
    True = 1,
}
impl Bool {
    #[inline(always)]
    pub const fn new(b: bool) -> Self {
        if b { Bool::True } else { Bool::False }
    }
    #[inline(always)]
    pub const fn into_bool(self) -> bool {
        matches!(self, Self::True)
    }
}
impl From<bool> for Bool {
    fn from(value: bool) -> Self {
        Self::new(value)
    }
}
impl From<Bool> for bool {
    fn from(value: Bool) -> Self {
        value.into_bool()
    }
}

c_enum! {
    /// A bayer dithering pattern
    #[doc(alias = "POABayerPattern")]
    #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
    pub enum BayerPattern {
        /// RGGB
        Rg = 0,
        /// BGGR
        Bg,
        /// GRBG
        Gr,
        /// GBRG
        Gb,
        /// Monochrome, the mono camera with this
        Mono = -1,
    }
}

c_enum! {
    /// An image data format
    #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
    #[doc(alias = "POAImgFormat")]
    pub enum ImageFormat {
        /// 8-bit raw data, 1 pixel 1 byte, value range `0..=255`
        Raw8 = 0,
        /// 16-bit raw data, 1 pixel 2 bytes, value range `0..=65535`
        Raw16,
        /// RGB888 color data, 1 pixel 3 bytes, value range `0..=255` (only color camera)
        Rgb24,
        /// 8-bit monochrome data, convert the Bayer Filter Array to monochrome data. 1 pixel 1 byte, value range `0..=255` (only color camera)
        Mono8,

        // End = -1,
    }
}

impl ImageFormat {
    /// Computes the buffer size for a given pixel format
    pub const fn buffer_size(self, width: u32, height: u32) -> usize {
        let m = match self {
            Self::Raw8 => 1,
            Self::Raw16 => 2,
            Self::Rgb24 => 3,
            Self::Mono8 => 1,
        };
        m * width as usize * height as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct MaybeInvalidImageFormat(pub MaybeInvalid<ImageFormat>);
unsafe impl bytemuck::NoUninit for MaybeInvalidImageFormat {}

const POA_END: c_int = -1;
impl Terminated for MaybeInvalidImageFormat {
    // Terminal value of the supported formats array. This is not returned by any APIs
    // normally and is considered a terminator, so we don't put it in the enum
    const SENTINEL: Self = MaybeInvalidImageFormat(MaybeInvalid::from_c_int(POA_END));
}

c_enum! {
    /// The state of a camera
    #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
    #[doc(alias = "POACameraState")]
    pub enum CameraState {
        /// The camera is currently closed
        Closed = 0,
        /// The camera is open, but not exposing
        Opened,
        /// The camera is currently exposing
        Exposing
    }
}

macro_rules! def_errors {
    {
        #[result($result_name:ident)]
        $(#[$attrs:meta])*
        $vis:vis enum $err_name:ident {
        $(
            #[msg = $msg:literal]
            $(#[$err_meta:meta])*

            $err:ident $(= $value:expr)?
        ),*$(,)?
        }
    } => {
        c_enum! {
            $(#[$attrs])*
            #[must_use]
            #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
            pub enum $result_name {
                /// Operation was successful
                Ok = 0,
                $(
                    $(#[$err_meta])*
                    $err $(= $value)?
                ),*
            }
        }
        impl $result_name {
            /// Converts the result into a Rust `Result`
            pub const fn into_result(self) -> ::std::result::Result<(), $err_name> {
                use $result_name::*;
                use $err_name as E;
                match self {
                    $result_name::Ok => ::std::result::Result::Ok(()),
                    $($err => ::std::result::Result::Err(E::$err)),*
                }
            }
            //// Creates a result from a Rust result
            pub const fn from_result(res: ::std::result::Result<(), $err_name>) -> Self {
                match res {
                     ::std::result::Result::Ok(()) => $result_name::Ok,
                     ::std::result::Result::Err(err) => Self::from_err(err),
                }
            }

            /// Creates a result from an error value
            pub const fn from_err(err: $err_name) -> Self {
                use $result_name::*;
                use $err_name as E;
                match err {
                    $(E::$err => $err),*
                }
            }
            /// Whether the result is ok
            pub const fn is_ok(&self) -> bool {
                matches!(self, $result_name::Ok)
            }

            /// Whether the result is an error
            pub const fn is_err(&self) -> bool {
                !self.is_ok()
            }

            /// Attempts to get the ok value out of the result
            pub const fn ok(self) -> Option<()> {
                if self.is_ok() { Some(()) } else { None }
            }

            /// Attempts to get the error value out of the result
            pub const fn err(self) -> Option<$err_name> {
                use $result_name::*;
                use $err_name as E;
                Some(match self {
                    Ok => return None,
                    $($err => E::$err),*
                })
            }
        }
        c_enum! {
            $(#[$attrs])*
            #[must_use]
            #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
            pub enum $err_name {
                $(
                    $(#[$err_meta])*
                    $err $(= $value)?
                ),*
            }
        }
        impl $err_name {
            /// Gets the message for the error
            pub const fn message(self) -> &'static str {
                use $err_name::*;
                match self {
                    $($err => $msg),*
                }
            }
        }
        impl ::std::fmt::Display for $err_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.message())
            }
        }
        impl ::std::error::Error for $err_name {}

        impl From<$result_name> for ::std::result::Result<(), $err_name> {
            fn from(res: $result_name) -> Self {
                res.into_result()
            }
        }
        impl<T: Into<$err_name>> From<::std::result::Result<(), T>> for $result_name {
            fn from(val: ::std::result::Result<(), T>) -> Self {
                Self::from_result(val.map_err(Into::into))
            }
        }
        impl From<$err_name> for $result_name {
            fn from(val: $err_name) -> Self {
                Self::from_err(val)
            }
        }
        impl From<$result_name> for Option<$err_name> {
            fn from(val: $result_name) -> Self {
                val.err()
            }
        }
    };
}

def_errors! {
    #[result(PoaResult)]
    #[doc(alias = "POAErrors")]
    /// The result of an operation.
    pub enum Error {
        #[msg = "invalid index"]
        /// Invalid index, means the index is negative or out of bounds (camera or config)
        InvalidIndex = 1,

        #[msg = "invalid camera id was passed"]
        /// Invalid camera ID
        InvalidId,

        #[msg = "invalid config attribute or value"]
        /// Invalid config attribute
        InvalidConfig,

        #[msg = "invalid argument was passed"]
        /// Invalid argument
        InvalidArgument,

        #[msg = "camera is not open"]
        /// The camera is not open
        NotOpened,

        #[msg = "device was not found"]
        /// The camera was not found, may have been removed
        DeviceNotFound,

        #[msg = "value is out of bounds"]
        /// The value is out out of bounds
        OutOfLimit,

        #[msg = "exposure failed"]
        /// Camera exposure failed
        ExposureFailed,

        #[msg = "timed out"]
        /// Timeout
        Timeout,

        #[msg = "data buffer is too small"]
        /// The data buffer is too small
        BufferSmall,

        #[msg = "camera is exposing"]
        /// The camera is currently exposing and the operation requires the camera to not be exposing
        Exposing,

        #[msg = "a null pointer was passed"]
        /// Invalid pointer. This should not happen since we ensure that a non-null pointer is passed.
        InvalidPointer,

        #[msg = "not writable"]
        /// The config is not writable
        NotWritable,

        #[msg = "not readable"]
        /// The config is not readable
        NotReadable,

        #[msg = "access denied"]
        /// Access denied
        AccessDenied,

        #[msg = "failed"]
        /// The operation failed, maybe the camera was disconnected suddenly
        Failed,

        #[msg = "out of memory"]
        /// Memory allocation failed
        Oom,
    }
}

c_enum! {
    /// Type of value for a config variable
    #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
    pub enum ConfigValueKind {
        /// An integer value (c_long)
        Int = 0,
        /// A floating-point value (f64)
        Float,
        /// A boolean value (Bool)
        Bool,
    }
}

/// A union of config value types
#[repr(C)]
#[derive(Clone, Copy)]
pub union ConfigValue {
    pub int_value: c_long,
    pub float_value: f64,
    pub bool_value: Bool,
}

mod config_parameter {
    // we need this to tell the compiler to shut the hell
    // up about using the depreccated
    // `ConfigParameter::Heater` which is literally only used
    // to match on to check validity
    #![allow(deprecated)]
    use senti::c_enum;

    c_enum! {
        #[doc(alias = "POAConfig")]
        /// The configurable parameters for a camera
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub enum ConfigParameter {
            /// Exposure time in microseconds, `10..=2000000000`, read/write, recommended to use `ExposureSeconds` instead, `Int`
            ExposureMicros = 0,
            /// Gain, read/write, `Int`
            Gain,
            /// Hardware bin, read/write, `Bool`
            HardwareBin,
            /// Camera temperature (Celcius), read-only, `Float`
            Temperature,
            /// Red channel white balance, `-1200..=1200`, read/write, `Int`
            WhiteBalanceR,
            /// Green channel white balance, `-1200..=1200`, read/write, `Int`
            WhiteBalanceG,
            /// Blue channel white balance, `-1200..=1200`, read/write, `Int`
            WhiteBalanceB,
            /// Camera offset, read/write, `Int`
            Offset,
            /// Maximum gain when auto-adjust is enabled, read/write, `Int`
            AutoExposureMaxGain,
            /// Maximum exposure when auto-adjust is enabled (unit: millis), read/write, `Int`
            AutoExposureMaxExposure,
            /// Target brightness when auto-adjust is enabled, read/write, `Int`
            AutoExposureBrightness,
            /// ST4 guide north, generally, it's DEC+ on the mount, read/write, `Bool`
            GuideNorth,
            /// ST4 guide south, generally, it's DEC- on the mount, read/write, `Bool`
            GuideSouth,
            /// ST4 guide east, generally, it's RA+ on the mount, read/write, `Bool`
            GuideEast,
            /// ST4 guide west, generally, it's RA- on the mount, read/write, `Bool`
            GuideWest,
            /// e/ADU, This value will change with gain, read-only, `Float`
            EGain,
            /// Cooler power percentage, `0..=100`, (only cool camera), read-only, `Int`
            CoolerPower,
            /// Camera target temperature (in degrees Celcius), read/write, `Int`
            TargetTemp,
            /// Turn cooler (and fan) on or off, read/write, `Bool`
            Cooler,
            /// (Deprecated) State of the lens heater, read-only, `Bool`
            #[deprecated]
            Heater,
            /// Lens heater power percentage, `0..=100`, read/write, `Int`
            HeaterPower,
            /// Radiator fan power percentage, `0..=100`, read/write, `Int`
            FanPower,
            /// No flip. Note that the value argument passed to [`set_config`] is ignored. read/write, `Bool`
            FlipNone,
            /// Flip the image horizontally. Note that the value argument passed to [`set_config`] is ignored. read/write, `Bool`
            FlipHorizontal,
            /// Flip the image vertically. Note that the value argument passed to [`set_config`] is ignored. read/write, `Bool`
            FlipVertical,
            /// Flip the image both vertically and horizontally. Note that the value argument passed to [`set_config`] is ignored. read/write, `Bool`
            FlipBoth,
            /// Frame rate limit, `0..=2000`, 0 means no limit, read/write, `Int`
            FrameLimit,
            /// High Quality Image, for those without DDR camera (guide camera), if set to `True`, this will reduce the waviness and stripe of the image,
            /// but frame rate may go down. Note: this config has no effect on those cameras that have DDR. read/write, `Bool`
            Hqi,
            /// USB bandwidth limit, read/write, `Int`
            UsbBandwidthLimit,
            /// Take the sum of pixels after binning, `True` is sum and `False` is average, default is `False`, read/write, `Bool`
            BinSum,
            /// Only for color cameras, when set to `True`, pixel binning will use neighbor pixels and the image after
            /// binning will lose the bayer pattern, read/write, `Bool`
            MonoBin,
            /// Exposure time in seconds, `0.00001..=7200.0`, read/write, `Float`
            ExposureSeconds,
        }

    }
}
#[doc(inline)]
pub use config_parameter::ConfigParameter;

impl ConfigParameter {
    /// The type of value associated with this parameter
    pub const fn value_type(self) -> ConfigValueKind {
        #![allow(deprecated)]
        use ConfigParameter::*;
        match self {
            ExposureMicros
            | Gain
            | WhiteBalanceB
            | WhiteBalanceG
            | WhiteBalanceR
            | Offset
            | AutoExposureMaxGain
            | AutoExposureBrightness
            | AutoExposureMaxExposure
            | CoolerPower
            | TargetTemp
            | FanPower
            | FrameLimit
            | HeaterPower
            | UsbBandwidthLimit => ConfigValueKind::Int,
            HardwareBin | GuideNorth | GuideSouth | GuideEast | GuideWest | Cooler | Heater
            | FlipNone | FlipHorizontal | FlipBoth | FlipVertical | Hqi | BinSum | MonoBin => {
                ConfigValueKind::Bool
            }
            Temperature | EGain | ExposureSeconds => ConfigValueKind::Float,
        }
    }
    /// Whether this parameter is normally writable
    pub const fn writable(self) -> bool {
        #![allow(deprecated)]
        use ConfigParameter::*;
        match self {
            ExposureMicros
            | Gain
            | HardwareBin
            | WhiteBalanceR
            | WhiteBalanceG
            | WhiteBalanceB
            | Offset
            | Cooler
            | AutoExposureMaxGain
            | AutoExposureMaxExposure
            | AutoExposureBrightness
            | GuideNorth
            | GuideEast
            | GuideSouth
            | GuideWest
            | TargetTemp
            | HeaterPower
            | FanPower
            | FlipBoth
            | FlipHorizontal
            | FlipVertical
            | FlipNone
            | FrameLimit
            | Hqi
            | UsbBandwidthLimit
            | BinSum
            | MonoBin
            | ExposureSeconds => true,
            Temperature | EGain | Heater | CoolerPower => false,
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
struct SendButNotSync<Res>(PhantomData<Cell<Res>>);

impl<Res> Clone for SendButNotSync<Res> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<Res> Copy for SendButNotSync<Res> {}

/// An opaque identifier for some resource denoted by the marker type `Res`.
/// These resources are generally safe to send across threads, but need manual synchronization.
#[derive(Debug, Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct Id<Res>(c_int, SendButNotSync<Res>);

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}

impl<Res> Id<Res> {
    /// Returns the underlying value of the id, useful for debugging. For
    /// binning modes, this is the value.
    pub const fn id(self) -> c_int {
        self.0
    }
}

/// A marker struct indicating that an [`Id`] represents a camera
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Camera {}
/// A marker struct indicating that an [`Id`] represents a binning mode
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum BinningMode {}

unsafe impl<T: 'static> NoUninit for Id<T> {}

impl Terminated for Id<BinningMode> {
    const SENTINEL: Self = Id(0, SendButNotSync(PhantomData));
}

/// A marker struct indicating that an [`Id`] represents a product
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Product {}

pub type CameraId = Id<Camera>;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
/// Properties of the camera
pub struct CameraProperties {
    /// The camera name
    pub model_name: BoundedCString<256>,
    /// User custom id name
    pub user_custom_id: BoundedCString<16>,
    /// The id of the camera
    pub camera_id: Id<Camera>,
    /// The max width of the camera
    pub max_width: c_int,
    /// The max height of the camera
    pub max_height: c_int,
    /// ADC depth of CMOS sensor
    pub bit_depth: c_int,
    /// Whether the camera is a color camera
    pub is_color_camera: Bool,
    /// Whether the camera has an ST4 port. If `False`, does not support ST4 guide.
    pub has_st4_port: Bool,
    /// Whether the camera has a cooler assembly
    pub has_cooler: Bool,
    /// Whether the connection is usb-3.0 speed connection
    pub is_usb3_speed: Bool,
    /// The bayer filter pattern of the camera
    pub bayer_pattern: MaybeInvalid<BayerPattern>,
    /// The camera pixel size in micrometers
    pub pixel_size: f64,
    /// The unique serial number of camera
    pub serial: BoundedCString<64>,
    /// The sensor model name of the camera, eg.: `IMX462`
    pub sensor_model_name: BoundedCString<32>,
    /// The path of the camera in the computer host
    pub local_path: BoundedCString<256>,
    /// The binning modes supported by the camera, 1 == bin1, 2 == bin2,...
    pub binning_modes: Senti<Id<BinningMode>, 8>,
    /// Image formats supported by the camera, terminated with [`ImageFormat::End`].
    pub formats: Senti<MaybeInvalidImageFormat, 8>,
    /// Whether the camera sensor supports hardware binning
    pub harware_bin_supported: Bool,
    /// The camera's product ID. The vID of PlayerOne is `0xA0A0`.
    pub product_id: Id<Product>,
    /// Reserved data
    pub _reserved: Reserved<248>,
}
/// Attributes for a config parameter
#[doc(alias = "POAConfigAttributes")]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ConfigAttributes {
    /// Whether the attribute supports automatic adjustment
    pub supports_auto: Bool,
    /// Whether the attribute is writable
    pub writable: Bool,
    /// Whether the attribute is readable
    pub readable: Bool,
    /// The kind of attribute this is
    pub kind: MaybeInvalid<ConfigParameter>,
    /// The kind of value this attribute is
    pub value_type: MaybeInvalid<ConfigValueKind>,
    /// The maximum value of the attribute
    pub max_value: ConfigValue,
    /// The minimum value of the attribute
    pub min_value: ConfigValue,
    /// The default value of the attribute
    pub default_value: ConfigValue,
    /// The name of the attribute
    pub name: BoundedCString<64>,
    /// A short description of the attribute
    pub description: BoundedCString<128>,
    pub _reserved: Reserved<64>,
}

pub type Mu<T> = MaybeUninit<T>;
pub type Mi<T> = MaybeInvalid<T>;

/// A newtype representing some amount of milliseconds
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Millis(pub c_int);

/// Human-readable information about a sensor mode
#[doc(alias = "POASensorModeInfo")]
#[repr(C)]
pub struct SensorModeInfo {
    /// The sensor mode name, can be used to display in a UI
    pub name: BoundedCString<64>,
    /// Sensor mode description, may be useful for tooltips
    pub description: BoundedCString<128>,
}
unsafe extern "C" {
    /// Gets the number of POA cameras connected to the computer host
    ///
    /// # Safety
    /// - I don't know the actual safety conditions of this function and I really can't know unless
    ///   I reverse engineer the library. Good luck.
    /// - I'd probably recommend just not calling this concurrently with any other function in the
    ///   library.
    #[link_name = "POAGetCameraCount"]
    #[doc(alias = "POAGetCameraCount")]
    pub unsafe fn get_camera_count() -> c_int;

    /// Gets the properties of the `index`th camera available.
    /// The camera does not need to be open for this operation.
    /// Note that the index is not a camera ID.
    ///
    /// # Return
    ///  - [`PoaResult::Ok`] on success
    ///  - [`PoaResult::InvalidIndex`] if the index is invalid.
    ///
    /// The original C function also may return [`PoaResult::InvalidPointer`] if the input pointer is null,
    /// but that is impossible in this Rust wrapper.
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any write of data relating to the camera
    ///   at the index
    /// - You must ensure that `out_props` is valid for writes.
    #[link_name = "POAGetCameraProperties"]
    #[doc(alias = "POAGetCameraProperties")]
    pub unsafe fn get_camera_properties(
        index: c_int,
        out_props: &mut Mu<CameraProperties>,
    ) -> PoaResult;

    /// Gets the properties of a camera by its [`Id`], writing the result ot `out_props` on success.
    /// The camera does not need to be open for this operation.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if the ID is invalid.
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any write of data relating to this camera
    /// - You must ensure that `out_props` is valid for writes.
    #[link_name = "POAGetCameraPropertiesByID"]
    #[doc(alias = "POAGetCameraPropertiesByID")]
    pub unsafe fn get_camera_properties_by_id(
        id: Id<Camera>,
        out_props: &mut Mu<CameraProperties>,
    ) -> PoaResult;

    /// Opens a camera by an `id` received from [`get_camera_properties`].
    /// Almost every other operation needs to have an open camera.
    ///
    /// # Returns
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::DeviceNotFound`] if the device was not found, likely due to removal
    /// - [`PoaResult::Failed`] if some other failure condition occured
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    #[link_name = "POAOpenCamera"]
    #[doc(alias = "POAOpenCamera")]
    pub unsafe fn open_camera(id: Id<Camera>) -> PoaResult;

    /// Initialize the camera's hardware / parameters, and allocates memory.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera was not opened
    /// - [`PoaResult::DeviceNotFound`] if the device was not found, likely due to removal
    /// - [`PoaResult::Oom`] if memory allocation failed
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    #[link_name = "POAInitCamera"]
    #[doc(alias = "POAInitCamera")]
    pub unsafe fn init_camera(id: Id<Camera>) -> PoaResult;

    /// Close the camera and free all related resources
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - You must ensure that the camera denoted by `id` is open
    /// - All properties of the camera and all resources derived from the camera are invalididated
    /// after this call. The camera id considered no longer valid.
    #[link_name = "POACloseCamera"]
    #[doc(alias = "POACloseCamera")]
    pub unsafe fn close_camera(id: Id<Camera>) -> PoaResult;

    /// Set a config value for a given camera
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera was not opened
    /// - [`PoaResult::InvalidConfig`] if the camera doesn't support `attr`
    /// - [`PoaResult::NotWritable`] if the config attribute is not writable
    /// - [`PoaResult::Failed`] if the operation failed for some other reason
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - You must ensure that the `value` has a value corresponding to the [`ConfigAttributeKind::value_type`]
    /// - You must ensure that `value` is in the valid range for this `attr`
    /// - The camera must be initialized if it is open
    #[link_name = "POASetConfig"]
    #[doc(alias = "POASetConfig")]
    pub unsafe fn set_config(
        id: Id<Camera>,
        attr: ConfigParameter,
        value: ConfigValue,
        is_auto: Bool,
    ) -> PoaResult;

    /// Gets the number of config attributes for a given camera, writing the result to `out` on success
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera was not opened
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POAGetConfigsCount"]
    #[doc(alias = "POAGetConfigsCount")]
    pub unsafe fn get_config_count(id: Id<Camera>, out: &mut MaybeUninit<c_int>) -> PoaResult;

    /// Gets the config attributes for a config parameter at the given index `index` of a given camera,
    /// writing the result to `out` on success
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera was not opened
    /// - [`PoaResult::InvalidIndex`] if no camera is connected or `index` value out is out of bounds or negative
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POAGetConfigAttributes"]
    #[doc(alias = "POAGetConfigAttributes")]
    pub unsafe fn get_config_attributes(
        id: Id<Camera>,
        index: c_int,
        out: &mut MaybeUninit<ConfigAttributes>,
    ) -> PoaResult;

    /// Gets the attributes for a config parameter for a camera based on the `kind`, writing the result to `out` on success
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera was not opened
    /// - [`PoaResult::InvalidConfig`] if the config parameter is invalid or the camera does not support that config parameter
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POAGetConfigAttributesByConfigID"]
    #[doc(alias = "POAGetConfigAttributesByConfigID")]
    pub unsafe fn get_config_attributes_by_kind(
        id: Id<Camera>,
        kind: ConfigParameter,
        out: &mut MaybeUninit<ConfigAttributes>,
    ) -> PoaResult;

    /// Get the config value and the auto value for a given config attribute. On success, the values are written to `out_value` and `is_auto`
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    /// - [`PoaResult::InvalidConfig`] if the camera doesn't support `attr`
    /// - [`PoaResult::NotReadable`] if the `attr` is not readable
    /// - [`PoaResult::Failed`] if the operation failed for some other reason
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POAGetConfig"]
    #[doc(alias = "POAGetConfig")]
    pub unsafe fn get_config(
        id: Id<Camera>,
        attr: ConfigParameter,
        out_value: &mut Mu<ConfigValue>,
        is_auto: &mut Mu<Bool>,
    ) -> PoaResult;

    /// Gets the start position of the region of interest (ROI). On success, the x and y positions are
    /// written to `x` and `y`.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POAGetImageStartPos"]
    #[doc(alias = "POAGetImageStartPos")]
    pub unsafe fn get_roi_start_pos(
        id: Id<Camera>,
        x: &mut Mu<c_int>,
        y: &mut Mu<c_int>,
    ) -> PoaResult;

    /// Set the start position of the camera's ROI area.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    /// - [`PoaResult::InvalidArgument`] if the either `x` or `y` are negative
    /// - [`PoaResult::Failed`] if the operation failed for some other reason
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POASetImageStartPos"]
    #[doc(alias = "POASetImageStartPos")]
    pub unsafe fn set_roi_start_pos(id: Id<Camera>, x: c_int, y: c_int) -> PoaResult;

    /// Gets the dimensions of the camera's ROI area. The width and height are written
    /// to `width` and `height` on success.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POAGetImageSize"]
    #[doc(alias = "POAGetImageSize")]
    pub unsafe fn get_roi_size(
        id: Id<Camera>,
        width: &mut Mu<c_int>,
        height: &mut Mu<c_int>,
    ) -> PoaResult;

    /// Sets the dimensions of the camera's ROI area. `width` should be a multiple of 4, and `height` should be a multiple of 2
    /// or else the final dimensions will be adjusted and you should call [`get_roi_size`] after.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    /// - [`PoaResult::InvalidArgument`] if the either `x` or `y` are negative
    /// - [`PoaResult::Failed`] if the operation failed for some other reason
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POASetImageSize"]
    #[doc(alias = "POASetImageSize")]
    pub unsafe fn set_roi_size(id: Id<Camera>, width: c_int, height: c_int) -> PoaResult;

    /// Gets the pixel bin method index for the camera, writing the result to `out` on success
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POAGetImageBin"]
    #[doc(alias = "POAGetImageBin")]
    pub unsafe fn get_image_bin(id: Id<Camera>, out: &mut Mu<c_int>) -> PoaResult;

    /// Sets the image binning method for the camera. On success, the image size and start position
    /// will be changed. Please call [`get_roi_start_pos`] and [`get_roi_size`] to get the new dimensions
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    /// - [`PoaResult::Failed`] on other failure
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    /// - You must ensure that the camera is not exposing
    #[link_name = "POASetImageBin"]
    #[doc(alias = "POASetImageBin")]
    pub unsafe fn set_image_bin(id: Id<Camera>, bin: Id<BinningMode>) -> PoaResult;

    /// Gets the image format of the camera, writing the result to `out` on success. Note that `out` is a [`MaybeInvalid`]
    /// since the driver could add new image formats later.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POAGetImageFormat"]
    #[doc(alias = "POAGetImageFormat")]
    pub unsafe fn get_image_format<'out>(
        id: Id<Camera>,
        out: &mut Mu<Mi<ImageFormat>>,
    ) -> PoaResult;

    /// Sets the image format for a given camera
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    /// - [`PoaResult::Failed`] on other failure
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    /// - You must ensure that the camera is not exposing
    #[link_name = "POASetImageFormat"]
    #[doc(alias = "POASetImageFormat")]
    pub unsafe fn set_image_format(id: Id<Camera>, fmt: ImageFormat) -> PoaResult;

    /// Starts an exposure on the camera. If `single_frame` is true, then only capture a single frame and then stop exposing, otherwise
    /// perform continuous capture.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    /// - [`PoaResult::Failed`] on other failure
    /// - [`PoaResult::Exposing`] if the camera is already exposing
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POAStartExposure"]
    #[doc(alias = "POAStartExposure")]
    pub unsafe fn start_exposure(id: Id<Camera>, single_frame: Bool) -> PoaResult;

    /// Stops an ongoing exposure on the camera.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    /// - [`PoaResult::Failed`] on other failure
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POAStopExposure"]
    #[doc(alias = "POAStopExposure")]
    pub unsafe fn stop_exposure(id: Id<Camera>) -> PoaResult;

    /// Gets the camera's current state, writing the state to `out` on success.  Note that `out` is a [`MaybeInvalid`]
    /// since the driver could add new camera states later.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds.
    ///
    /// # Safety
    ///  You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    #[link_name = "POAGetCameraState"]
    #[doc(alias = "POAGetCameraState")]
    pub unsafe fn get_state(id: Id<Camera>, out: &mut Mu<Mi<CameraState>>) -> PoaResult;

    /// Check if the image data from an exposure is ready, writing the result to `out` on success.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds.
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POAImageReady"]
    #[doc(alias = "POAImageReady")]
    pub unsafe fn is_image_ready(id: Id<Camera>, out: &mut Mu<Bool>) -> PoaResult;

    /// Gets the image data after exposure. If `timeout` is not -1, blocks for up to `timeout` milliseconds waiting for the
    /// data to be available, otherwise may block indefinitely waiting for the data to be available.
    ///
    /// On success data is written to `buf`, the amount written depends on the image format and ROI size:
    /// - [`ImageFormat::Raw8`] or [`ImageFormat::Mono8`]: `width * height`
    /// - [`ImageFormat::Raw16`]: `width * height * 2`,
    /// - [`ImageFormat::Rgb24`]: `width * height * 3`
    ///
    /// If [`is_image_ready`] returned `true` successfully, then this function will not block unless this function was called
    /// or some error occurred in between.
    ///
    /// The recommended timeout from Player One themselves is `exposure + 500ms` if calling this function immediately after
    /// starting exposure
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds.
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    /// - [`PoaResult::InvalidArgument`] if `buf_len` is negative
    /// - [`PoaResult::BufferSmall`] if the `buf_len` is not large enough to hold the image data
    /// - [`PoaResult::Failed`] on other failure
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any write of data relating to this camera
    /// - The camera must be initialized if it is open
    /// - `buf` must be valid for remembering up to `buf_len` bytes even if that many bytes are not actually written
    /// - The camera must currently be exposing
    #[link_name = "POAGetImageData"]
    #[doc(alias = "POAGetImageData")]
    pub unsafe fn get_image_data<'buf>(
        id: Id<Camera>,
        buf: Buffer<'buf, Mu<u8>>,
        buf_len: c_long,
        timeout: Millis,
    ) -> PoaResult;

    /// Gets the number of sensor modes for a given camera, writing it to `out` on success. If the written
    /// value was 0, the camera does not support sensor mode selection.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds.
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POAGetSensorModeCount"]
    #[doc(alias = "POAGetSensorModeCount")]
    pub unsafe fn get_sensor_mode_count(id: Id<Camera>, out: &mut Mu<c_int>) -> PoaResult;

    /// Gets information about a camera's `mode_idx`th sensor mode, writing the result into `out` on success.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::NotOpened`] if the camera is not opened
    /// - [`PoaResult::AccessDenied`] if the camera does not support sensor mode selection
    /// - [`PoaResult::InvalidArgument`] if `mode_idx` is out of range or negative (don't ask why it isn't [`PoaResult::InvalidIndex`])
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POAGetSensorModeInfo"]
    #[doc(alias = "POAGetSensorModeInfo")]
    pub unsafe fn get_sensor_mode_info(
        id: Id<Camera>,
        mode_idx: c_int,
        out: &mut Mu<SensorModeInfo>,
    ) -> PoaResult;

    /// Sets the sensor mode for a given camera to the `mode_idx`th sensor mode
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::AccessDenied`] if the camera does not support sensor mode selection
    /// - [`PoaResult::InvalidArgument`] if `mode_idx` is out of range or negative (don't ask why it isn't [`PoaResult::InvalidIndex`])
    /// - [`PoaResult::Failed`] on other failure
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    #[link_name = "POASetSensorMode"]
    #[doc(alias = "POASetSensorMode")]
    pub unsafe fn set_sensor_mode(id: Id<Camera>, mode_idx: c_int) -> PoaResult;

    /// Write the user custom ID into a camera's flash. If successful, you should refresh the information of this camera to see
    /// that the new user id has been written. If `string` is `None` or `len` is 0, then the previous settings will be cleared.
    /// If `len > 16`, then the string is truncated.
    ///
    /// # Return
    /// - [`PoaResult::Ok`] on success
    /// - [`PoaResult::InvalidId`] if no camera with this ID was found or the ID is out of bounds
    /// - [`PoaResult::Failed`] on other failure
    ///
    /// <div class="warning">
    /// The documentation says that if this is called during an exposure, the exposure will be interrupted, terminating the capture if in
    /// single shot mode. However, it also says that it returns [`PoaResult::Exposing`] if called during an exposure.
    ///
    /// I wouldn't risk calling this during an exposure to find out which of those behaviors are accurate, if any.
    /// </div>
    ///
    /// # Safety
    /// - You must not call this function from multiple threads concurrently with any read or write of data relating to this camera
    /// - The camera must be initialized if it is open
    /// - The camera should not be exposing or else some janky behavior might happen.
    #[link_name = "POASetUserCustomID"]
    #[doc(alias = "POASetUserCustomID")]
    pub unsafe fn set_user_custom_id(
        id: Id<Camera>,
        string: Option<&BoundedCString<16>>,
        len: c_int,
    ) -> PoaResult;

    /// Gets the API version. This is safe to call whenever.
    #[link_name = "POAGetAPIVersion"]
    #[doc(alias = "POAGetAPIVersion")]
    pub safe fn api_version() -> c_int;

    /// Gets the SDK version in the form `major.minor.patch`. This is safe to call whenever.
    #[link_name = "POAGetSDKVersion"]
    #[doc(alias = "POAGetSDKVersion")]
    pub safe fn sdk_version() -> &'static BoundedCString<16>;

}
