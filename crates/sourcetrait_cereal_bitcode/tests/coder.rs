//! The public coder API from outside the crate: a scalar coder, a recursive
//! type through lazy mirrors, a collection charging the budget before it
//! allocates, a level kept past its scope, and the decoder cache. Each input
//! a buffer decodes is dropped before its next decode, so Miri sees any read
//! of an earlier one.
use sourcetrait_cereal_bitcode as bitcode;

use bitcode::budget::{self, Level};
use bitcode::coder::{Decoder, Encoder, EncoderBuffer, Result, View, MAX_VECTORED_CHUNK};
use bitcode::{
    decode, decode_budgeted, encode, Budget, BudgetError, Buffer, Decode, Encode, Error, Exceeded,
};
use std::cell::Cell;
use std::num::NonZeroUsize;

/// A budget no input reaches.
const OPEN: Budget = Budget {
    elements: u64::MAX,
    heap: u64::MAX,
    depth: u32::MAX,
};

// ---------------------------------------------------------------------------
// A scalar coder: whole degrees Celsius, refused below absolute zero.
// ---------------------------------------------------------------------------

/// A temperature in whole degrees Celsius, never below absolute zero.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Celsius(i16);

const ABSOLUTE_ZERO: i16 = -273;

impl Encode for Celsius {
    type Encoder = CelsiusEncoder;
}

impl<'a> Decode<'a> for Celsius {
    type Decoder = CelsiusDecoder<'a>;
}

/// Each value's two little-endian bytes, in one column.
#[derive(Default)]
struct CelsiusEncoder {
    column: Vec<u8>,
}

impl EncoderBuffer for CelsiusEncoder {
    fn collect_into(&mut self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.column);
        self.column.clear();
    }

    fn reserve(&mut self, additional: NonZeroUsize) {
        self.column.reserve(additional.get() * 2);
    }
}

// SAFETY: the encoder copies each value's bytes and keeps nothing borrowed.
unsafe impl Encoder<Celsius> for CelsiusEncoder {
    unsafe fn encode(&mut self, t: &Celsius) {
        self.column.extend_from_slice(&t.0.to_le_bytes());
    }
}

/// The column `populate` validated, its values not yet decoded.
#[derive(Default)]
struct CelsiusDecoder<'a> {
    column: &'a [u8],
}

impl<'a> View<'a> for CelsiusDecoder<'a> {
    fn populate(&mut self, input: &mut &'a [u8], length: usize) -> Result<()> {
        let size = length
            .checked_mul(2)
            .ok_or_else(|| Error::new("length overflow"))?;
        if input.len() < size {
            return Err(Error::new("EOF"));
        }
        let (column, rest) = input.split_at(size);
        let below = column
            .as_chunks::<2>()
            .0
            .iter()
            .any(|b| i16::from_le_bytes(*b) < ABSOLUTE_ZERO);
        if below {
            return Err(Error::new("below absolute zero"));
        }
        self.column = column;
        *input = rest;
        Ok(())
    }
}

// SAFETY: `decode` returns its value and `decode_in_place` keeps its default.
// The decoder holds one slice of an input, which each `populate` replaces
// before anything reads it and `Drop` never reads, so it is `CACHE_SAFE`.
unsafe impl<'a> Decoder<'a, Celsius> for CelsiusDecoder<'a> {
    const CACHE_SAFE: bool = true;

    unsafe fn decode(&mut self) -> Celsius {
        let (value, rest) = self.column.split_at(2);
        self.column = rest;
        Celsius(i16::from_le_bytes([value[0], value[1]]))
    }
}

/// A derived struct holding the scalar.
#[derive(Encode, Decode, Clone, Debug, PartialEq)]
struct Reading {
    at: u32,
    temperature: Celsius,
    place: String,
}

fn readings(n: usize) -> Vec<Reading> {
    (0..n)
        .map(|i| Reading {
            at: i as u32 * 7,
            temperature: Celsius(i as i16 % 400 - 200),
            place: "p".repeat(i % 5),
        })
        .collect()
}

