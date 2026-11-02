//! bitcode for recursive types.
//!
//! bitcode's derive composes a type's coder from its fields' coders inline, so
//! a recursive type would take a coder of infinite size. `#[cereal::derived]`
//! gives such a type two mirrors of its shape instead and derives bitcode on
//! them: an encode mirror whose fields borrow the type's through [`Ref`], and
//! an owned decode mirror. The type's own coders, [`LazyEncoder`] and
//! [`LazyDecoder`], hold their mirror's coder behind a box made at its first
//! use, which ends the cycle, and convert each value through [`EncodeMirror`]
//! and [`DecodeMirror`].
//!
//! The wire is the mirrors', each level of the recursion taking its own columns
//! where bitcode's derive would lay them out. Two rules keep both sides level:
//! an encoder emits bytes only when it encoded a value since its last collect,
//! as a reserve may be followed by no value at all, and a decoder populated with
//! no values reads nothing and descends no further, which also ends the
//! recursion.
use bitcode::coder::{Decoder, Encoder, EncoderBuffer, View, MAX_VECTORED_CHUNK};
use bitcode::{Decode, Encode, Error};
use core::mem::MaybeUninit;
use core::num::NonZeroUsize;
use core::ptr::NonNull;

/// A recursive type's encode mirror: its shape, each field a [`Ref`] to the
/// value's own. `#[cereal::derived]` implements it.
pub trait EncodeMirror {
    /// The encode mirror.
    type Enc: Encode + Copy;

    /// The mirror of `self`, borrowing its fields.
    ///
    /// # Safety
    /// The mirror and its copies are used only while `self` lives, unmoved and
    /// unmutated.
    unsafe fn to_enc(&self) -> Self::Enc;
}

/// A recursive type's decode mirror: its shape, owning its fields.
/// `#[cereal::derived]` implements it.
pub trait DecodeMirror<'de>: Sized {
    /// The decode mirror.
    type Dec: Decode<'de>;

    /// The value the mirror's fields make.
    fn from_dec(dec: Self::Dec) -> Self;
}

/// A borrow of an `F` without a lifetime, so an encode mirror takes none:
/// bitcode's derive names a field's encoder at the field type's `'static`,
/// which a borrow of a generic `F` would hold to `F: 'static`.
pub struct Ref<F: ?Sized>(NonNull<F>);

impl<F: ?Sized> Ref<F> {
    /// A borrow of `f`.
    ///
    /// # Safety
    /// The `Ref` and its copies are used only while `f` lives, unmoved and
    /// unmutated.
    #[inline(always)]
    pub unsafe fn new(f: &F) -> Self {
        Self(NonNull::from(f))
    }
}

impl<F: ?Sized> Clone for Ref<F> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<F: ?Sized> Copy for Ref<F> {}

impl<F: ?Sized + Encode> Encode for Ref<F> {
    type Encoder = RefEncoder<F>;
}

/// A [`Ref`]'s encoder: the borrowed type's own.
pub struct RefEncoder<F: ?Sized + Encode>(F::Encoder);

impl<F: ?Sized + Encode> Default for RefEncoder<F> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<F: ?Sized + Encode> EncoderBuffer for RefEncoder<F> {
    fn collect_into(&mut self, out: &mut Vec<u8>) {
        self.0.collect_into(out);
    }

    fn reserve(&mut self, additional: NonZeroUsize) {
        self.0.reserve(additional);
    }
}

// SAFETY: the encoder holds `F`'s encoder alone, which keeps nothing borrowed.
unsafe impl<F: ?Sized + Encode> Encoder<Ref<F>> for RefEncoder<F> {
    #[inline(always)]
    unsafe fn encode(&mut self, r: &Ref<F>) {
        // SAFETY: a `Ref` is used while its value lives, as `Ref::new`'s caller
        // promises, and the caller's reserve, which `reserve` forwards, covers it.
        unsafe { self.0.encode(r.0.as_ref()) };
    }

    #[inline(always)]
    unsafe fn encode_vectored<'v>(&mut self, i: impl Iterator<Item = &'v Ref<F>> + Clone)
    where
        Ref<F>: 'v,
    {
        // The borrowed values take a lifetime local to this call, which any `F`
        // outlives, so this needs no `F: 'v`. SAFETY: as in `encode`, the mapped
        // iterator yielding the caller's count.
        unsafe { self.0.encode_vectored(i.map(|r| r.0.as_ref())) };
    }
}

