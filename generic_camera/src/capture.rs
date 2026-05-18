use std::{future::Future, sync::Arc, time::Duration};

use refimage::GenericImageRef;

use crate::{GenCam, GenCamError, GenCamResult, PollExposure};

enum CaptureInner<'cam, C: GenCam + ?Sized> {
    InProgress(&'cam mut C),
    Finished,
}

/// A guard indicating that a capture might be active and cancels the capture operation
/// on `Drop` if still in progress.
pub struct Capturing<'cam, C: GenCam + ?Sized> {
    inner: CaptureInner<'cam, C>,
}
unsafe fn disconnect_lt<'a, 'b>(
    x: GenCamResult<GenericImageRef<'a>>,
) -> GenCamResult<GenericImageRef<'b>>
where
    'b: 'a,
{
    unsafe { std::mem::transmute(x) }
}
impl<'cam, C: GenCam + ?Sized> Capturing<'cam, C> {
    fn new(c: &'cam mut C) -> Self {
        Capturing {
            inner: CaptureInner::InProgress(c),
        }
    }

    /// Capture an image, blocking the current thread until either the capture completes,
    /// an error is returned, or a panic happens. If `self` is finished and already
    /// yielded a result once, returns an [`AccessViolation`] error.
    ///
    /// If a panic happens, the panic is propagated and the capture will still be cancelled.
    pub fn capture(mut self) -> GenCamResult<GenericImageRef<'cam>> {
        loop {
            match self.poll_once() {
                Some(PollExposure::Ready(res)) => break res,
                Some(PollExposure::Wait(dur)) => std::thread::sleep(dur),
                Some(PollExposure::Soon) => continue,
                None => break Err(GenCamError::AccessViolation),
            }
        }
    }

    /// Capture an image, blocking the current task until either the capture completes,
    /// an error is returned, or a panic happens. If `self` is finished and already
    /// yielded a result once, returns an `AccessViolation` error.
    ///
    /// The future returned is cancellation-safe, meaning that if it is dropped before
    /// the capture completes, it will cancel the capture safely.
    ///
    /// If a panic happens, the panic is propagated and the capture will still be cancelled.
    pub async fn capture_async(
        mut self,
        sleeper: impl Sleep,
    ) -> GenCamResult<GenericImageRef<'cam>> {
        loop {
            match self.poll_once() {
                Some(PollExposure::Ready(res)) => break res,
                Some(PollExposure::Wait(dur)) => sleeper.sleep(dur).await,
                Some(PollExposure::Soon) => continue,
                None => break Err(GenCamError::AccessViolation),
            }
        }
    }

    /// Polls the inner [`GenCam`]'s capture once, returning the result of polling. If `self` is already finished
    /// and has already returned a result, this returns [`None`].
    pub fn poll_once(&mut self) -> Option<PollExposure<'cam>> {
        let inner = std::mem::replace(&mut self.inner, CaptureInner::Finished);
        let mut this = Self { inner };
        let res = match &mut this.inner {
            CaptureInner::Finished => None,
            CaptureInner::InProgress(cam) => match cam.poll_exposure() {
                PollExposure::Ready(res) => {
                    // SAFETY:
                    //
                    // The return value does not actually live as long as `'_` but
                    // `'cam`, but the compiler can only give us access to `'_` since
                    // we can't move out of this. In fact, in general it would be unsound
                    // for the compiler to give us `'cam` since this type has a `Drop`
                    // and the drop would need to access `'cam`.
                    //
                    // However, we know that we can only possibly give out
                    // `'cam` once since we always set the state to `Finished`
                    // when giving it out, and `Finished` cannot possibly access `'cam`
                    let res = Some(PollExposure::Ready(unsafe { disconnect_lt(res) }));

                    // We already took the guard from self, now we need to take
                    // it away from `this` to prevent cancelling when we just finished
                    std::mem::forget(this);
                    return res;
                }
                // We need to write out these ones manually because of the almighty Polonius
                // the crab.
                PollExposure::Wait(dur) => Some(PollExposure::Wait(dur)),
                PollExposure::Soon => {
                    #[cfg(all(feature = "loom", not(doctest)))]
                    {
                        loom::thread::yield_now();
                    }
                    Some(PollExposure::Soon)
                }
            },
        };
        // Now we can put the guard back
        *self = this;
        res
    }
}

impl<'cam, C: GenCam + ?Sized> Drop for Capturing<'cam, C> {
    fn drop(&mut self) {
        match &mut self.inner {
            CaptureInner::InProgress(cam) => _ = cam.cancel_capture(),
            CaptureInner::Finished => {}
        }
    }
}
/// A high-level extension trait for capturing a frame based on the low-level methods in [`GenCam`].
///
/// This trait does not include sugar for an async capturing. See [`CaptureAsync`] for that.
pub trait Capture: GenCam {
    /// Creates a guard that can be used to progress a frame capture and cancel the capture on `Drop`.
    fn capture_guard(&mut self) -> GenCamResult<Capturing<'_, Self>> {
        self.start_exposure()?;
        Ok(Capturing::new(self))
    }

    /// Capture an image, blocking the current thread until either the capture completes,
    /// an error is returned, or a panic happens.
    ///
    /// If a panic happens, the panic is propagated and the capture will still be cancelled.
    ///
    /// This is sugar for the corresponding method on the capture guard.
    fn capture(&mut self) -> GenCamResult<GenericImageRef<'_>> {
        self.capture_guard()?.capture()
    }
}

