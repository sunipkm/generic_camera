use player_one_camera_sys::{self as poa, CameraState, senti::ValidationError};
error_set::error_set! {

    pub CameraError := {
        #[display("Got an unknown camera state: {0}")]
        UnknownCameraState(ValidationError<CameraState>),
        Internal(poa::Error),
        #[display("Image is not ready")]
        NotReady,
        #[display("Image has unknown image format")]
        UnknownImageFormat,
    }
    pub PropertyError := {
        #[display("Property is not supported")]
        Unsupported,
        #[display("Property value is out of range")]
        ValueOutOfRange,
        #[display("Wrong type of value for property")]
        WrongType,
        #[display("Failed to get or set property")]
        Failed,
        #[display("Failed to set property because camera is exposing")]
        Exposing,
    }
}
