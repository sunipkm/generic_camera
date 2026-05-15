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
    cell::UnsafeCell,
    collections::HashMap,
    fmt::Debug,
    future::Future,
    hint::unreachable_unchecked,
    sync::{
        atomic::{fence, AtomicBool, AtomicU8, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

use rand::{thread_rng, Rng};

use refimage::{DynamicImageRef, GenericImageRef, ImageRef};

use crate::{
    controls::ExposureCtrl, property::PropertyLims, GenCam, GenCamCtrl, GenCamDescriptor,
    GenCamDriver, GenCamError, GenCamResult, GenCamRoi, GenCamState, PollExposure, Property,
    PropertyError, PropertyValue,
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
            vals: Mutex::new(vals),
            // capturing: Arc::new(AtomicBool::new(false)),
            roi: GenCamRoi {
                x_min: 0,
                y_min: 0,
                width: 1920,
                height: 1080,
            },
            data: vec![0; 1920 * 1080 * 3],
            // imgready: Arc::new(AtomicBool::new(false)),
            capture_state: Arc::new(CaptureState::new()), // start: AtomicOptionInstant::none(),
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

/// The capture state for the dummy camera
struct CaptureState {
    state: AtomicU8,
    start_time: UnsafeCell<Instant>,
}
unsafe impl Send for CaptureState {}
unsafe impl Sync for CaptureState {}
impl Debug for CaptureState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaptureState")
            .field("state", &self.state)
            .finish()
    }
}
fn preserve_or_store(
    x: &AtomicU8,
    preserve: u8,
    store: u8,
    success: Ordering,
    failure: Ordering,
) -> u8 {
    let mut prev = x.load(failure);
    while prev != preserve {
        match x.compare_exchange_weak(prev, store, success, failure) {
            Ok(x) => return x,
            Err(next_prev) => prev = next_prev,
        }
    }
    preserve
}
impl CaptureState {
    /// The camera is idle
    const IDLE: u8 = 0;
    /// Someone has access to or is updating the starting time.
    /// This then needs to transition to capturing
    const WAITING_FOR_TIME: u8 = 1;
    /// We are capturing
    const CAPTURING: u8 = 2;
    /// We have finished a capture
    const READY: u8 = 3;

    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(Self::IDLE),
            start_time: UnsafeCell::new(Instant::now()),
        }
    }
    fn is_state_capturing(x: u8) -> bool {
        [Self::WAITING_FOR_TIME, Self::CAPTURING].contains(&x)
    }
    pub fn is_capturing(&self, order: Ordering) -> bool {
        Self::is_state_capturing(self.state.load(order))
    }
    pub fn start_capture(&self) -> GenCamResult<Instant> {
        let old = preserve_or_store(
            &self.state,
            Self::CAPTURING,
            Self::WAITING_FOR_TIME,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
        if Self::is_state_capturing(old) {
            // If we are already capturing, nothing needs to change and there's no ordering needed.
            return Err(GenCamError::ExposureInProgress);
        }
        // If the state change did not result in an error, we want the change to
        // happen before all of these stores,
        fence(Ordering::AcqRel);
        let now = Instant::now();
        // SAFETY: We have exclusive access over self.start_time. Access to self.start_time
        // is guarded by self.state being WAITING_FOR_TIME
        unsafe { self.start_time.get().write(now) }

        // this can be relaxed since the release part of the AcqRel fence ensures that it
        // happens after the store to self.state
        self.state.store(Self::CAPTURING, Ordering::Relaxed);
        Ok(now)
    }

    pub fn get_state(&self) -> GenCamState {
        match self.state.compare_exchange(
            Self::CAPTURING,
            Self::WAITING_FOR_TIME,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => {
                // SAFETY: we were capturing, now we obtained the lock over the start time.
                let start = unsafe { self.start_time.get().read() };
                self.state.store(Self::CAPTURING, Ordering::Release);
                GenCamState::Exposing(Some(start.elapsed()))
            }
            // Someone else is updating or reading the start time,
            // just spuriously indicate that the exposing time is unknown.
            // We don't want to spin loop.
            Err(Self::WAITING_FOR_TIME) => GenCamState::Exposing(None),
            Err(Self::IDLE) => GenCamState::Idle,
            Err(Self::READY) => GenCamState::ExposureFinished,
            _ => GenCamState::Unknown,
        }
    }
    #[cold]
    fn relax() {
        std::hint::spin_loop();
    }
    #[cold]
    fn relax_harder() {
        std::thread::yield_now();
    }

    fn wait_until_capture_and_then_update_state(&self, update_to: u8) -> GenCamResult<()> {
        let mut spinny_iters = 50u8;
        loop {
            // This spin loop should only happen for at most a few iterations
            match self.state.compare_exchange_weak(
                Self::CAPTURING,
                update_to,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                // We saw capturing, everything is fine now
                Ok(_) => break Ok(()),
                // It is very very unlikely to get here.
                //
                // If we got here, it's almost certain that this is
                // from another thread starting initialization.
                // If we don't immediately yield to the scheduler,
                // it is possible we could end up with priority inversion
                // on some platforms since `Instant::now()` calls into the kernel
                // and could make the calling thread yield the time slice.
                Err(Self::WAITING_FOR_TIME | Self::CAPTURING) if spinny_iters == 0 => {
                    Self::relax_harder()
                }

                // Someone is getting or setting the start time.
                // It is very unlikely to even get here.
                // We need to wait to see whether we actually can cancel.
                //
                // For the first tiny amount of iterations, we simply just spin loop
                // to filter out the times we get here from someone acquiring the lock
                // for reading
                Err(Self::WAITING_FOR_TIME | Self::CAPTURING) => Self::relax(),
                // we saw something that is not a capturing state, even briefly, cancellation fails.
                Err(_) => break Err(GenCamError::ExposureNotStarted),
            }
            spinny_iters = spinny_iters.saturating_sub(1);
        }
    }
    pub fn cancel_capture(&self) -> GenCamResult<()> {
        self.wait_until_capture_and_then_update_state(Self::IDLE)
    }
    pub fn mark_ready(&self) -> GenCamResult<()> {
        self.wait_until_capture_and_then_update_state(Self::READY)
    }
}

