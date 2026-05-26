use player_one_camera_sys::{
    self as poa, CameraState,
    ffi_util::{MaybeInvalid, ValidationError},
};
error_set::error_set! {
    pub CameraError := {
        UnknownCameraState(ValidationError<CameraState>),
        (poa::Error)
    }
    pub PropertyError := {
        WrongType,
        NotInRange
    }
}