#[test]
fn a_scalar_coder_round_trips() {
    let one = Celsius(21);
    assert_eq!(decode::<Celsius>(&encode(&one)).unwrap(), one);
    assert_eq!(encode(&one), 21i16.to_le_bytes());

    let many: Vec<Celsius> = (ABSOLUTE_ZERO..ABSOLUTE_ZERO + 600).map(Celsius).collect();
    assert_eq!(decode::<Vec<Celsius>>(&encode(&many)).unwrap(), many);

    let some = vec![Some(Celsius(5)), None, Some(Celsius(ABSOLUTE_ZERO))];
    assert_eq!(decode::<Vec<Option<Celsius>>>(&encode(&some)).unwrap(), some);

    let rows = readings(50);
    assert_eq!(decode::<Vec<Reading>>(&encode(&rows)).unwrap(), rows);
}

#[test]
fn encode_vectored_fills_the_columns_encode_fills() {
    let rows = readings(150);
    let mut each = <Reading as Encode>::Encoder::default();
    each.reserve(NonZeroUsize::new(rows.len()).unwrap());
    for row in &rows {
        // SAFETY: the reserve above made room for every row.
        unsafe { each.encode(row) };
    }
    let mut vectored = <Reading as Encode>::Encoder::default();
    vectored.reserve(NonZeroUsize::new(rows.len()).unwrap());
    for chunk in rows.chunks(MAX_VECTORED_CHUNK) {
        // SAFETY: the reserve above made room for every row, and a chunk's
        // iterator yields from 1 to MAX_VECTORED_CHUNK rows, as its clones do.
        unsafe { vectored.encode_vectored(chunk.iter()) };
    }
    assert_eq!(each.collect(), vectored.collect());
}

#[test]
fn invalid_input_is_refused_in_populate() {
    let bytes = encode(&vec![Celsius(20), Celsius(-300)]);
    let error = decode::<Vec<Celsius>>(&bytes).unwrap_err();
    let reason = if cfg!(debug_assertions) {
        "below absolute zero"
    } else {
        "bitcode error"
    };
    assert_eq!(error.to_string(), reason);
    assert!(matches!(
        decode_budgeted::<Vec<Celsius>>(&bytes, OPEN),
        Err(BudgetError::Decode(_))
    ));

    let whole = encode(&vec![Celsius(1), Celsius(2)]);
    assert!(decode::<Vec<Celsius>>(&whole[..whole.len() - 1]).is_err());
}

#[test]
fn a_decoder_is_reused_after_a_failed_decode() {
    let mut buffer = Buffer::new();
    for round in 0..4i16 {
        let bad = encode(&vec![Celsius(1), Celsius(-500), Celsius(2)]);
        assert!(buffer.decode::<Vec<Celsius>>(&bad).is_err());
        drop(bad);

        let good = vec![Celsius(round), Celsius(ABSOLUTE_ZERO), Celsius(100)];
        let bytes = encode(&good);
        assert_eq!(buffer.decode::<Vec<Celsius>>(&bytes).unwrap(), good);
        drop(bytes);

        let cut = encode(&readings(9));
        assert!(buffer.decode::<Vec<Reading>>(&cut[..cut.len() / 2]).is_err());
        drop(cut);

        let rows = readings(9 + round as usize);
        let bytes = encode(&rows);
        assert_eq!(buffer.decode::<Vec<Reading>>(&bytes).unwrap(), rows);
        drop(bytes);
    }
}

// ---------------------------------------------------------------------------
// The decoder cache: a decoder kept across inputs only when it says so.
// ---------------------------------------------------------------------------

std::thread_local! {
    /// The `CountedDecoder`s made on this thread.
    static MADE: Cell<usize> = const { Cell::new(0) };
}

/// A byte whose decoder counts its making, cache-safe when `SAFE`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Counted<const SAFE: bool>(u8);

impl<const SAFE: bool> Encode for Counted<SAFE> {
    type Encoder = CountedEncoder;
}

impl<'a, const SAFE: bool> Decode<'a> for Counted<SAFE> {
    type Decoder = CountedDecoder<'a>;
}

#[derive(Default)]
struct CountedEncoder(<u8 as Encode>::Encoder);

impl EncoderBuffer for CountedEncoder {
    fn collect_into(&mut self, out: &mut Vec<u8>) {
        self.0.collect_into(out);
    }

    fn reserve(&mut self, additional: NonZeroUsize) {
        self.0.reserve(additional);
    }
}

// SAFETY: the encoder holds `u8`'s encoder alone, which keeps nothing borrowed.
unsafe impl<const SAFE: bool> Encoder<Counted<SAFE>> for CountedEncoder {
    unsafe fn encode(&mut self, t: &Counted<SAFE>) {
        // SAFETY: the caller's reserve, which `reserve` forwards, covers it.
        unsafe { self.0.encode(&t.0) };
    }
}