/// A recursive type's encoder: its mirror's encoder, behind a box made at the
/// first reserve, emitting bytes only for values encoded.
pub struct LazyEncoder<T: EncodeMirror> {
    inner: Option<Box<<T::Enc as Encode>::Encoder>>,
    /// The values encoded since the last collect.
    count: usize,
}

impl<T: EncodeMirror> Default for LazyEncoder<T> {
    fn default() -> Self {
        Self {
            inner: None,
            count: 0,
        }
    }
}

impl<T: EncodeMirror> LazyEncoder<T> {
    /// The mirror's encoder, made at its first use.
    #[inline(always)]
    fn inner(&mut self) -> &mut <T::Enc as Encode>::Encoder {
        self.inner.get_or_insert_with(Default::default)
    }
}

impl<T: EncodeMirror> EncoderBuffer for LazyEncoder<T> {
    fn collect_into(&mut self, out: &mut Vec<u8>) {
        // A reserve that no value followed emits nothing.
        if core::mem::take(&mut self.count) > 0 {
            self.inner().collect_into(out);
        }
    }

    fn reserve(&mut self, additional: NonZeroUsize) {
        self.inner().reserve(additional);
    }
}

// SAFETY: the encoder keeps nothing borrowed past an encode: each mirror,
// borrowing the value through `Ref`s, is encoded within the call that makes it,
// by the mirror's encoder, which keeps nothing borrowed.
unsafe impl<T: EncodeMirror> Encoder<T> for LazyEncoder<T> {
    #[inline(always)]
    unsafe fn encode(&mut self, t: &T) {
        self.count += 1;
        // SAFETY: the mirror is encoded here, while `t` is borrowed.
        let mirror = unsafe { t.to_enc() };
        // SAFETY: the caller's reserve covers `t`, and `reserve` forwarded it.
        unsafe { self.inner().encode(&mirror) };
    }

    unsafe fn encode_vectored<'a>(&mut self, i: impl Iterator<Item = &'a T> + Clone)
    where
        T: 'a,
    {
        // The mirrors are made into an array, so their encoder iterates a
        // slice, exact and cheap to clone, and a struct's fields stay vectored.
        // The caller yields at most `MAX_VECTORED_CHUNK` values, or the index
        // panics.
        let mut mirrors = [const { MaybeUninit::<T::Enc>::uninit() }; MAX_VECTORED_CHUNK];
        let mut n = 0;
        for t in i {
            // SAFETY: the mirrors are encoded below, while `i`'s values are
            // borrowed.
            mirrors[n].write(unsafe { t.to_enc() });
            n += 1;
        }
        if n == 0 {
            return;
        }
        self.count += n;
        // SAFETY: `mirrors[..n]` are initialized, and a mirror, `Copy`, needs no
        // drop.
        let mirrors = unsafe { core::slice::from_raw_parts(mirrors.as_ptr().cast::<T::Enc>(), n) };
        // SAFETY: the caller's reserve covers the `n` values, which `reserve`
        // forwarded, and a slice's iterator yields exactly them each time.
        unsafe { self.inner().encode_vectored(mirrors.iter()) };
    }
}

/// A recursive type's decoder: its mirror's decoder, behind a box made at the
/// first populate of any values.
pub struct LazyDecoder<'de, T: DecodeMirror<'de>> {
    inner: Option<Box<<T::Dec as Decode<'de>>::Decoder>>,
}

impl<'de, T: DecodeMirror<'de>> Default for LazyDecoder<'de, T> {
    fn default() -> Self {
        Self { inner: None }
    }
}

impl<'de, T: DecodeMirror<'de>> View<'de> for LazyDecoder<'de, T> {
    fn populate(&mut self, input: &mut &'de [u8], length: usize) -> Result<(), Error> {
        // No values were emitted for none, and descending would recurse
        // without end.
        if length == 0 {
            return Ok(());
        }
        self.inner
            .get_or_insert_with(Default::default)
            .populate(input, length)
    }
}

// SAFETY: `decode_in_place` keeps its default, writing `decode`'s value, and
// `CACHE_SAFE` its default, false: the mirror's decoder takes its own from this
// one, which states it rather than taking the mirror's.
unsafe impl<'de, T: DecodeMirror<'de>> Decoder<'de, T> for LazyDecoder<'de, T> {
    #[inline(always)]
    unsafe fn decode(&mut self) -> T {
        let inner = self
            .inner
            .as_mut()
            .expect("a populate of one value or more precedes a decode");
        // SAFETY: the caller's populate of one value or more reached the
        // mirror's decoder, which decodes one of them.
        T::from_dec(unsafe { inner.decode() })
    }
}
