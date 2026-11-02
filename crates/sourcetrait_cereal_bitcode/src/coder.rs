//! The coder traits, for a type whose encoder and decoder are written by hand.
//!
//! A type's [`Encode::Encoder`](crate::Encode::Encoder) fills columns: room is
//! reserved with [`EncoderBuffer::reserve`], each value goes in with
//! [`Encoder::encode`], and the columns come out as bytes with
//! [`EncoderBuffer::collect_into`]. Its [`Decode::Decoder`](crate::Decode::Decoder)
//! reads them back: [`View::populate`] reads and validates a number of values'
//! columns, and [`Decoder::decode`] takes each value out. A coder holds other
//! types' coders for its parts, as the derive's do.
//!
//! The contracts the encoding relies on are carried by the types. A method
//! whose caller keeps a contract is an `unsafe fn`, and a trait whose
//! implementer keeps one is an `unsafe trait`, each stating it under
//! `# Safety`. [`crate::budget`] holds the hooks a decoder charges a budgeted
//! decode through, and [`crate::Error::new`] the error a `populate` returns
//! for input that does not decode.
use crate::fast::{SliceImpl, Unaligned, VecImpl};
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use core::num::NonZeroUsize;

/// A coder's result, its error the crate's [`crate::Error`].
pub type Result<T> = core::result::Result<T, crate::Error>;

/// The columns an encoder fills: room reserved before values go in, and the
/// columns collected into bytes after.
pub trait EncoderBuffer {
    /// The columns collected into a new vector, as [`Self::collect_into`] does.
    fn collect(&mut self) -> Vec<u8> {
        let mut vec = vec![];
        self.collect_into(&mut vec);
        vec
    }

    /// Appends the columns to `out`, leaving the buffer empty.
    fn collect_into(&mut self, out: &mut Vec<u8>);

    /// Makes room for `additional` values, encoded before the next `reserve`
    /// or `collect_into`.
    fn reserve(&mut self, additional: NonZeroUsize);
}

/// The most values one [`Encoder::encode_vectored`] call takes.
pub const MAX_VECTORED_CHUNK: usize = 64;

/// Encodes values of `T` into the columns of its [`EncoderBuffer`].
///
/// # Safety
/// An implementer keeps nothing borrowed from a value once the call that
/// encodes it returns: the derive encodes a borrowed field through its
/// `'static` type's encoder, and [`crate::Buffer::encode`] keeps an encoder
/// across calls, its lifetimes erased.
pub unsafe trait Encoder<T: ?Sized>: EncoderBuffer + Default + Send + Sync {
    /// The column a type encoded by copying is copied into, if `T` is one.
    #[doc(hidden)]
    #[inline(always)]
    fn as_primitive(&mut self) -> Option<&mut VecImpl<T>>
    where
        T: Sized,
    {
        None
    }

    /// Encodes one value. Encoding never fails.
    ///
    /// # Safety
    /// The last [`EncoderBuffer::reserve`] made room for it: after
    /// `reserve(n)`, at most `n` values are encoded, by this method and by
    /// [`Self::encode_vectored`], before the next `reserve` or `collect_into`.
    unsafe fn encode(&mut self, t: &T);

    /// Encodes each value `i` yields, as [`Self::encode`] does. An
    /// implementation may iterate `i`'s clones once a part, so `i` is cheap to
    /// clone and to iterate.
    ///
    /// # Safety
    /// [`Self::encode`]'s, for each value; and `i` and each of its clones yield
    /// the same `n` values, `i.size_hint().1` being `Some(n)` with
    /// `1 <= n <= MAX_VECTORED_CHUNK`.
    unsafe fn encode_vectored<'a>(&mut self, i: impl Iterator<Item = &'a T> + Clone)
    where
        T: 'a,
    {
        for t in i {
            // SAFETY: the caller made room for each value `i` yields.
            unsafe { self.encode(t) };
        }
    }
}

/// Reads columns from the input and validates them before their values
/// decode.
pub trait View<'a> {
    /// Reads `length` values' columns from the front of `input`, advancing it
    /// past them, and validates them: once it returns `Ok`, [`Decoder::decode`]
    /// takes those `length` values out.
    fn populate(&mut self, input: &mut &'a [u8], length: usize) -> Result<()>;
}

/// Decodes the values a [`View::populate`] validated, one call a value.
///
/// One of [`Self::decode`] and [`Self::decode_in_place`] is implemented: each
/// defaults to the other, so an implementer of neither recurses until its
/// stack overflows.
///
/// # Safety
/// An implementer's [`Self::decode_in_place`] has initialized `out` when it
/// returns, and its [`Self::CACHE_SAFE`] is true only as that constant's
/// own description allows.
pub unsafe trait Decoder<'a, T>: View<'a> + Default + Send + Sync {
    /// Whether [`crate::Buffer::decode`] may keep the decoder across inputs.
    /// True promises that nothing the decoder holds from an earlier input,
    /// whose memory may be gone, is read again, by a later `populate` or
    /// `decode` or by `Drop`, and that `populate` starts over whatever a
    /// failed or partial decode left. False, the default, decodes each input
    /// with a decoder of its own.
    const CACHE_SAFE: bool = false;

    /// The column a type decoded by copying is copied from, if `T` is one.
    #[doc(hidden)]
    #[inline(always)]
    fn as_primitive(&mut self) -> Option<&mut SliceImpl<'_, Unaligned<T>>> {
        None
    }

    /// Decodes the next value. Decoding never fails, `populate` having
    /// validated the input.
    ///
    /// # Safety
    /// A [`View::populate`] of `n` values returned `Ok`, and fewer than `n`
    /// values were decoded since, by this method and by
    /// [`Self::decode_in_place`].
    #[inline(always)]
    unsafe fn decode(&mut self) -> T {
        let mut out = MaybeUninit::uninit();
        // SAFETY: the caller's contract is this method's, and
        // `decode_in_place` initializes `out`, as the implementer promises.
        unsafe {
            self.decode_in_place(&mut out);
            out.assume_init()
        }
    }

    /// Decodes the next value into `out`, sparing [`Self::decode`]'s copies.
    /// A panic leaks the value.
    ///
    /// # Safety
    /// [`Self::decode`]'s.
    #[inline(always)]
    unsafe fn decode_in_place(&mut self, out: &mut MaybeUninit<T>) {
        // SAFETY: the caller's contract is this method's.
        out.write(unsafe { self.decode() });
    }
}

/// A `&mut MaybeUninit` of a field of the struct a `&mut MaybeUninit` points
/// to, for the derive's `decode_in_place`. It takes an unsafe context:
/// `$field_ty` is the field's own type, and the reference the field's only
/// one while it lives.
#[doc(hidden)]
#[macro_export]
macro_rules! __private_uninit_field {
    ($uninit:ident.$field:tt:$field_ty:ty) => {
        &mut *(::core::ptr::addr_of_mut!((*$uninit.as_mut_ptr()).$field)
            as *mut ::core::mem::MaybeUninit<$field_ty>)
    };
}
#[allow(unused_imports)]
pub(crate) use __private_uninit_field as uninit_field;