struct CountedDecoder<'a>(<u8 as Decode<'a>>::Decoder);

impl Default for CountedDecoder<'_> {
    fn default() -> Self {
        MADE.with(|made| made.set(made.get() + 1));
        Self(Default::default())
    }
}

impl<'a> View<'a> for CountedDecoder<'a> {
    fn populate(&mut self, input: &mut &'a [u8], length: usize) -> Result<()> {
        self.0.populate(input, length)
    }
}

// SAFETY: `decode` returns `u8`'s decoder's value, and the decoder holds that
// decoder alone, so it is `CACHE_SAFE` when that one is and `SAFE` says so.
unsafe impl<'a, const SAFE: bool> Decoder<'a, Counted<SAFE>> for CountedDecoder<'a> {
    const CACHE_SAFE: bool = SAFE && <<u8 as Decode<'a>>::Decoder as Decoder<'a, u8>>::CACHE_SAFE;

    unsafe fn decode(&mut self) -> Counted<SAFE> {
        // SAFETY: the caller's contract is `u8`'s decoder's, populated here.
        Counted(unsafe { self.0.decode() })
    }
}

/// A derived struct holding a decoder that isn't cache-safe.
#[derive(Encode, Decode, Debug, PartialEq)]
struct Holds {
    counted: Counted<false>,
    plain: u16,
}

/// The `CountedDecoder`s three decodes of `T` through one buffer made, each
/// from an input of its own.
fn made_by_three<T>(value: &T) -> usize
where
    T: Encode + for<'a> Decode<'a> + PartialEq + std::fmt::Debug,
{
    let mut buffer = Buffer::new();
    MADE.with(|made| made.set(0));
    for _ in 0..3 {
        let bytes = encode(value);
        assert_eq!(&buffer.decode::<T>(&bytes).unwrap(), value);
    }
    MADE.with(Cell::get)
}

#[test]
fn an_outside_decoder_stays_out_of_the_cache_unless_it_declares_itself_safe() {
    assert_eq!(made_by_three(&Counted::<true>(1)), 1);
    assert_eq!(made_by_three(&vec![Counted::<true>(1); 4]), 1);
    assert_eq!(made_by_three(&Counted::<false>(1)), 3);
    assert_eq!(made_by_three(&vec![Counted::<false>(1); 4]), 3);
    let holds = Holds {
        counted: Counted(9),
        plain: 300,
    };
    assert_eq!(made_by_three(&holds), 3);
}

// ---------------------------------------------------------------------------
// A recursive type through lazy mirrors.
// ---------------------------------------------------------------------------

/// A tree, recursive through its children.
#[derive(Clone, Debug, PartialEq)]
enum Tree {
    Leaf(u8),
    Node(Vec<Tree>),
}

impl Tree {
    /// The levels holding a node: a leaf's one, a node's one past its children's.
    fn depth(&self) -> usize {
        match self {
            Self::Leaf(_) => 1,
            Self::Node(children) => 1 + children.iter().map(Self::depth).max().unwrap_or(0),
        }
    }
}

/// `Tree`'s shape with its children owned, the mirror its encoder encodes.
#[derive(Encode)]
enum TreeEnc {
    Leaf(u8),
    Node(Vec<Tree>),
}

/// `Tree`'s shape, the mirror its decoder decodes.
#[derive(Decode)]
enum TreeDec {
    Leaf(u8),
    Node(Vec<Tree>),
}

/// A recursive type's mirror for encoding.
trait EncodeMirror {
    type Enc: Encode;
    fn to_enc(&self) -> Self::Enc;
}

/// A recursive type's mirror for decoding.
trait DecodeMirror<'de>: Sized {
    type Dec: Decode<'de>;
    fn from_dec(dec: Self::Dec) -> Self;
}

impl EncodeMirror for Tree {
    type Enc = TreeEnc;
    fn to_enc(&self) -> TreeEnc {
        match self {
            Self::Leaf(v) => TreeEnc::Leaf(*v),
            Self::Node(children) => TreeEnc::Node(children.clone()),
        }
    }
}

impl<'de> DecodeMirror<'de> for Tree {
    type Dec = TreeDec;
    fn from_dec(dec: TreeDec) -> Self {
        match dec {
            TreeDec::Leaf(v) => Self::Leaf(v),
            TreeDec::Node(children) => Self::Node(children),
        }
    }
}

impl Encode for Tree {
    type Encoder = LazyEncoder<Tree>;
}

