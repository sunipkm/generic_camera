//! Random assortment of FFI-related utilities
use std::{
    any::{Any, type_name},
    borrow::Cow,
    ffi::{CStr, CString, c_char, c_int},
    fmt::{Debug, Display},
    hash::Hash,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::Deref,
    ptr::NonNull,
    str::Utf8Error,
};

/// Reserved bytes for a struct. These essentially just act as padding
/// for things that may be added in the future.
///
/// This struct has a [`PartialEq`] implementation that always returns equal
/// since all instances of the struct act as the same (and we don't want to ruin
/// the `PartialEq` derive)
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Reserved<const N: usize> {
    _reserved: [MaybeUninit<u8>; N],
}

impl<const N: usize> PartialEq for Reserved<N> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl<const N: usize> Eq for Reserved<N> {}

impl<const N: usize> Debug for Reserved<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reserved")
    }
}

/// An exclusive raw reference to a slice that has "forgotten" its length
/// and needs to "remember" its length for access
/// Used for passing a slice to C code.
///
/// This type has the exact same invariants as `&mut [T]` except that it has forgotten
/// its length.
#[derive(Debug)]
#[repr(transparent)]
pub struct Buffer<'buf, T>(NonNull<T>, PhantomData<&'buf mut [T]>);

impl<'buf, T> Buffer<'buf, T> {
    /// Consume an exclusive slice, converting it into a [`Buffer`] and length pair. This length is guaranteed
    /// to be valid to call [`Buffer::remember`].
    pub const fn new(buf: &'buf mut [T]) -> (Buffer<'buf, T>, usize) {
        let len = buf.len();
        // SAFETY: this is always not null
        let ptr = unsafe { NonNull::new_unchecked(buf.as_mut_ptr()) };
        (unsafe { Self::from_raw(ptr) }, len)
    }

    /// "Remembers" the length of the [`Buffer`] and reconstructs the original slice.
    ///
    /// # Safety
    /// `length` must be <= the length of the original slice
    pub const unsafe fn remember(self, length: usize) -> &'buf mut [T] {
        // SAFETY: the caller must ensure this is valid
        unsafe { std::slice::from_raw_parts_mut(self.0.as_ptr(), length) }
    }

    /// Performs an operation on an a slice by consuming it as a `(buffer, length)` pair
    /// and then reborrows the original slice.
    #[inline(always)]
    pub fn with<U>(
        slice: &'buf mut [T],
        func: impl for<'a> FnOnce(Buffer<'a, T>, usize) -> U,
    ) -> (&'buf mut [T], U) {
        let (buf, len) = Buffer::new(slice);
        let ptr = buf.0;
        let res = func(buf, len);
        // SAFETY:
        // - We have the right length since we got it from the originating slice
        // - We can construct this `Buffer` safely since:
        //    - The pointer comes from a valid `Buffer`, so it is valid
        //    - We know that result outlives `'buf` since it was derived from an input
        //      that outlives `'buf`
        //    - We have ensured no aliasing can happen for since it is impossible for
        //      `func` to return data that references the buffer because the input to the
        //      function is `'a`, which does not outlive `'buf` and the compiler prevents
        //      the aliasing.
        (unsafe { Self::from_raw(ptr).remember(len) }, res)
    }

    /// Constructs a buffer from a raw pointer
    /// # Safety
    /// See `std::slice::from_raw_parts`
    const unsafe fn from_raw(ptr: NonNull<T>) -> Self {
        Buffer(ptr, PhantomData)
    }

    /// Gets the underlying raw pointer.
    pub const fn as_mut_ptr(&mut self) -> NonNull<T> {
        self.0
    }
}

/// A non-nullable raw mutable pointer with a lifetime attached
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Hash)]
pub struct MutPtr<'a, T> {
    ptr: NonNull<T>,
    _lt: PhantomData<&'a mut MaybeUninit<T>>,
}

impl<'a, T> MutPtr<'a, T> {
    /// Create a raw [`MutPtr`] from an exclusive reference
    ///
    /// # Safety
    /// You must ensure that you do not cause aliasing issues.
    pub const unsafe fn from_mut(r: &'a mut T) -> Self {
        Self {
            ptr: NonNull::from_ref(r),
            _lt: PhantomData,
        }
    }
    /// Create a [`MutPtr`] from a raw pointer.
    ///
    /// # Safety
    /// You must ensure that you don't cause aliasing issues
    /// and that if `ptr` is valid, it is valid for at least `'a`
    /// and that nobody aliases with this `ptr` for `'a`
    pub const unsafe fn from_raw(ptr: NonNull<T>) -> Self {
        Self {
            ptr,
            _lt: PhantomData,
        }
    }
}

impl<T> Deref for MutPtr<'_, T> {
    type Target = NonNull<T>;
    fn deref(&self) -> &Self::Target {
        &self.ptr
    }
}

/// An enum type that may or may not have valid data, used for when the enum value may
/// have more values added in future versions.
#[repr(transparent)]
#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaybeInvalid<T: CEnum>(pub(crate) c_int, pub(crate) PhantomData<T>);

