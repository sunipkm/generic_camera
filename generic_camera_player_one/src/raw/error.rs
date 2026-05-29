use player_one_camera_sys::{self as poa, CameraState, ffi_util::ValidationError};
error_set::error_set! {

    pub CameraError := {
        #[display("Got an unknown camera state: {0}")]
        UnknownCameraState(ValidationError<CameraState>),
        Internal(poa::Error),
        #[display("Property value is out of range")]
        PropertyValueOutOfRange,
        #[display("Wrong type of value for property")]
        WrongType,
    }
}
