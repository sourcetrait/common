use crate::budget;
use crate::coder::{Decoder, Encoder, EncoderBuffer, Result, View};
use crate::derive::{Decode, Encode};
use alloc::string::String;
use alloc::vec::Vec;
use core::num::NonZeroUsize;
use core::ops::Deref;

pub struct DerefEncoder<T: Encode + ?Sized>(T::Encoder);

// Can't derive since it would bound T: Default.
impl<T: Encode + ?Sized> Default for DerefEncoder<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

unsafe impl<D: Deref<Target = T>, T: Encode + ?Sized> Encoder<D> for DerefEncoder<T> {
    #[inline(always)]
    unsafe fn encode(&mut self, t: &D) {
        self.0.encode(t);
    }
}

impl<T: Encode + ?Sized> EncoderBuffer for DerefEncoder<T> {
    fn collect_into(&mut self, out: &mut Vec<u8>) {
        self.0.collect_into(out);
    }
    fn reserve(&mut self, additional: NonZeroUsize) {
        self.0.reserve(additional);
    }
}

/// Decodes a `T` and then converts it with [`From`]. For example `T` -> `Box<T>` and `Vec<T>` -> `Box<[T]>`.
pub struct FromDecoder<'a, T: Decode<'a>>(T::Decoder);

// Can't derive since it would bound T: Default.
impl<'a, T: Decode<'a>> Default for FromDecoder<'a, T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<'a, T: Decode<'a>> View<'a> for FromDecoder<'a, T> {
    fn populate(&mut self, input: &mut &'a [u8], length: usize) -> Result<()> {
        // Each box's value is its own allocation, charged before it is read; a
        // recursive type nests through pointers as through collections.
        let _level = budget::enter(length, core::mem::size_of::<T>(), false)?;
        self.0.populate(input, length)
    }
}

/// A [`FromDecoder`] for `Rc` and `Arc`, whose allocation a budget charges as
/// their value after their two counts.
pub struct SharedDecoder<'a, T: Decode<'a>>(FromDecoder<'a, T>);

// Can't derive since it would bound T: Default.
impl<'a, T: Decode<'a>> Default for SharedDecoder<'a, T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<'a, T: Decode<'a>> View<'a> for SharedDecoder<'a, T> {
    fn populate(&mut self, input: &mut &'a [u8], length: usize) -> Result<()> {
        let each = budget::after_two_words(core::mem::size_of::<T>(), core::mem::align_of::<T>());
        let _level = budget::enter(length, each, false)?;
        self.0 .0.populate(input, length)
    }
}

unsafe impl<'a, F: From<T>, T: Decode<'a>> Decoder<'a, F> for SharedDecoder<'a, T> {
    const CACHE_SAFE: bool = <T::Decoder as Decoder<'a, T>>::CACHE_SAFE;

    #[inline(always)]
    unsafe fn decode(&mut self) -> F {
        F::from(self.0 .0.decode())
    }
}

/// The owned buffer an `Rc`'s or an `Arc`'s slice or `str` decodes into before
/// it is shared, and the alignment its elements take in the shared allocation.
pub trait SharedSlice {
    /// The alignment of the slice's elements.
    const ALIGN: usize;
}

impl<T> SharedSlice for Vec<T> {
    const ALIGN: usize = core::mem::align_of::<T>();
}

impl SharedSlice for String {
    const ALIGN: usize = 1;
}

/// A [`FromDecoder`] for an `Rc`'s or an `Arc`'s slice or `str`, decoded first
/// as `T`, its elements' `Vec` or a `String`: a budget charges each allocation's
/// counts padded to the elements' alignment, and `T`'s decoder the elements.
pub struct SharedSliceDecoder<'a, T: Decode<'a> + SharedSlice>(FromDecoder<'a, T>);

// Can't derive since it would bound T: Default.
impl<'a, T: Decode<'a> + SharedSlice> Default for SharedSliceDecoder<'a, T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<'a, T: Decode<'a> + SharedSlice> View<'a> for SharedSliceDecoder<'a, T> {
    fn populate(&mut self, input: &mut &'a [u8], length: usize) -> Result<()> {
        let _level = budget::enter(length, budget::slice_after_two_words(T::ALIGN), false)?;
        self.0 .0.populate(input, length)
    }
}

unsafe impl<'a, F: From<T>, T: Decode<'a> + SharedSlice> Decoder<'a, F> for SharedSliceDecoder<'a, T> {
    const CACHE_SAFE: bool = <T::Decoder as Decoder<'a, T>>::CACHE_SAFE;

    #[inline(always)]
    unsafe fn decode(&mut self) -> F {
        F::from(self.0 .0.decode())
    }
}

unsafe impl<'a, F: From<T>, T: Decode<'a>> Decoder<'a, F> for FromDecoder<'a, T> {
    const CACHE_SAFE: bool = <T::Decoder as Decoder<'a, T>>::CACHE_SAFE;

    #[inline(always)]
    unsafe fn decode(&mut self) -> F {
        F::from(self.0.decode())
    }
}

#[cfg(all(test, feature = "nightly"))]
mod tests {
    use crate::{decode, encode};
    use alloc::boxed::Box;
    use alloc::string::ToString;

    #[test]
    fn box_() {
        let v = Box::new(123u8);
        assert_eq!(decode::<Box<u8>>(&encode(&v)).unwrap(), v);
    }

    #[test]
    fn box_slice() {
        let v = vec![123u8].into_boxed_slice();
        assert_eq!(decode::<Box<[u8]>>(&encode(&v)).unwrap(), v);
    }

    #[test]
    fn box_str() {
        let v = "box".to_string().into_boxed_str();
        assert_eq!(decode::<Box<str>>(&encode(&v)).unwrap(), v);
    }
}
