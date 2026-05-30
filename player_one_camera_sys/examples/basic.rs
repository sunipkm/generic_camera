//! A port of the `examples/C/main.c` example
//! into Rust but with more data and stuff
use std::{io::Read, mem::MaybeUninit, time::Duration};

use player_one_camera_sys::{
    self as poa, Bool, Camera, CameraState, ConfigParameter, ConfigValue, ConfigValueKind, Error,
    Id, ImageFormat, Millis, get_image_data, senti::ptr::Buffer,
};

fn wait_for_user_input() {
    println!("Press enter to quit");
    _ = std::io::stdin().read(&mut [0]);
}
struct CloseOnDrop(Id<Camera>);
impl Drop for CloseOnDrop {
    fn drop(&mut self) {
        unsafe {
            println!("Closing camera");
            let mut out = MaybeUninit::uninit();
            if poa::get_state(self.0, &mut out).is_err() {
                return;
            }
            match out.assume_init().get() {
                Ok(CameraState::Closed) | Err(_) => {}
                Ok(CameraState::Exposing) => {
                    _ = poa::stop_exposure(self.0);
                    _ = poa::close_camera(self.0);
                }
                Ok(CameraState::Opened) => _ = poa::close_camera(self.0),
            }
        }
    }
}
macro_rules! bail {
    ($e:expr, $fmt:literal, $($args:tt)* $(,)?) => {
        {
            match $e {
                Ok(v) => v,
                Err(e) => {
                    eprintln!($fmt $($args)*, e = e);
                    wait_for_user_input();
                    return Ok(())
                }
            }
        }
    };
}
fn main() -> Result<(), Error> {
    let cam_count = unsafe { poa::get_camera_count() };
    if cam_count <= 0 {
        eprintln!("There are no Player One cameras connected");
        wait_for_user_input();
        return Ok(());
    }
    println!("There are {cam_count} connected cameras\n");
    println!("===== Connected Cameras =====");
    let first_cam_properties = (0..cam_count)
        .filter_map(|idx| {
            let mut out = MaybeUninit::uninit();
            unsafe {
                match poa::get_camera_properties(idx, &mut out).into_result() {
                    Ok(()) => {
                        let prop = out.assume_init();
                        println!("      Index: {idx}");
                        println!("         ID: {}", prop.camera_id.id());
                        println!("       Name: {}", prop.model_name);
                        println!("     Serial: {}", prop.serial);
                        println!("Sensor Name: {}", prop.sensor_model_name);
                        println!(" Local Path: {}", prop.local_path);
                        println!("  Custom ID: {}", prop.user_custom_id);
                        println!(" Product ID: {:X}", prop.product_id.id());
                        println!("   Max Dims: {}x{}", prop.max_width, prop.max_height);
                        println!(" Pixel Size: {}μm", prop.pixel_size);
                        println!("  Bit Depth: {}", prop.bit_depth);
                        println!("   Is Color: {}", prop.is_color_camera.into_bool());
                        println!("  Bayer Pat: {:?}", prop.bayer_pattern);
                        println!("Out Formats: {:?}", prop.formats);
                        println!(" Has Cooler: {}", prop.has_cooler.into_bool());
                        println!("  STP4 Port: {}", prop.has_st4_port.into_bool());
                        println!(" USB3 Speed: {}", prop.is_usb3_speed.into_bool());
                        println!("   Hard Bin: {}", prop.harware_bin_supported.into_bool());

                        println!(
                            "  Bin Modes: {:?}",
                            (&prop.binning_modes)
                                .into_iter()
                                .map(|x| x.id())
                                .collect::<Vec<_>>()
                        );

                        println!();
                        Some(prop)
                    }
                    Err(e) => {
                        eprintln!("Failed to get properties of camera at index {idx}: {e}");
                        println!();
                        None
                    }
                }
            }
        })
        .reduce(|x, _| x);
    let Some(cam_props) = first_cam_properties else {
        eprintln!("Failed to get properties of any camera");
        return Ok(());
    };

    let cam_id = cam_props.camera_id;
    let _close_on_drop = CloseOnDrop(cam_id);
    println!("\n===== Config Info =====");
    unsafe {
        bail!(
            poa::open_camera(cam_id).into_result(),
            "Failed to open camera: {e}",
        );
        bail!(
            poa::init_camera(cam_id).into_result(),
            "Failed to initialize camera: {e}",
        );
        let mut cfg_count = MaybeUninit::new(0);
        bail!(
            poa::get_config_count(cam_id, &mut cfg_count).into_result(),
            "Failed to get config count: {e}",
        );
        let cfg_count = cfg_count.assume_init();
        for cfg_idx in 0..cfg_count {
            let mut attrs = MaybeUninit::uninit();
            let attrs = match poa::get_config_attributes(cam_id, cfg_idx, &mut attrs).into_result()
            {
                Ok(()) => attrs.assume_init(),
                Err(e) => {
                    eprintln!("Failed to get config attributes of config {cfg_idx}: {e}");
                    continue;
                }
            };
            println!();
            println!("       Name: {}", attrs.name);
            println!("Description: {}", attrs.description);
            println!("   Writable: {}", attrs.writable.into_bool());
            println!("   Readable: {}", attrs.readable.into_bool());
            match attrs.value_type.get() {
                Ok(ConfigValueKind::Bool) => {
                    println!(
                        "    Default: {}",
                        attrs.default_value.bool_value.into_bool()
                    )
                }
                Ok(ConfigValueKind::Float) => {
                    println!(
                        "      Range: {}..={}",
                        attrs.min_value.float_value, attrs.max_value.float_value
                    );
                    println!("    Default: {}", attrs.default_value.float_value)
                }
                Ok(ConfigValueKind::Int) => {
                    println!(
                        "      Range: {}..={}",
                        attrs.min_value.int_value, attrs.max_value.int_value
                    );
                    println!("    Default: {}", attrs.default_value.int_value)
                }
                Err(e) => {
                    eprintln!("Got unknown value type for config value: {e}");
                }
            }
        }
    }

    println!("===== Starting Capture =====");
    let camera_state = {
        let mut out = MaybeUninit::uninit();
        unsafe {
            poa::get_state(cam_id, &mut out)
                .into_result()
                .map(|()| out.assume_init().get().unwrap_or(CameraState::Opened))
                .unwrap_or(CameraState::Opened)
        }
    };
    if camera_state == CameraState::Exposing {
        println!("Camera is currently exposing, stopping exposure");
        match unsafe { poa::stop_exposure(cam_id).into_result() } {
            Ok(()) => (),
            Err(e) => {
                eprintln!("Failed to stop exposure: {e}. Weird things may happen");
            }
        }
    }
    // Set binning mode
    println!("Setting binning mode to the second one");
    let second = (cam_props.binning_modes).to_slice()[1];
    match unsafe { poa::set_image_bin(cam_id, second).into_result() } {
        Ok(()) => {}
        Err(e) => {
            println!("Failed to set binning mode: {e}");
            println!("Continuing anyway");
        }
    }
    let [mut start_x, mut start_y, mut width, mut height] = [MaybeUninit::new(0); 4];
    let (mut start_x, mut start_y, mut width, mut height) = unsafe {
        let (start_x, start_y) =
            match poa::get_roi_start_pos(cam_id, &mut start_x, &mut start_y).into_result() {
                Ok(()) => (start_x.assume_init(), start_y.assume_init()),
                Err(e) => {
                    eprintln!("Failed to get ROI start position: {e}\nAssuming (0, 0)");
                    (0, 0)
                }
            };
        let (width, height) = match poa::get_roi_size(cam_id, &mut width, &mut height).into_result()
        {
            Ok(()) => (width.assume_init(), height.assume_init()),
            Err(e) => {
                println!("Failed to get ROI size: {e}\nAssuming maximum under the bin.");
                (
                    cam_props.max_width / second.id(),
                    cam_props.max_height / second.id(),
                )
            }
        };
        (start_x, start_y, width, height)
    };

    println!("Original ROI: ({start_x}, {start_y}) {width}x{height}");
    // Do some shifting
    width -= 50;
    height -= 20;
    // Make sure the width is a multiple of 4
    width &= -4;
    // Make sure the height is a multiple of 2
    height &= -2;

    start_x += 20;
    start_y += 10;

    println!("New ROI: ({start_x}, {start_y}) {width}x{height}");
    unsafe {
        match poa::set_roi_size(cam_id, width, height).into_result() {
            Ok(()) => {}
            Err(e) => {
                println!("Failed to set ROI size: {e}\nContinuing anyway");
            }
        }
        match poa::set_roi_start_pos(cam_id, start_x, start_y).into_result() {
            Ok(()) => {}
            Err(e) => {
                println!("Failed to set ROI offset: {e}\nContinuing anyway");
            }
        }
    }
    println!("Setting parameters (0.1s exposure, 100 gain, raw 16)");
    unsafe {
        bail!(
            poa::set_image_format(cam_id, poa::ImageFormat::Raw16).into_result(),
            "Failed to set image format to raw16: {e}",
        );
        let exposure_time_us = 100000;
        let gain = 100;
        let res = poa::set_config(
            cam_id,
            ConfigParameter::ExposureMicros,
            ConfigValue {
                int_value: exposure_time_us,
            },
            Bool::False,
        );
        bail!(res.into_result(), "Failed to set exposure time: {e}",);
        let res = poa::set_config(
            cam_id,
            ConfigParameter::Gain,
            ConfigValue { int_value: gain },
            Bool::False,
        );
        bail!(res.into_result(), "Failed to set gain: {e}",);
    }
    println!("Starting exposure");
    // Continuous exposure
    let res = unsafe { poa::start_exposure(cam_id, Bool::False).into_result() };
    bail!(res, "Failed to start exposure: {e}",);
    println!("Started exposure");
    let num_images = 10;
    let mut buf =
        vec![MaybeUninit::uninit(); ImageFormat::Raw16.buffer_size(width as _, height as _)];
    for image_num in 0..num_images {
        println!("Capturing image {image_num}");
        loop {
            // sleep for 10 millis at a time. You can also just use `get_image_data` with a -1 timeout
            // to sleep until ready
            std::thread::sleep(Duration::from_millis(10));
            let mut ready = MaybeUninit::new(false.into());
            unsafe {
                bail!(
                    poa::is_image_ready(cam_id, &mut ready).into_result(),
                    "Failed to poll readiness of the image: {e}",
                );
                if ready.assume_init().into_bool() {
                    break;
                }
            };
        }
        let (slice, res) = Buffer::with(&mut buf, |buf, len| unsafe {
            get_image_data(cam_id, buf, len as _, Millis(-1))
        });
        bail!(res.into_result(), "Failed to read image data: {e}",);
        let path = format!("image_{}_raw16.bin", image_num);
        println!("Writing image data to {path}");
        let data = unsafe { std::slice::from_raw_parts(slice.as_ptr().cast::<u8>(), slice.len()) };
        bail!(
            std::fs::write(&path, data),
            "Failed to write image data to {path}: {e}",
        );
    }
    Ok(())
}