impl<'de> Decode<'de> for Tree {
    type Decoder = LazyDecoder<'de, Tree>;
}

/// A recursive type's encoder: its mirror's encoder behind a box made at the
/// first reserve, emitting nothing until a value is encoded.
struct LazyEncoder<T: EncodeMirror> {
    inner: Option<Box<<T::Enc as Encode>::Encoder>>,
    /// The values encoded since the last collect: bytes go out iff there are any.
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
    fn inner(&mut self) -> &mut <T::Enc as Encode>::Encoder {
        self.inner.get_or_insert_with(Default::default)
    }
}

impl<T: EncodeMirror> EncoderBuffer for LazyEncoder<T> {
    fn collect_into(&mut self, out: &mut Vec<u8>) {
        if std::mem::take(&mut self.count) > 0 {
            self.inner().collect_into(out);
        }
    }

    fn reserve(&mut self, additional: NonZeroUsize) {
        self.inner().reserve(additional);
    }
}

// SAFETY: each mirror is made, encoded, and dropped within the call, by an
// encoder that keeps nothing borrowed.
unsafe impl<T: EncodeMirror> Encoder<T> for LazyEncoder<T> {
    unsafe fn encode(&mut self, t: &T) {
        self.count += 1;
        let mirror = t.to_enc();
        // SAFETY: the caller's reserve covers `t`, and `reserve` forwarded it.
        unsafe { self.inner().encode(&mirror) };
    }
}

std::thread_local! {
    /// The lazy decoders that made their mirror's decoder on this thread.
    static DESCENTS: Cell<usize> = const { Cell::new(0) };
}

/// A recursive type's decoder: its mirror's decoder behind a box made at the
/// first populate of any values.
struct LazyDecoder<'de, T: DecodeMirror<'de>> {
    inner: Option<Box<<T::Dec as Decode<'de>>::Decoder>>,
}

impl<'de, T: DecodeMirror<'de>> Default for LazyDecoder<'de, T> {
    fn default() -> Self {
        Self { inner: None }
    }
}

impl<'de, T: DecodeMirror<'de>> View<'de> for LazyDecoder<'de, T> {
    fn populate(&mut self, input: &mut &'de [u8], length: usize) -> Result<()> {
        if length == 0 {
            // No values emitted nothing, and descending would recurse without end.
            return Ok(());
        }
        let inner = self.inner.get_or_insert_with(|| {
            DESCENTS.with(|descents| descents.set(descents.get() + 1));
            Box::default()
        });
        inner.populate(input, length)
    }
}

// SAFETY: `decode_in_place` keeps its default, and `CACHE_SAFE` its default,
// false: the mirror's decoder takes its own from this one, so this one states
// it rather than taking the mirror's.
unsafe impl<'de, T: DecodeMirror<'de>> Decoder<'de, T> for LazyDecoder<'de, T> {
    unsafe fn decode(&mut self) -> T {
        let inner = self.inner.as_mut().expect("populated before it decodes");
        // SAFETY: the caller's populate of one value or more reached the
        // mirror's decoder, which decodes one of them.
        T::from_dec(unsafe { inner.decode() })
    }
}

fn trees() -> Vec<Tree> {
    let chain = (0..40).fold(Tree::Leaf(0), |tree, _| Tree::Node(vec![tree]));
    vec![
        Tree::Leaf(7),
        Tree::Node(vec![]),
        Tree::Node(vec![Tree::Leaf(1), Tree::Node(vec![Tree::Leaf(2)])]),
        Tree::Node(vec![Tree::Node(vec![]), Tree::Leaf(3), Tree::Node(vec![])]),
        chain,
    ]
}

#[test]
fn a_recursive_type_round_trips_through_lazy_mirrors() {
    let trees = trees();
    for tree in &trees {
        assert_eq!(&decode::<Tree>(&encode(tree)).unwrap(), tree);
    }
    assert_eq!(decode::<Vec<Tree>>(&encode(&trees)).unwrap(), trees);
    let some: Vec<Option<Tree>> = trees.iter().cloned().map(Some).chain([None]).collect();
    assert_eq!(decode::<Vec<Option<Tree>>>(&encode(&some)).unwrap(), some);
}

#[test]
fn populate_of_zero_descends_nowhere() {
    for tree in trees() {
        DESCENTS.with(|descents| descents.set(0));
        assert_eq!(decode::<Tree>(&encode(&tree)).unwrap(), tree);
        assert_eq!(DESCENTS.with(Cell::get), tree.depth(), "{tree:?}");
    }
}