impl<T: CEnum> MaybeInvalid<T> {
    /// Attempts to get the inner value, returning [`None`]
    /// if the value is not valid for the type.
    pub fn get(self) -> Result<T, ValidationError<T>> {
        T::try_from(self.0)
    }

    /// Gets the inner value without checking for validity.
    ///
    /// Currently, in debug mode, this does perform a check
    /// and panics if the value is invalid.
    ///
    /// # Safety
    /// The caller must ensure that the value is valid.
    #[track_caller] // track_caller because we panic in debug builds
    pub unsafe fn get_unchecked(self) -> T {
        // SAFETY: caller must ensure this is valid.
        unsafe { T::convert_unchecked(self.0) }
    }

    pub fn new(data: T) -> Self {
        Self(data.into_value(), PhantomData)
    }

    pub const fn new_from_value(value: c_int) -> Self {
        Self(value, PhantomData)
    }
}

impl<T: Debug + CEnum> Debug for MaybeInvalid<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Ok(v) = self.get() {
            v.fmt(f)
        } else {
            f.debug_tuple("Invalid").field(&self.0).finish()
        }
    }
}
/// A trait marking that a type is a C-style enum that can be validated
///
/// # Safety
/// It should be obvious. Don't implement this.
pub unsafe trait CEnum: Copy + TryFrom<c_int, Error = ValidationError<Self>> {
    #[doc(hidden)]
    unsafe fn convert_unchecked(x: c_int) -> Self;
    #[doc(hidden)]
    fn into_value(self) -> c_int;
}

/// The error for a failed conversion from a c_int to an enum
#[derive(Clone, Copy)]
pub struct ValidationError<T>(c_int, PhantomData<T>);

impl<T> Debug for ValidationError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ValidationError").field(&self.0).finish()
    }
}
impl<T> ValidationError<T> {
    pub(crate) fn new(x: c_int) -> Self {
        Self(x, PhantomData)
    }
}
impl<T: Any> Display for ValidationError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is an invalid value for {}", self.0, type_name::<T>())
    }
}

impl<T: Any> std::error::Error for ValidationError<T> {}

