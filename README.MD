# `generic-camera`
`generic-camera` is a framework for implementing type-agnostic interfaces for cameras. `generic-camera` provides two fundamental traits, `GenCamDriver` and `GenCam`.

## `GenCamDriver`
`GenCamDriver` defines functionality used to query a driver for available devices, their capabilities, and connect to a specific device.

## `GenCam`
`GenCam` defines functionality to query a specific driver for its capabilities (`get_properties`), which return a map of camera settings, along with legal values, controlled using the `get_property` and `set_property` functions.
`start_exposure`, `image_ready` and `download_image` functions allow non-blocking image capture, and `capture` blocks to return an image. `generic-camera` uses `refimage::GenericImageRef` to support arbitrary image encapsulation (`u8`, `u16` and `f32` data types) while being serdes compatible and supporting metadata.

## `GenCamCtrl`
`GenCamCtrl` encapsulates control of different aspects of a camera (serdes compatible). 
- `GenCamCtrl::Device` encapsulates device control, 
- `GenCamCtrl::Sensor` encapsulates sensor controls, such as setting ROI, pixel bit depth, `
- `GenCamCtrl::Trigger` encapsulates trigger selection and settings, 
- `GenCamCtrl::Exposure` encapsulates exposure control (such as exposure time, auto exposure)
- `GenCamCtrl::FrameTime` encapsulates frame time controls (such as frame rate)
- `GenCamCtrl::Analog` controls analog features such as gain, white balance etc.
- `GenCamCtrl::DigitalIo` controls digital I/O.

## `Property`
`Property` structs encapsulate allowed ranges and variants for the various controls. The API accepts concrete values through the `PropertyValue` struct. Both `Property` and `PropertyValue` are serdes compatible.

# Optional Features
- `server`: This optional feature exports `GenCamServer`, a type that aggregates multiple cameras, accessed by a `i32` ID returned when the camera is inserted into `GenCamServer`. Functions associated with a camera are called by passing in the appropriate `GenSrvCmd`, and the returns values are encapsulated in `GenSrvValue`.
- `dummy`: This optional feature exports a dummy camera through `GenCamDriverDummy` and `GenCamDummy` to demonstrate the use of the API.