#[derive(Debug)]
/// A dummy camera for testing purposes.
pub struct GenCamDummy {
    desc: GenCamDescriptor,
    name: String,
    vendor: String,
    caps: HashMap<GenCamCtrl, Property>,
    vals: Mutex<HashMap<GenCamCtrl, (PropertyValue, bool)>>,
    capture_state: Arc<CaptureState>,
    // capturing: Arc<AtomicBool>,
    // imgready: Arc<AtomicBool>,
    roi: GenCamRoi,
    data: Vec<u8>,
}

impl GenCamDummy {
    fn set_property_impl(
        &mut self,
        name: crate::GenCamCtrl,
        value: &crate::PropertyValue,
        auto: bool,
    ) -> GenCamResult<()> {
        if self.capture_state.is_capturing(Ordering::Relaxed) {
            return Err(GenCamError::ExposureInProgress);
        }
        let mut guard = self
            .vals
            .try_lock()
            .map_err(|_| GenCamError::AccessViolation)?;
        match guard.get_mut(&name) {
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
    fn make_dummy_image(&mut self) -> GenCamResult<GenericImageRef<'_>> {
        if !cfg!(miri) {
            thread_rng().fill(self.data.as_mut_slice());
        }

        let img = ImageRef::new(
            &mut self.data,
            self.roi.width as _,
            self.roi.height as _,
            refimage::ColorSpace::Rgb,
        )
        .map_err(|e| GenCamError::InvalidImageType(e.to_string()))?;
        let img = DynamicImageRef::from(img);
        let mut img = GenericImageRef::new(
            if cfg!(miri) {
                SystemTime::UNIX_EPOCH
            } else {
                SystemTime::now()
            },
            img,
        );
        img.insert_key("XOFST", self.roi.x_min as u32)
            .map_err(|e| GenCamError::InvalidImageType(format!("Error inserting key: {e}")))?;
        img.insert_key("YOFST", self.roi.y_min as u32)
            .map_err(|e| GenCamError::InvalidImageType(format!("Error inserting key: {e}")))?;
        Ok(img)
    }
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
        // deadlock me not
        let guard = self
            .vals
            .try_lock()
            .map_err(|_| GenCamError::AccessViolation)?;
        match guard.get(&name) {
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
    ) -> GenCamResult<()> {
        self.set_property_impl(name, value, false)
    }

    fn set_property_auto(
        &mut self,
        name: crate::GenCamCtrl,
        value: &crate::PropertyValue,
    ) -> GenCamResult<()> {
        self.set_property_impl(name, value, true)
    }

    fn cancel_capture(&self) -> GenCamResult<()> {
        self.capture_state.cancel_capture()
    }

    fn is_capturing(&self) -> bool {
        self.capture_state.is_capturing(Ordering::Relaxed)
    }

    fn start_exposure(&mut self) -> GenCamResult<()> {
        let now = self.capture_state.start_capture()?;
        let (exp, _) = self.get_property(GenCamCtrl::Exposure(ExposureCtrl::ExposureTime))?;

        let exp = exp.try_into().map_err(|e| GenCamError::PropertyError {
            control: GenCamCtrl::Exposure(ExposureCtrl::ExposureTime),
            error: e,
        })?;

        let state = self.capture_state.clone();
        thread::spawn(move || loop {
            if !state.is_capturing(Ordering::Relaxed) {
                break;
            }
            if now.elapsed() >= exp {
                _ = state.mark_ready();
                break;
            }
            thread::sleep(Duration::from_secs_f32(
                thread_rng().gen_range(1.0..15.0) * 0.001,
            ));
        });
        Ok(())
    }
    fn poll_exposure(&mut self) -> PollExposure<'_> {
        fn get_exposure_time_remaining(
            this: &mut GenCamDummy,
            time: Duration,
        ) -> GenCamResult<Duration> {
            let (exp, _) = this.get_property(GenCamCtrl::Exposure(ExposureCtrl::ExposureTime))?;

            let total_exposure_time: Duration =
                exp.try_into().map_err(|e| GenCamError::PropertyError {
                    control: GenCamCtrl::Exposure(ExposureCtrl::ExposureTime),
                    error: e,
                })?;
            Ok(total_exposure_time.saturating_sub(time))
        }
        panic!();
        match self.capture_state.get_state() {
            GenCamState::Exposing(Some(time)) => match get_exposure_time_remaining(self, time) {
                Ok(time) => PollExposure::Wait(time),
                Err(e) => PollExposure::Ready(Err(e)),
            },
            GenCamState::Exposing(None) => PollExposure::Soon,
            GenCamState::ExposureFinished => match self.make_dummy_image() {
                Ok(img) => PollExposure::Ready(Ok(img)),
                Err(e) => PollExposure::Ready(Err(e)),
            },
            _ => PollExposure::Ready(Err(GenCamError::ExposureNotStarted)),
        }
    }

    fn camera_state(&self) -> GenCamResult<GenCamState> {
        Ok(self.capture_state.get_state())
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

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::{
        dummy::{GenCamDriverDummy, GenCamDummy},
        Capture, GenCamCtrl, GenCamDriver, GenCamState,
    };

    #[test]
    fn start_stop_exposure() {
        let mut dummy = GenCamDriverDummy {};
        let mut cam = dummy.connect_first_device().unwrap();
        cam.set_property(
            GenCamCtrl::Exposure(crate::controls::ExposureCtrl::ExposureTime),
            &Duration::from_millis(100).into(),
        )
        .unwrap();
        let img = cam.capture().unwrap();

        // drop(img);
        assert!(matches!(
            cam.camera_state().unwrap(),
            GenCamState::ExposureFinished
        ));
        // cam.capture().unwrap();
        // assert!(matches!(
        //     cam.camera_state().unwrap(),
        //     GenCamState::ExposureFinished
        // ));
    }
}