macro_rules! c_enum {
    {
        $(
            #[$meta:meta]
        )*
        $vis:vis enum $name:ident {
            $(
                $(#[$var_meta:meta])*
                $variant:ident $(= $value:expr)?
            ),* $(,)?
        }
    } => {
        $(#[$meta])*
        #[repr(C)]
        $vis enum $name {
            $(
                $(#[$var_meta])*
                $variant $(= $value)?
            ),*
        }
        impl TryFrom<std::ffi::c_int> for $name {
            type Error = $crate::ffi_util::ValidationError<Self>;
            fn try_from(x: std::ffi::c_int) -> ::std::result::Result<Self, Self::Error> {
                #![allow(deprecated)]
                #![allow(non_upper_case_globals)]
            $(
                const $variant: std::ffi::c_int = $name::$variant as _;
            )*
                ::std::result::Result::Ok(match x {
                    $(
                        $variant => Self::$variant,
                    )*
                    __failed => return ::std::result::Result::Err($crate::ffi_util::ValidationError::new(__failed))
                })
            }
        }
        unsafe impl $crate::ffi_util::CEnum for $name {

            #[inline(always)]
            #[track_caller]
            unsafe fn convert_unchecked(x: std::ffi::c_int) -> Self {
                if cfg!(debug_assertions) {
                    Self::try_from(x).expect("Invalid data passed to `get_unchecked`")
                } else {
                    unsafe { std::mem::transmute(x) }
                }
            }
            fn into_value(self) -> c_int {
                self as _
            }
        }
    };
}

pub(crate) use c_enum;

/// Represents that a type has a terminal value, ie., it is used to terminate an array.
/// Byte comparisons are used for checking for the terminal value.
///
/// # Safety
/// - There must be no padding in `Self`
/// - `Self` must consist entirely of initialized bytes
/// - `Self` must have a defined abi and byte layout, ie., `Self` must not be `repr(rust)`.
pub unsafe trait Terminated: Copy {
    const TERMINAL: Self;
}

/// An array of up to `N` elements of type `T` with a sentinel value indicating the
/// array length if the entire array is not filled. That is, if `len < N`, the `len`th
/// element in the array is guaranteed to be the sentinel value.
///
/// This type is designed to make dealing with sentinel-terminated C arrays
/// safer and easier. All comparisons are done by *bitwise comparison* and this type is
/// ABI-compatible and has the same ABI and layout as `[T; N]`
///
/// Given that the type is meant to be ABI-compatible with `[T; N]`, most safe operations
/// need to recompute the length. Therefore it is recommended to keep the returned
/// slice around as long as possible to prevent recomputation.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct TerminatedArray<T: Copy, const N: usize> {
    buf: [MaybeUninit<T>; N],
}

const fn min(x: usize, y: usize) -> usize {
    if x < y { x } else { y }
}

/// Compares the bytewise representation of 2 values
///
/// # Safety
/// - `T` must have a defined byte layout and no uninitialized bytes
const unsafe fn are_bitwise_equal<T: Copy>(x: T, y: T) -> bool {
    let mut x = (&raw const x).cast::<u8>();
    let mut y = (&raw const y).cast::<u8>();
    let mut i = 0;
    unsafe {
        while i < size_of::<T>() {
            if *x != *y {
                return false;
            }
            x = x.add(1);
            y = y.add(1);
            i += 1;
        }
        true
    }
}

impl<T: Terminated, const N: usize> TerminatedArray<T, N> {
    /// Creates a terminated array that is empty and has a buffer initialized with `N` terminal elements.
    pub const fn empty() -> Self {
        Self::from_array([T::TERMINAL; N])
    }

    /// Creates a new [`TerminatedArray`] by copying from a slice. If the slice cannot
    /// fit into the buffer after accounting for any premature truncation, returns [`None`],
    pub const fn from_slice(slice: &[T]) -> Option<TerminatedArray<T, N>> {
        // Quick check before checking the actual data
        if slice.len() <= N {
            Some(unsafe { Self::from_slice_unchecked(slice) })
        } else {
            // If the slice is terminated before N elements, then we can safely copy those
            if let Some(terminated_len) = unsafe {
                Self::terminated_len_from_ptr(NonNull::new_unchecked(slice.as_ptr().cast_mut()))
            } {
                Some(unsafe {
                    Self::from_slice_unchecked(std::slice::from_raw_parts(
                        slice.as_ptr(),
                        terminated_len,
                    ))
                })
            } else {
                None
            }
        }
    }

    /// Creates a new [`TerminatedArray`] by copying from a slice. If the `slice.len() > N`,
    /// the result is truncated to the first `N` elements.
    ///
    /// Note that if the slice contains the terminator within the first `N` elements,
    /// the terminated array is effectively truncated, but data is still continued to
    /// be copied from the slice until either the end of the slice or the end of the buffer
    ///
    /// If `slice.len() < N`, the rest of the buffer is guaranteed to be initialized and
    /// filled with the terminator
    pub const fn from_slice_truncate(slice: &[T]) -> TerminatedArray<T, N> {
        let mut buf = [T::TERMINAL; N];
        // SAFETY: We write at most `N` elements and the buffer can hold `N` elements and read
        // at most `N` elements from the slice.
        // We also know that `buf` and `slice` can't alias because `buf` is a local variable.
        unsafe {
            buf.as_mut_ptr()
                .copy_from_nonoverlapping(slice.as_ptr(), min(slice.len(), N))
        };
        Self::from_array(buf)
    }

    /// Creates a new [`TerminatedArray`] by copying from a slice.
    ///
    /// Note that if the slice contains the terminator within the first `N` elements,
    /// the terminated array is effectively truncated, but data is still continued to
    /// be copied from the slice until either the end of the slice or the end of the buffer
    ///
    /// The remainder of the buffer is guaranteed to be filled with the terminator.
    ///
    /// # Safety
    /// The caller must ensure that `slice.len() <= N`
    pub const unsafe fn from_slice_unchecked(slice: &[T]) -> TerminatedArray<T, N> {
        let mut buf = [T::TERMINAL; N];
        // SAFETY: caller must ensure validity
        unsafe {
            buf.as_mut_ptr()
                .copy_from_nonoverlapping(slice.as_ptr(), slice.len())
        };
        Self::from_array(buf)
    }

    /// Creates a [`TerminatedArray`] with a new capacity from `self`, preserving the length, by creating a new buffer
    /// and copying elements over. If `M < self.compute_len()` the resulting array is guaranteed to not have a terminator.
    /// All data past the first terminator, or N elements, whatever is first, is considered uninitialized.
    pub const fn extend_or_truncate<const M: usize>(self) -> TerminatedArray<T, M> {
        if M == N {
            // SAFETY: We are literally the same type and can be bitwise copied
            return unsafe { std::mem::transmute_copy(&self) };
        }
        if M <= N {
            // SAFETY: `self` is guaranteed to either be terminated or have the first `N` elements initialized
            // and M <= N
            return unsafe { TerminatedArray::from_ptr(self.as_nonnull()) };
        }
        // we could do it the less efficient way and do `TerminatedArray::from_slice(self.to_slice())`,
        // but that is less efficient than just a straight copy if there isn't a lot of data, especially
        // if size_of::<T> != 1
        let mut buf = [MaybeUninit::uninit(); M];

        // SAFETY: M > N and initialization status is preserved
        unsafe {
            buf.as_mut_ptr()
                .copy_from_nonoverlapping(self.buf.as_ptr(), N);
        };
        // Now guarantee that the new buffer has a terminator no matter the true length
        //
        // SAFETY: M > N, therefore M >= N + 1, meaning that writing at offset N is always
        // in bounds
        unsafe {
            buf.as_mut_ptr().add(N).cast::<T>().write(T::TERMINAL);
        };
        TerminatedArray { buf }
    }
    /// Reads a [`TerminatedArray`] from a raw pointer, reading elements until it finds a terminator, up to `N` elements.
    /// This is not guaranteed to initialize the remainder of the internal buffer with the terminator.
    ///
    /// # Safety
    /// - `ptr` must be valid for reads of up to `min(terminator_pos, N)` elements
    pub const unsafe fn from_ptr(mut ptr: NonNull<T>) -> TerminatedArray<T, N> {
        let mut buf = [MaybeUninit::<T>::uninit(); N];
        let mut buf_ptr = buf.as_mut_ptr();
        let mut i = 0;
        // SAFETY: the caller must guarantee safety
        unsafe {
            // ideally we'd have something like a nice and fast memccpy, but
            // nobody likes that function
            while i < N {
                let elem = *ptr.as_ptr();
                *buf_ptr.cast() = elem;

                if are_bitwise_equal(elem, T::TERMINAL) {
                    break;
                }

                ptr = ptr.add(1);
                buf_ptr = buf_ptr.add(1);

                i += 1
            }
        }
        TerminatedArray { buf }
    }

    /// Computes the length of the array including the first terminator if any. This is an O(len) operation.
    pub const fn terminated_len(&self) -> Option<usize> {
        // SAFETY: our invariants say this is valid
        unsafe { Self::terminated_len_from_ptr(self.as_nonnull()) }
    }

    /// Computes the length of the array. This is O(len) operation.
    pub const fn compute_len(&self) -> usize {
        if let Some(len) = self.terminated_len() {
            len + 1
        } else {
            N
        }
    }

    const unsafe fn slice_up_to(&self, len: usize) -> &[T] {
        let ptr = self.as_ptr();
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }

    const unsafe fn slice_up_to_mut(&mut self, len: usize) -> &mut [T] {
        let ptr = self.as_mut_ptr();
        unsafe { std::slice::from_raw_parts_mut(ptr, len) }
    }

    /// Converts the [`TerminatedArray`] into a slice including the first terminator if any. This is an `O(len)`
    /// operation since it has to compute the length. The returned slice is guaranteed to have at
    /// least 1 element.
    pub const fn to_slice_with_terminator(&self) -> Option<&[T]> {
        if let Some(len) = self.terminated_len() {
            // SAFETY: we know that the buffer is at least initialized to the terminator
            Some(unsafe { self.slice_up_to(len) })
        } else {
            None
        }
    }

    /// Converts the [`TerminatedArray`] into a slice including the first terminator if any.
    /// If there is no terminator, the full slice is still returned. This is an `O(len)`
    /// operation since it has to compute the length. The returned slice is guaranteed to have at
    /// least 1 element.
    pub const fn to_slice_with_terminator_or_full(&self) -> &[T] {
        let len = if let Some(len) = self.terminated_len() {
            len
        } else {
            N
        };
        // SAFETY: If the terminated length is None, that means that all elements in the array are initialized
        unsafe { self.slice_up_to(len) }
    }

    /// Converts the [`TerminatedArray`] into a mutable slice including the first terminator if any. This is an `O(len)`
    /// operation since it has to compute the length
    ///
    /// # Safety
    /// The terminator should not be overwritten unless the array would still be valid, ie.
    /// there must either be another terminator or all of the data in the buffer must be valid
    pub const unsafe fn to_slice_with_terminator_mut(&mut self) -> Option<&mut [T]> {
        if let Some(len) = self.terminated_len() {
            // SAFETY: we know that the buffer is at least initialized to the terminator
            Some(unsafe { self.slice_up_to_mut(len) })
        } else {
            None
        }
    }

    /// Converts the [`TerminatedArray`] into a slice including the first terminator if any.
    /// If there is no terminator, the full slice is still returned. This is an `O(len)`
    /// operation since it has to compute the length. The returned slice is guaranteed to have at
    /// least 1 element.
    ///
    /// # Safety
    /// The terminator should not be overwritten unless the array would still be valid, ie.
    /// there must either be another terminator or all of the data in the buffer must be valid.
    pub const fn to_slice_with_terminator_or_full_mut(&mut self) -> &mut [T] {
        let len = if let Some(len) = self.terminated_len() {
            len
        } else {
            N
        };
        // SAFETY: If the terminated length is None, that means that all elements in the array are initialized
        unsafe { self.slice_up_to_mut(len) }
    }

    /// Converts the [`TerminatedArray`] into a slice up to, but not including, the first terminator.
    /// The returned slice is guaranteed to contain no terminator.
    /// This is an `O(len)` operation since it has to compute the length.
    pub const fn to_slice(&self) -> &[T] {
        let len = if let Some(len) = self.terminated_len() {
            len - 1
        } else {
            N
        };
        // SAFETY: we are valid up until the terminator if there is one and len < terminator_pos
        unsafe { self.slice_up_to(len) }
    }

    /// Converts the [`TerminatedArray`] into a slice up to, but not including, the first terminator.
    /// The returned slice is guaranteed to contain no terminator. This is an `O(len)` operation since it
    ///  has to compute the length.
    ///
    /// This is always safe since there is no terminator in the slice; the array cannot grow from mutating
    /// this portion
    pub const fn to_slice_mut(&mut self) -> &mut [T] {
        let len = if let Some(len) = self.terminated_len() {
            len - 1
        } else {
            N
        };
        // SAFETY: we are valid up until the terminator if there is one and len < terminator_pos
        unsafe { self.slice_up_to_mut(len) }
    }

    // const unsafe fn len_from_ptr(ptr: NonNull<T>) -> Option<T>
    const unsafe fn terminated_len_from_ptr(ptr: NonNull<T>) -> Option<usize> {
        let mut p = ptr.as_ptr();
        let mut i = 0;
        unsafe {
            while i < N {
                i += 1;
                if are_bitwise_equal(*p, T::TERMINAL) {
                    return Some(i);
                }
                p = p.add(1);
            }
            None
        }
    }

    /// Converts the [`TerminatedArray`] into a slice that  is guaranteed to be terminated
    /// with the terminator by moving data to the heap if there is no terminator.
    pub fn guarantee_termination(&self) -> Cow<'_, [T]> {
        if let Some(len) = self.terminated_len() {
            Cow::Borrowed(unsafe { self.slice_up_to(len) })
        } else {
            let mut v = unsafe { self.slice_up_to(N) }.to_vec();
            v.reserve_exact(1);
            v.push(T::TERMINAL);
            Cow::Owned(v)
        }
    }
}

impl<T: Copy, const N: usize> TerminatedArray<T, N> {
    /// Creates a [`TerminatedArray`] from a regular array, effectively truncating the array to the first terminator if there is one.
    /// This is always safe since there is no termination guarantee for `N` elements.
    pub const fn from_array(array: [T; N]) -> TerminatedArray<T, N> {
        Self {
            buf: transpose_mu(MaybeUninit::new(array)),
        }
    }

    /// Transmute a [`TerminatedArray`] to a new new element type with the same layout and validity
    /// as `T`. This is a very unsafe operation, but useful for casting between repr(transparent)
    /// wrappers.
    ///
    /// A compile-time error is emitted if `size_of::<T>() != size_of::<U>()`
    ///
    /// # Safety
    /// - Both `U` and `T` must have no uninitialized bytes within them, including padding.
    /// - All elements in the buffer must be valid when transmuted to type `U`
    /// - If `U: Terminated`, and there are any uninitialized elements in `self`'s buffer, there must be a
    ///   an element with bitwise equivalence to `U::TERMINAL` before that uninitialized element
    /// - All other invariants of [`TerminatedArray`] must hold for `U`
    /// - All other invariants for transmuting `[MaybeUninit<T>; N]` to `[MaybeUninit<U>; N]` must hold
    /// - Basically, just don't do this unless either `U` or `T` is a `repr(transparent)` wrapper around the other
    pub const unsafe fn transmute<U: Copy>(self) -> TerminatedArray<U, N> {
        const {
            assert!(
                size_of::<T>() == size_of::<U>(),
                "Cannot transmute between `TerminatedArray`s with elements of different sizes"
            );
            assert!(
                size_of::<TerminatedArray<T, N>>() == size_of::<TerminatedArray<U, N>>(),
                "Cannot transmute between `TerminatedArray`s with elements that have padding"
            );
        };
        // SAFETY: the caller must ensure that the transmute is valid. We at least made sure that
        // the sizes match
        unsafe { std::mem::transmute_copy(&self) }
    }

    /// Transmute a [`TerminatedArray`] to a new element type `U` that might have a different size
    /// from `T`. This is a very *very* unsafe operation if you're not careful, but it does have its uses.
    ///
    /// A compile-time error if `size_of::<TerminatedArray<U, M>>() != size_of::<Self>()`
    /// # Safety
    /// - See [`Self::transmute`] and [`std::mem::transmute`] for details
    pub const unsafe fn transmute_with_new_elem_size<U: Copy, const M: usize>(
        self,
    ) -> TerminatedArray<U, M> {
        const {
            assert!(
                size_of::<TerminatedArray<T, N>>() == size_of::<TerminatedArray<U, N>>(),
                "Cannot transmute between `TerminatedArray`s with elements that have padding"
            );
        };
        // SAFETY: the caller must ensure that the transmute is valid. We at least made sure that
        // the sizes match
        unsafe { std::mem::transmute_copy(&self) }
    }

    /// Gets the entire potentially uninitialized buffer of the [`TerminatedArray`].
    pub const fn buffer(&self) -> &[MaybeUninit<T>; N] {
        &self.buf
    }
    /// Gets the entire potentially uninitialized buffer of the [`TerminatedArray`].
    /// # Safety
    /// You must never uninitialize any elements before the first terminator
    pub const unsafe fn buffer_mut(&mut self) -> &mut [MaybeUninit<T>; N] {
        &mut self.buf
    }

    /// Gets a pointer to the underlying buffer
    pub const fn as_ptr(&self) -> *const T {
        self.buf.as_ptr().cast()
    }

    /// Gets a mut pointer to the underlying buffer
    pub const fn as_mut_ptr(&mut self) -> *mut T {
        self.buf.as_mut_ptr().cast()
    }

    /// Gets a non-null pointer to the underlying buffer
    pub const fn as_nonnull(&self) -> NonNull<T> {
        unsafe { NonNull::new_unchecked(self.buf.as_ptr().cast_mut().cast()) }
    }
}

impl<T: Eq + Terminated, const N: usize> Eq for TerminatedArray<T, N> {}

impl<T, U, const M: usize, const N: usize> PartialEq<TerminatedArray<U, M>>
    for TerminatedArray<T, N>
where
    T: PartialEq + Terminated + PartialEq<U>,
    U: Terminated,
{
    fn eq(&self, other: &TerminatedArray<U, M>) -> bool {
        self.to_slice() == other.to_slice()
    }
}

impl<T, U, const N: usize> PartialEq<[U]> for TerminatedArray<T, N>
where
    T: PartialEq<U> + Terminated,
{
    fn eq(&self, other: &[U]) -> bool {
        self.to_slice() == other
    }
}

impl<T, U, const M: usize, const N: usize> PartialOrd<TerminatedArray<U, M>>
    for TerminatedArray<T, N>
where
    T: Terminated + PartialOrd<U> + PartialEq,
    U: Terminated,
{
    fn partial_cmp(&self, other: &TerminatedArray<U, M>) -> Option<std::cmp::Ordering> {
        self.to_slice().iter().partial_cmp(other.to_slice())
    }
}

impl<T, U, const N: usize> PartialOrd<[U]> for TerminatedArray<T, N>
where
    T: PartialOrd<U> + Terminated,
{
    fn partial_cmp(&self, other: &[U]) -> Option<std::cmp::Ordering> {
        self.to_slice().iter().partial_cmp(other)
    }
}
impl<T, const N: usize> Ord for TerminatedArray<T, N>
where
    T: Ord + Terminated,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_slice().cmp(other.to_slice())
    }
}
impl<T: Hash, const N: usize> Hash for TerminatedArray<T, N>
where
    T: Terminated,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.to_slice().hash(state);
    }
}
impl<T, const N: usize> Debug for TerminatedArray<T, N>
where
    T: Debug + Terminated,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_slice().fmt(f)
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a TerminatedArray<T, N>
where
    T: Terminated,
{
    type IntoIter = std::slice::Iter<'a, T>;
    type Item = &'a T;
    fn into_iter(self) -> Self::IntoIter {
        self.to_slice().iter()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a mut TerminatedArray<T, N>
where
    T: Terminated,
{
    type IntoIter = std::slice::IterMut<'a, T>;
    type Item = &'a mut T;
    fn into_iter(self) -> Self::IntoIter {
        self.to_slice_mut().iter_mut()
    }
}

unsafe impl Terminated for c_char {
    const TERMINAL: Self = 0;
}

/// A C string that is bounded to a maximum `N` bytes, guaranteed to be null-terminated if `len < N`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundedCString<const N: usize> {
    data: TerminatedArray<c_char, N>,
}

impl<const N: usize> PartialEq<CStr> for BoundedCString<N> {
    fn eq(&self, other: &CStr) -> bool {
        self.to_bytes() == other.to_bytes()
    }
}

impl<const N: usize> PartialEq<str> for BoundedCString<N> {
    fn eq(&self, other: &str) -> bool {
        self.to_bytes() == other.as_bytes()
    }
}

impl<const N: usize> Debug for BoundedCString<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let slice = self.to_bytes();
        write!(f, "\"")?;
        for chunk in slice.utf8_chunks() {
            for c in chunk.valid().chars() {
                match c {
                    '\x01'..='\x7f' => write!(f, "{}", (c as u8).escape_ascii())?,
                    _ => write!(f, "{}", c.escape_debug())?,
                }
            }
            write!(f, "{}", chunk.invalid().escape_ascii())?;
        }
        write!(f, "\"")?;
        Ok(())
    }
}

