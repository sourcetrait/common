# sourcetrait_cereal_bitcode

SourceTrait's fork of [bitcode](https://github.com/SoftbearStudios/bitcode),
taken from its published 0.6.9 with upstream's master since merged. It adds a
budgeted decode, sound short copies, and a public coder API, and its wire
format is upstream's.
`CODEC` names the codec, `cereal-bitcode-` and the crate's version.

A dependent takes it under bitcode's own name, so the derive's paths hold:

```toml
bitcode = { package = "sourcetrait_cereal_bitcode", version = "=0.0.0-1" }
```

Its unit tests and benches take a nightly toolchain, as upstream's do, behind
the `nightly` feature: `cargo +nightly test -p sourcetrait_cereal_bitcode
--features nightly`. Its integration tests and doctests run on stable.

A binary encoder/decoder with the following goals:
- 🔥 Blazingly fast
- 🐁 Tiny serialized size
- 💎 Highly compressible by Deflate/LZ4/Zstd

In contrast, these are non-goals:
- Stable format across major versions
- Self describing format
- Compatibility with languages other than Rust

See [rust_serialization_benchmark](https://github.com/djkoloski/rust_serialization_benchmark) for benchmarks.

## Example
```rust
# use sourcetrait_cereal_bitcode as bitcode;
use bitcode::{Encode, Decode};

#[derive(Encode, Decode, PartialEq, Debug)]
struct Foo<'a> {
    x: u32,
    y: &'a str,
}

let original = Foo {
    x: 10,
    y: "abc",
};

let encoded: Vec<u8> = bitcode::encode(&original); // No error
let decoded: Foo<'_> = bitcode::decode(&encoded).unwrap();
assert_eq!(original, decoded);
```

## Adding Support for Libraries

See upstream's instructions [here](https://github.com/SoftbearStudios/bitcode/wiki/Adding-library-support)!

## Implementation Details
- Heavily inspired by <https://github.com/That3Percent/tree-buf>
- All instances of each field are grouped together making compression easier
- Uses smaller integers where possible all the way down to 1 bit
- Validation is performed up front on typed vectors before deserialization
- Code is designed to be auto-vectorized by LLVM

## Budgeted decoding
This fork adds `decode_budgeted`, a decode held to a `Budget` of collection
elements, heap bytes, and nesting depth. Each collection's decoder charges the
budget once its lengths are read, before its elements are read or allocated,
so an input declaring a value far larger, or far deeper, than its own size is
refused before it takes the memory or the stack. The heap is what the decoded
value keeps, each collection and pointer charged by its own layout: a vector's
elements, a string's bytes, a linked list's nodes, a box's value, an `Rc`'s or
an `Arc`'s value with its counts, a slice's counts padded to its elements'
alignment; the decoder's own column buffers scale with
what it charges and are freed when the decode ends. Byte vectors and strings
count as heap alone. No budget bounds a B-tree's or a hash table's allocations
yet, so a budgeted decode refuses one. A charge is made once a column, never
once an element, and a decode outside `decode_budgeted` charges nothing. The
wire format is upstream's.

## Sound short copies
Upstream encodes a vector of short primitive slices by reading a whole
fixed-width block past each slice's end when the read stays in one page, which
is undefined behaviour in Rust's memory model (upstream's issue 50). This fork
copies each short slice as two moves of one fixed width, the second ending
where the slice ends, and a slice of one to three bytes as its first, middle,
and last, so every byte read lies inside it. On an x86-64 desktop, upstream's
two benchmarks of short slices, `bench_bool_vecs_encode` and
`bench_str_vec_encode`, run 5% and 6% slower than with upstream's over-read,
where a plain `memcpy` of each slice runs 97% and 67% slower.

## Coder API
This fork makes bitcode's coder traits a public extension API, the `coder`
module, for a type whose encoder and decoder are written by hand. A type names
its coders as `Encode::Encoder` and `Decode::Decoder`. An encoder fills
columns: `EncoderBuffer::reserve` makes room, `Encoder::encode` and
`Encoder::encode_vectored` put values in, and `EncoderBuffer::collect_into`
writes the columns out. A decoder reads them back: `View::populate` reads and
validates a number of values' columns, refusing input that does not decode
with `Error::new`, and `Decoder::decode` takes each value out.

Each contract the encoding relies on is carried by the types. `encode`,
`encode_vectored`, `decode`, and `decode_in_place` are `unsafe fn`s whose
callers keep their contracts: a value goes only into room a reserve made, a
vectored chunk holds an exact count of 1 to `MAX_VECTORED_CHUNK` values, and a
value is decoded only after a populate validated it. `Encoder` and `Decoder`
are `unsafe trait`s whose implementers keep theirs: an encoder keeps nothing
borrowed from a value once its encode returns, and a decoder's
`decode_in_place` initializes its output. `Buffer::decode` keeps a decoder
across inputs only when it declares `Decoder::CACHE_SAFE`, promising that
nothing it holds from an earlier input is read again; a recursive type's lazy
decoder states that constant itself, since its mirror's decoder takes its own
from the lazy decoder. A
decoder charges a budgeted decode through `budget::enter`, `budget::heap`, and
`budget::unbudgeted` before it reads or allocates what it charges. Each only
spends the scope `decode_budgeted` opens, and the `budget::Level` that `enter`
takes gives back to that scope alone.

The derive's coders and the crate's own implement the same traits, and the wire
format stays upstream's: a differential against crates.io's 0.6.9 holds it.

## `serde`
A `serde` integration is gated behind the `"serde"` feature flag. Click [here](https://github.com/SoftbearStudios/bitcode/wiki/Serde) to learn more.

## `#![no_std]`
All `std`-only functionality is gated behind the (default) `"std"` feature.

`alloc` is required.

## License
SourceTrait's changes are licensed under the GNU Affero General Public License,
version 3 or later, as SourceTrait Common is. bitcode's own code is used under
its MIT license, in [LICENSE-MIT](LICENSE-MIT); upstream offers it under
either the Apache License 2.0 or the MIT license.