#[test]
fn a_reserve_with_nothing_encoded_emits_nothing() {
    let mut encoder = LazyEncoder::<Tree>::default();
    encoder.reserve(NonZeroUsize::new(3).unwrap());
    assert!(encoder.collect().is_empty());

    // An option encoder reserves its present values' encoder for a whole
    // chunk, and encodes none of them here.
    let absent = vec![None::<Tree>; 4];
    let bytes = encode(&absent);
    assert_eq!(bytes, encode(&vec![None::<()>; 4]));
    assert_eq!(decode::<Vec<Option<Tree>>>(&bytes).unwrap(), absent);
}

// ---------------------------------------------------------------------------
// A collection charging the budget before it allocates.
// ---------------------------------------------------------------------------

std::thread_local! {
    /// Whether a `BagDecoder` allocated a bag's numbers on this thread.
    static ALLOCATED: Cell<bool> = const { Cell::new(false) };
}

/// A bag of numbers: its count in one column, its numbers in another.
#[derive(Clone, Debug, PartialEq)]
struct Bag(Vec<u32>);

impl Encode for Bag {
    type Encoder = BagEncoder;
}

impl<'a> Decode<'a> for Bag {
    type Decoder = BagDecoder<'a>;
}

#[derive(Default)]
struct BagEncoder {
    counts: Vec<u8>,
    numbers: Vec<u8>,
}

impl EncoderBuffer for BagEncoder {
    fn collect_into(&mut self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.counts);
        out.extend_from_slice(&self.numbers);
        self.counts.clear();
        self.numbers.clear();
    }

    fn reserve(&mut self, additional: NonZeroUsize) {
        self.counts.reserve(additional.get() * 4);
    }
}

// SAFETY: the encoder copies each bag's numbers and keeps nothing borrowed.
unsafe impl Encoder<Bag> for BagEncoder {
    unsafe fn encode(&mut self, t: &Bag) {
        let count = u32::try_from(t.0.len()).expect("a bag's count fits a u32");
        self.counts.extend_from_slice(&count.to_le_bytes());
        for number in &t.0 {
            self.numbers.extend_from_slice(&number.to_le_bytes());
        }
    }
}

#[derive(Default)]
struct BagDecoder<'a> {
    counts: &'a [u8],
    numbers: &'a [u8],
}

/// The next `n` bytes of `input`, or the error that it ended.
fn take<'a>(input: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
    if input.len() < n {
        return Err(Error::new("EOF"));
    }
    let (taken, rest) = input.split_at(n);
    *input = rest;
    Ok(taken)
}

fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().expect("four bytes"))
}

impl<'a> View<'a> for BagDecoder<'a> {
    fn populate(&mut self, input: &mut &'a [u8], length: usize) -> Result<()> {
        let size = length
            .checked_mul(4)
            .ok_or_else(|| Error::new("length overflow"))?;
        let counts = take(input, size)?;
        let total = counts
            .as_chunks::<4>()
            .0
            .iter()
            .try_fold(0usize, |total, count| {
                total.checked_add(u32::from_le_bytes(*count) as usize)
            })
            .ok_or_else(|| Error::new("length overflow"))?;
        // Charged before the numbers are read, or a bag allocated.
        let _level: Level = budget::enter(total, std::mem::size_of::<u32>(), true)?;
        let size = total
            .checked_mul(4)
            .ok_or_else(|| Error::new("length overflow"))?;
        self.numbers = take(input, size)?;
        self.counts = counts;
        Ok(())
    }
}

// SAFETY: `decode` returns its value and `decode_in_place` keeps its default;
// `CACHE_SAFE` keeps its default, false.
unsafe impl<'a> Decoder<'a, Bag> for BagDecoder<'a> {
    unsafe fn decode(&mut self) -> Bag {
        let (count, counts) = self.counts.split_at(4);
        self.counts = counts;
        let count = le_u32(count) as usize;
        ALLOCATED.with(|allocated| allocated.set(true));
        let mut numbers = Vec::with_capacity(count);
        let (bytes, rest) = self.numbers.split_at(count * 4);
        self.numbers = rest;
        numbers.extend(bytes.as_chunks::<4>().0.iter().map(|b| u32::from_le_bytes(*b)));
        Bag(numbers)
    }
}