impl<const N: usize> Display for BoundedCString<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: do this efficiently without allocating
        f.write_str(&self.to_str_lossy())
    }
}

impl<const N: usize> BoundedCString<N> {
    /// Creates a [`BoundedCString`] from a string, truncating the returned string
    /// if it contains a nul byte, returning `None` if the string is too long.
    pub const fn from_str(data: &str) -> Option<Self> {
        Self::from_bytes(data.as_bytes())
    }

    /// Creates a [`BoundedCString`] from a byte slice, truncating the returned string
    /// if it contains a nul byte, returning `None` if the string is too long.
    pub const fn from_bytes(data: &[u8]) -> Option<Self> {
        Self::from_c_chars(bytes_to_c_char(data))
    }

    /// Creates a [`BoundedCString`] from a [`c_char`] slice, truncating the returned string
    /// if it contains a nul byte, returning `None` if the string is too long.
    pub const fn from_c_chars(data: &[c_char]) -> Option<Self> {
        if let Some(data) = TerminatedArray::from_slice(data) {
            Some(Self { data })
        } else {
            None
        }
    }

    /// Creates a [`BoundedCString`] from a string, truncating the returned string
    /// if it contains a null byte as well as truncating the string to `N` bytes if it is too long.
    ///
    /// This truncation is guaranteed to preserve the UTF-8 validity of the string.
    pub const fn from_str_truncate(data: &str) -> Self {
        let last_cp = data.floor_char_boundary(N);
        let slice = unsafe { std::slice::from_raw_parts(data.as_ptr(), last_cp) };

        Self {
            // SAFETY: slice is at most length N
            data: unsafe { TerminatedArray::from_slice_unchecked(bytes_to_c_char(slice)) },
        }
    }