impl<C: GenCam + ?Sized> Capture for C {}

/// High-level extension trait for capturing frames asynchronously from a [`GenCam`].
pub trait CaptureAsync<S: Sleep>: Capture {
    /// Starts a capture, blocking until it starts and then returns a future that blocks
    /// the current task until either the capture completes, an error is
    /// returned, or a panic happens. Before the future is returned, the
    /// current thread may be blocked waiting for the device to not be busy,
    /// but it won't block waiting for the
    ///
    /// The future returned is cancellation-safe, meaning that if it is dropped before
    /// the capture completes, it will cancel the capture safely.
    ///
    /// If a panic happens, the panic is propagated and the capture will still be cancelled.
    ///
    /// This is sugar for the corresponding method on the capture guard.
    fn capture_async<'s>(
        &'s mut self,
        sleeper: S,
    ) -> GenCamResult<impl Future<Output = GenCamResult<GenericImageRef<'s>>> + 's>
    where
        S: 's,
    {
        Ok(self.capture_guard()?.capture_async(sleeper))
    }
}

impl<C: GenCam + ?Sized, S: Sleep> CaptureAsync<S> for C {}

/// Helper for letting a task sleep, abstracting over different async backends
pub trait Sleep {
    /// Tells the current async task to be put to sleep for approximately `duration` amount of time.
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + Sync + 'static;
}

impl<T: Sleep + ?Sized> Sleep for Arc<T> {
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + Sync + 'static {
        (**self).sleep(duration)
    }
}

/// An implementation of [`Sleep`] for the tokio runtime
#[cfg(any(feature = "tokio"))]
pub struct TokioSleep;

#[cfg(feature = "tokio")]
impl Sleep for TokioSleep {
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + Sync + 'static {
        tokio::time::sleep(duration)
    }
}

#[cfg(test)]
mod test {
    #![expect(unused_imports, reason = "I'll fix later")]
    use std::time::Duration;

    use crate::{
        AnyGenCam, Capture, GenCamCtrl, GenCamDriver, GenCamState, dummy::GenCamDriverDummy,
    };

    #[cfg(feature = "loom")]
    use loom::{sync, thread};

    #[cfg(not(feature = "loom"))]
    use std::{sync, thread};
    fn make_dummy() -> AnyGenCam {
        let mut dummy = GenCamDriverDummy {};
        let mut cam = dummy.connect_first_device().unwrap();
        let time = if cfg!(feature = "loom") {
            Duration::from_micros(0)
        } else {
            Duration::from_millis(100).into()
        };
        cam.set_property(
            GenCamCtrl::Exposure(crate::controls::ExposureCtrl::ExposureTime),
            &time.into(),
        )
        .unwrap();
        cam
    }
    #[cfg(not(feature = "loom"))]
    fn model(x: impl Fn() + Send + Sync + 'static) {
        x()
    }
    #[cfg(feature = "loom")]
    fn model(x: impl Fn() + Send + Sync + 'static) {
        unsafe {
            // we have some spinloops that no matter what we do, loom will
            // hate us
            std::env::set_var("LOOM_MAX_PREEMPTIONS", "3");
        }
        loom::model(x)
    }
    #[test]
    fn dummy_cancel_err() {
        model(|| {
            let cam = make_dummy();
            assert_eq!(
                cam.cancel_capture(),
                Err(crate::GenCamError::ExposureNotStarted)
            );
        })
    }
    #[test]
    fn dummy_starts_idle() {
        model(|| {
            let cam = make_dummy();
            assert_eq!(cam.camera_state().unwrap(), GenCamState::Idle);
        })
    }
    #[test]
    fn dummy_capture_finishes() {
        model(|| {
            let mut cam = make_dummy();
            _ = cam.capture().unwrap();
            assert_eq!(cam.camera_state().unwrap(), GenCamState::ExposureFinished);
        })
    }
    #[test]
    fn cancel_on_drop() {
        model(|| {
            let mut cam = make_dummy();
            let guard = cam.capture_guard().unwrap();
            drop(guard);
            assert!(!cam.is_capturing());
        });
    }
    #[test]
    fn dummy_start_exposure_twice_err() {
        model(|| {
            let mut cam = make_dummy();
            _ = cam.start_exposure().unwrap();
            assert_eq!(
                cam.start_exposure(),
                Err(crate::GenCamError::ExposureInProgress)
            );
        })
    }
    #[test]
    fn dummy_start_exposure_guard_err() {
        model(|| {
            let mut cam = make_dummy();
            _ = cam.start_exposure();
            let guard = cam.capture_guard();
            assert!(guard.is_err());
        })
    }
}