#[test]
fn a_budget_is_charged_before_a_custom_allocation() {
    let bag = Bag((0..100).collect());
    let bytes = encode(&bag);
    let exact = Budget {
        elements: 100,
        heap: 400,
        depth: 1,
    };
    let refused = |budget| {
        ALLOCATED.with(|allocated| allocated.set(false));
        let bound = match decode_budgeted::<Bag>(&bytes, budget) {
            Err(BudgetError::Exceeded(bound)) => bound,
            other => panic!("not refused by a bound: {other:?}"),
        };
        assert!(!ALLOCATED.with(Cell::get), "allocated before the charge");
        bound
    };
    assert_eq!(refused(Budget { elements: 99, ..exact }), Exceeded::Elements);
    assert_eq!(refused(Budget { heap: 399, ..exact }), Exceeded::Heap);
    assert_eq!(refused(Budget { depth: 0, ..exact }), Exceeded::Depth);

    ALLOCATED.with(|allocated| allocated.set(false));
    assert_eq!(decode_budgeted::<Bag>(&bytes, exact).unwrap(), bag);
    assert!(ALLOCATED.with(Cell::get));

    // The charge precedes the numbers' read: an input cut short of them is
    // refused by the bound, not found short.
    let cut = &bytes[..4 + 8];
    assert!(matches!(
        decode_budgeted::<Bag>(cut, Budget { elements: 99, ..exact }),
        Err(BudgetError::Exceeded(Exceeded::Elements))
    ));
    assert!(matches!(
        decode_budgeted::<Bag>(cut, exact),
        Err(BudgetError::Decode(_))
    ));

    // Outside a budgeted decode the hooks charge nothing.
    assert_eq!(decode::<Bag>(&bytes).unwrap(), bag);
    let bags = vec![Bag(vec![]), bag.clone(), Bag(vec![7])];
    assert_eq!(decode::<Vec<Bag>>(&encode(&bags)).unwrap(), bags);
}

// ---------------------------------------------------------------------------
// A level kept past its scope gives nothing to the next.
// ---------------------------------------------------------------------------

std::thread_local! {
    /// A level a `HoardDecoder` kept from one decode for the next.
    static STASH: Cell<Option<Level>> = const { Cell::new(None) };
}

/// A value of no columns whose decoder keeps a level past its scope.
#[derive(Debug, PartialEq)]
struct Hoard;

impl Encode for Hoard {
    type Encoder = HoardEncoder;
}

impl<'a> Decode<'a> for Hoard {
    type Decoder = HoardDecoder;
}

#[derive(Default)]
struct HoardEncoder;

impl EncoderBuffer for HoardEncoder {
    fn collect_into(&mut self, _: &mut Vec<u8>) {}
    fn reserve(&mut self, _: NonZeroUsize) {}
}

// SAFETY: the encoder holds nothing.
unsafe impl Encoder<Hoard> for HoardEncoder {
    unsafe fn encode(&mut self, _: &Hoard) {}
}

#[derive(Default)]
struct HoardDecoder;

impl<'a> View<'a> for HoardDecoder {
    fn populate(&mut self, _: &mut &'a [u8], length: usize) -> Result<()> {
        // The level kept by an earlier decode drops here, in this one's scope.
        drop(STASH.with(Cell::take));
        let outer = budget::enter(length, 0, false)?;
        let inner = budget::enter(length, 0, false)?;
        STASH.with(|stash| stash.set(Some(inner)));
        drop(outer);
        Ok(())
    }
}

// SAFETY: `decode` returns its value and `decode_in_place` keeps its default.
unsafe impl<'a> Decoder<'a, Hoard> for HoardDecoder {
    unsafe fn decode(&mut self) -> Hoard {
        Hoard
    }
}

#[test]
fn a_level_gives_back_to_its_own_scope_alone() {
    let bytes = encode(&Hoard);
    assert!(bytes.is_empty());
    assert_eq!(decode_budgeted::<Hoard>(&bytes, OPEN).unwrap(), Hoard);
    // Two levels deep takes a depth of two: the level the first decode kept,
    // dropped in this one's scope, gives it no third.
    let shallow = Budget { depth: 1, ..OPEN };
    assert!(matches!(
        decode_budgeted::<Hoard>(&bytes, shallow),
        Err(BudgetError::Exceeded(Exceeded::Depth))
    ));
    assert_eq!(
        decode_budgeted::<Hoard>(&bytes, Budget { depth: 2, ..OPEN }).unwrap(),
        Hoard
    );
    drop(STASH.with(Cell::take));
}