    /// Creates a [`BoundedCString`] from a byte slice, truncating the returned string
    /// if it contains a null byte as well as truncating the string to `N` bytes if it is too long
    pub const fn from_bytes_truncate(data: &[u8]) -> Self {
        Self::from_c_chars_truncate(bytes_to_c_char(data))
    }

    /// Creates a [`BoundedCString`] from a [`c_char`] slice, truncating the returned string
    /// if it contains a null byte as well as truncating the string to `N` bytes if it is too long
    pub const fn from_c_chars_truncate(data: &[c_char]) -> Self {
        Self {
            data: TerminatedArray::from_slice_truncate(data),
        }
    }

    /// Creates a [`BoundedCString`] from a [`CStr`], truncating the returned string
    ///truncating the string to `N` bytes if it is too long.
    pub const fn from_cstr_truncate(data: &CStr) -> Self {
        Self {
            // SAFETY: `data` is guaranteed to be null-terminated or have a len >= N
            data: unsafe {
                TerminatedArray::from_ptr(NonNull::new_unchecked(data.as_ptr().cast_mut()))
            },
        }
    }

    /// Converts the [`BoundedCString`] into a [`c_char`] slice without the trailing nul byte.
    ///
    /// This is an `O(len)` operation since the length has to be computed.
    pub const fn to_c_chars(&self) -> &[c_char] {
        self.data.to_slice()
    }

