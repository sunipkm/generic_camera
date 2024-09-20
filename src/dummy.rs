/*!
# Dummy camera driver

This module contains a dummy camera that can be used for testing purposes, and as a reference or implementing new cameras.
# Usage
```
use generic_camera::dummy::{GenCamDriverDummy, GenCamDummy};
use generic_camera::{GenCam, GenCamDriver};
use generic_camera::{GenCamCtrl, controls::ExposureCtrl};
use std::time::Duration;
let mut driver = GenCamDriverDummy {};
let mut camera = driver.connect_first_device().expect("Failed to connect to camera");

let img = camera.capture().expect("Failed to capture image");
let exposure: Duration = camera.get_property(GenCamCtrl::Exposure(ExposureCtrl::ExposureTime)).expect("Failed to get exposure time").0.try_into().expect("Failed to convert exposure time");
println!("Exposure time: {:?}", exposure);
```
*/
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{
        atomic::{
            AtomicBool,
            Ordering::{Relaxed, SeqCst},
        },
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

use rand::{thread_rng, Rng};

use refimage::{DynamicImageData, ImageData};

use crate::{
    controls::ExposureCtrl, property::PropertyLims, GenCam, GenCamCtrl, GenCamDescriptor,
    GenCamDriver, GenCamError, GenCamResult, GenCamRoi, GenCamState, Property, PropertyError,
    PropertyValue,
};

#[derive(Debug)]
/// A dummy driver for testing purposes.
pub struct GenCamDriverDummy {}

impl GenCamDriver for GenCamDriverDummy {
    fn available_devices(&self) -> usize {
        1
    }

    fn list_devices(&mut self) -> GenCamResult<Vec<GenCamDescriptor>> {
        let mut desc = GenCamDescriptor {
            vendor: "Dummy".to_string(),
            name: "Dummy Camera".to_string(),
            id: 0xdeadbeef,
            ..Default::default()
        };
        desc.info.insert("Interface".into(), "Aether".into());
        Ok(vec![desc])
    }

    fn connect_device(&mut self, descriptor: &GenCamDescriptor) -> GenCamResult<crate::AnyGenCam> {
        let mut caps = HashMap::new();
        caps.insert(
            GenCamCtrl::Exposure(ExposureCtrl::ExposureTime),
            Property::new(
                PropertyLims::Duration {
                    min: Duration::from_millis(1),
                    max: Duration::from_secs(60),
                    step: Duration::from_millis(1),
                    default: Duration::from_secs(1),
                },
                false,
                false,
            ),
        );
        let mut vals = HashMap::new();
        vals.insert(
            GenCamCtrl::Exposure(ExposureCtrl::ExposureTime),
            (PropertyValue::Duration(Duration::from_secs(1)), false),
        );
        Ok(Box::new(GenCamDummy {
            desc: descriptor.clone(),
            name: descriptor.name.clone(),
            vendor: descriptor.vendor.clone(),
            caps,
            vals: RefCell::new(vals),
            capturing: Arc::new(AtomicBool::new(false)),
            roi: GenCamRoi {
                x_min: 0,
                y_min: 0,
                width: 1920,
                height: 1080,
            },
            data: Arc::new(Mutex::new(vec![0; 1920 * 1080 * 3])),
            imgready: Arc::new(AtomicBool::new(false)),
            start: RefCell::new(None),
        }))
    }

    fn connect_first_device(&mut self) -> GenCamResult<crate::AnyGenCam> {
        let desc = self
            .list_devices()?
            .pop()
            .ok_or(GenCamError::NoCamerasAvailable)?;
        self.connect_device(&desc)
    }
}

#[derive(Debug)]
/// A dummy camera for testing purposes.
pub struct GenCamDummy {
    desc: GenCamDescriptor,
    name: String,
    vendor: String,
    caps: HashMap<GenCamCtrl, Property>,
    vals: RefCell<HashMap<GenCamCtrl, (PropertyValue, bool)>>,
    capturing: Arc<AtomicBool>,
    imgready: Arc<AtomicBool>,
    roi: GenCamRoi,
    data: Arc<Mutex<Vec<u8>>>,
    start: RefCell<Option<Instant>>,
}

impl GenCam for GenCamDummy {
    fn info_handle(&self) -> Option<crate::AnyGenCamInfo> {
        None
    }

    fn info(&self) -> GenCamResult<&GenCamDescriptor> {
        Ok(&self.desc)
    }

    fn vendor(&self) -> &str {
        &self.vendor
    }

    fn camera_ready(&self) -> bool {
        true
    }

    fn camera_name(&self) -> &str {
        &self.name
    }

    fn list_properties(&self) -> &std::collections::HashMap<crate::GenCamCtrl, crate::Property> {
        &self.caps
    }

    fn get_property(&self, name: crate::GenCamCtrl) -> GenCamResult<(crate::PropertyValue, bool)> {
        match self.vals.borrow().get(&name) {
            Some(val) => Ok(val.clone()),
            None => Err(GenCamError::PropertyError {
                control: name,
                error: PropertyError::NotFound,
            }),
        }
    }

    fn set_property(
        &mut self,
        name: crate::GenCamCtrl,
        value: &crate::PropertyValue,
        auto: bool,
    ) -> GenCamResult<()> {
        if self.capturing.load(SeqCst) {
            return Err(GenCamError::ExposureInProgress);
        }
        match self.vals.borrow_mut().get_mut(&name) {
            Some(val) => {
                *val = (value.clone(), auto);
                Ok(())
            }
            None => Err(GenCamError::PropertyError {
                control: name,
                error: PropertyError::NotFound,
            }),
        }
    }

    fn cancel_capture(&self) -> GenCamResult<()> {
        self.capturing.store(false, SeqCst);
        Ok(())
    }

    fn is_capturing(&self) -> bool {
        self.capturing.load(SeqCst)
    }

    fn capture(&mut self) -> GenCamResult<refimage::GenericImage> {
        if self.imgready.load(Relaxed) {
            self.imgready.store(false, Relaxed);
            self.capturing.store(false, SeqCst);
            self.start.borrow_mut().take();
        }
        if self.capturing.load(SeqCst) {
            return Err(GenCamError::ExposureInProgress);
        }
        let now = Instant::now();
        let (exp, _) = self.get_property(GenCamCtrl::Exposure(ExposureCtrl::ExposureTime))?;
        let exp = exp.try_into().map_err(|e| GenCamError::PropertyError {
            control: GenCamCtrl::Exposure(ExposureCtrl::ExposureTime),
            error: e,
        })?;
        self.start.borrow_mut().replace(now.clone());
        self.capturing.store(true, SeqCst);
        self.imgready.store(false, Relaxed);
        loop {
            if !self.capturing.load(Relaxed) {
                break;
            }
            if now.elapsed() >= exp {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        {
            let mut data = self.data.lock().unwrap();
            thread_rng().fill(data.as_mut_slice());
        }
        self.imgready.store(true, Relaxed);
        self.download_image()
    }

    fn start_exposure(&mut self) -> GenCamResult<()> {
        if self.imgready.load(Relaxed) {
            self.imgready.store(false, Relaxed);
            self.capturing.store(false, SeqCst);
            self.start.borrow_mut().take();
        }
        if self.capturing.load(SeqCst) {
            return Err(GenCamError::ExposureInProgress);
        }
        let now = Instant::now();
        let (exp, _) = self.get_property(GenCamCtrl::Exposure(ExposureCtrl::ExposureTime))?;
        let exp = exp.try_into().map_err(|e| GenCamError::PropertyError {
            control: GenCamCtrl::Exposure(ExposureCtrl::ExposureTime),
            error: e,
        })?;
        self.start.borrow_mut().replace(now.clone());
        self.capturing.store(true, SeqCst);
        self.imgready.store(false, Relaxed);
        let capturing = self.capturing.clone();
        let imgready = self.imgready.clone();
        let img = self.data.clone();
        thread::spawn(move || {
            loop {
                if !capturing.load(SeqCst) {
                    break;
                }
                if now.elapsed() >= exp {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            {
                let mut img = img.lock().unwrap();
                thread_rng().fill(img.as_mut_slice());
            }
            imgready.store(true, Relaxed);
        });
        Ok(())
    }

    fn download_image(&mut self) -> GenCamResult<refimage::GenericImage> {
        let state = self.camera_state()?;
        match state {
            GenCamState::Exposing(_) => Err(GenCamError::ExposureInProgress),
            GenCamState::Idle => Err(GenCamError::ExposureNotStarted),
            GenCamState::ExposureFinished => {
                let data = self.data.lock().unwrap().clone();
                self.imgready.store(false, Relaxed);
                self.capturing.store(false, SeqCst);
                let img = ImageData::from_owned(
                    data,
                    self.roi.width as _,
                    self.roi.height as _,
                    refimage::ColorSpace::Rgb,
                )
                .map_err(|e| GenCamError::InvalidImageType(e.to_string()))?;
                let img = DynamicImageData::from(img);
                let mut img = refimage::GenericImage::new(SystemTime::now(), img);
                img.insert_key("XOFST", self.roi.x_min as u32)
                    .map_err(|e| {
                        GenCamError::InvalidImageType(format!("Error inserting key: {}", e))
                    })?;
                img.insert_key("YOFST", self.roi.y_min as u32)
                    .map_err(|e| {
                        GenCamError::InvalidImageType(format!("Error inserting key: {}", e))
                    })?;
                Ok(img)
            }
            GenCamState::Downloading(_) => Err(GenCamError::InvalidSequence),
            GenCamState::Errored(gen_cam_error) => Err(gen_cam_error),
            GenCamState::Unknown => Err(GenCamError::InvalidSequence),
        }
    }

    fn image_ready(&self) -> GenCamResult<bool> {
        Ok(self.imgready.load(Relaxed))
    }

    fn camera_state(&self) -> GenCamResult<GenCamState> {
        let capturing = self.capturing.load(SeqCst);
        let imgready = self.imgready.load(Relaxed);
        let state = if capturing && imgready {
            GenCamState::ExposureFinished
        } else if capturing {
            GenCamState::Exposing(Some(self.start.borrow().unwrap().elapsed()))
        } else {
            GenCamState::Idle
        };
        Ok(state)
    }

    fn set_roi(&mut self, roi: &GenCamRoi) -> GenCamResult<&GenCamRoi> {
        let mut roi = *roi;
        roi.x_min = roi.x_min.max(1);
        roi.y_min = roi.y_min.max(1);
        roi.width = roi.width.max(1920);
        roi.height = roi.height.max(1080);
        roi.x_min = roi.x_min.min(1920 - roi.width);
        roi.y_min = roi.y_min.min(1080 - roi.height);
        self.roi = roi;
        Ok(&self.roi)
    }

    fn get_roi(&self) -> &GenCamRoi {
        &self.roi
    }
}
