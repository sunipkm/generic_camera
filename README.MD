# `generic-camera`
`generic-camera` is a framework for implementing type-agnostic interfaces for cameras. `generic-camera` provides two fundamental traits, `GenCamDriver` and `GenCam`.

## `GenCamDriver`
`GenCamDriver` defines functionality used to query a driver for available devices, their capabilities, and connect to a specific device.

## `GenCam`
`GenCam` defines functionality to query a specific driver for its capabilities (`get_properties`), which return a map of camera settings, along with legal values, controlled using the `get_property` and `set_property` functions.
`start_exposure`, `image_ready` and `download_image` functions allow non-blocking image capture, and `capture` blocks to return an image.

## `GenCamCtrl`
`GenCamCtrl` encapsulates control of different aspects of a camera, e.g. `GenCamCtrl::Device` encapsulates device controls such as retrieving sensor temperature, set a cooler temperature, etc. 