    /// Converts the [`BoundedCString`] into a [`c_char`] slice without the trailing nul byte.
    ///
    /// This is an `O(len)` operation since the length has to be computed.
    pub const fn to_c_chars_mut(&self) -> &[c_char] {
        self.data.to_slice()
    }
    /// Converts the [`BoundedCString`] into a byte slice without the trailing nul byte.
    ///
    /// This is an `O(len)` operation since the length has to be computed.
    pub const fn to_bytes(&self) -> &[u8] {
        c_char_to_bytes(self.to_c_chars())
    }

    /// Converts the [`BoundedCString`] into a byte slice without the trailing nul byte.
    ///
    /// This is an `O(len)` operation since the length has to be computed.
    pub const fn to_bytes_mut(&mut self) -> &mut [u8] {
        c_char_to_bytes_mut(self.data.to_slice_mut())
    }

    /// Attempts to convert the [`BoundedCString`] into a byte slice with the trailing nul byte if there is one
    pub const fn to_bytes_with_nul(&self) -> Option<&[u8]> {
        if let Some(slice) = self.to_c_chars_with_nul() {
            Some(c_char_to_bytes(slice))
        } else {
            None
        }
    }

    /// Attempts to convert the [`BoundedCString`] into a byte slice with the trailing nul byte if there is one
    ///
    /// # Safety
    /// The nul terminator should not be overwritten unless the array would still be valid, ie.
    /// there must either be another terminator or all of the data in the buffer must be valid
    pub const unsafe fn to_bytes_with_nul_mut(&mut self) -> Option<&mut [u8]> {
        // SAFETY: the caller must ensure this is valid
        if let Some(slice) = unsafe { self.to_c_chars_with_nul_mut() } {
            Some(c_char_to_bytes_mut(slice))
        } else {
            None
        }
    }
    /// Attempts to convert the [`BoundedCString`] into a [`c_char`] slice with the trailing nul byte if there is one
    pub const fn to_c_chars_with_nul(&self) -> Option<&[c_char]> {
        self.data.to_slice_with_terminator()
    }

    /// Attempts to convert the [`BoundedCString`] into a [`c_char`] slice with the trailing nul byte if there is one
    ///
    /// # Safety
    /// The nul terminator should not be overwritten unless the array would still be valid, ie.
    /// there must either be another terminator or all of the data in the buffer must be valid
    pub const unsafe fn to_c_chars_with_nul_mut(&mut self) -> Option<&mut [c_char]> {
        unsafe { self.data.to_slice_with_terminator_mut() }
    }

    /// Attempts to convert the [`BoundedCString`] into an `&`[`CStr`], returning [`None`]
    /// there is no nul byte. This is an `O(len)` operation since the length has to be computed every time
    pub const fn to_cstr(&self) -> Option<&CStr> {
        if let Some(buf) = self.to_bytes_with_nul() {
            // SAFETY: buf is guaranteed to have exactly 1 terminator
            Some(unsafe { CStr::from_bytes_with_nul_unchecked(buf) })
        } else {
            None
        }
    }
    /// Converts the [`BoundedCString`] into a nul-terminated C-string, moving
    /// the data to the heap if there is no terminator.
    pub fn to_cstring(&self) -> Cow<'_, CStr> {
        // SAFETY: we are guaranteed to have nul termination
        unsafe {
            match self.data.guarantee_termination() {
                Cow::Borrowed(x) => {
                    Cow::Borrowed(CStr::from_bytes_with_nul_unchecked(c_char_to_bytes(x)))
                }
                Cow::Owned(x) => {
                    Cow::Owned(CString::from_vec_with_nul_unchecked(c_char_to_byte_vec(x)))
                }
            }
        }
    }
    /// Returns an empty [`BoundedCString`] with a guaranteed zeroed buffer.
    pub const fn zeroed() -> Self {
        // SAFETY: We are guaranteed to be intialized with all zero bytes
        unsafe { std::mem::zeroed() }
    }

    /// Returns a reference to the underlying buffer. This buffer may be unitialized, but is guaranteed
    /// to be initialized at least until the first null byte
    pub const fn buffer(&self) -> &[MaybeUninit<c_char>; N] {
        self.data.buffer()
    }

    /// Returns an exclusive reference to the underlying buffer.
    ///
    /// # Safety
    /// You must ensure that the buffer is initialized up until the first null byte before accessing the
    /// [`BoundedCString`] again
    pub const unsafe fn buffer_mut(&mut self) -> &mut [MaybeUninit<c_char>; N] {
        unsafe { self.data.buffer_mut() }
    }

    /// Attempts to convert the string to an [`&str`]
    pub const fn try_to_str(&self) -> Result<&str, Utf8Error> {
        std::str::from_utf8(self.to_bytes())
    }

    /// Converts the string to an [`&str`] without checking UTF-8 validity
    ///
    /// # Safety
    /// The internal string must be valid UTF-8
    pub const unsafe fn to_str_unchecked(&self) -> &str {
        // SAFETY: Caller must ensure this is safe
        unsafe { std::str::from_utf8_unchecked(self.to_bytes()) }
    }

    /// Converts the string to an `&mut str` without checking UTF-8 validity
    ///
    /// # Safety
    /// The internal string must be valid UTF-8
    pub const unsafe fn to_str_unchecked_mut(&mut self) -> &mut str {
        // SAFETY: Caller must ensure this is safe
        unsafe { std::str::from_utf8_unchecked_mut(self.to_bytes_mut()) }
    }

    /// Lossily converts the [`BoundedCString`] to a string
    pub fn to_str_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(self.to_bytes())
    }
}

const fn transpose_mu<T, const N: usize>(x: MaybeUninit<[T; N]>) -> [MaybeUninit<T>; N] {
    // SAFETY: this is always safe since both have the same layout and can both be uninitialized
    unsafe { std::mem::transmute_copy(&x) }
}
const fn c_char_to_bytes(x: &[c_char]) -> &[u8] {
    const {
        assert!(
            size_of::<c_char>() == size_of::<u8>(),
            "What kind of degenerate platform are you on"
        )
    };
    // SAFETY: this is always safe since we can be sure that the layout is the same
    unsafe { std::slice::from_raw_parts(x.as_ptr().cast(), x.len()) }
}

const fn c_char_to_bytes_mut(x: &mut [c_char]) -> &mut [u8] {
    const {
        assert!(
            size_of::<c_char>() == size_of::<u8>(),
            "What kind of degenerate platform are you on"
        )
    };
    // SAFETY: this is always safe since we can be sure that the layout is the same
    unsafe { std::slice::from_raw_parts_mut(x.as_mut_ptr().cast(), x.len()) }
}

const fn bytes_to_c_char(x: &[u8]) -> &[c_char] {
    const {
        assert!(
            size_of::<c_char>() == size_of::<u8>(),
            "What kind of degenerate platform are you on"
        )
    };
    // SAFETY: this is always safe since we can be sure that the layout is the same
    unsafe { std::slice::from_raw_parts(x.as_ptr().cast(), x.len()) }
}

fn c_char_to_byte_vec(mut x: Vec<c_char>) -> Vec<u8> {
    const {
        assert!(
            size_of::<c_char>() == size_of::<u8>(),
            "What kind of degenerate platform are you on"
        )
    };
    // SAFETY: this is always valid since `c_char` and `u8` have the same layout
    unsafe {
        let res = Vec::from_raw_parts(x.as_mut_ptr().cast(), x.len(), x.capacity());
        std::mem::forget(x);
        res
    }
}
#[cfg(test)]
mod test {
    use std::os::raw::c_char;

    use crate::TerminatedArray;

    #[test]
    fn doesnt_explode() {
        let mut x: TerminatedArray<c_char, 8> =
            TerminatedArray::from_slice_truncate(&[1, 2, 3, 4, 5, 6, 7, 8, 0]);
        assert_eq!(x.to_slice(), [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(x.to_slice_with_terminator(), None);
        assert_eq!(
            x.to_slice_with_terminator_or_full(),
            [1, 2, 3, 4, 5, 6, 7, 8]
        );
        assert_eq!(x.to_slice_mut(), [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(unsafe { x.to_slice_with_terminator_mut() }, None,);
    }
}
