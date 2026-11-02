//! The fork's wire against crates.io's bitcode 0.6.9: random values of each
//! family, built-in and derived, encode to the same bytes in both and decode
//! across both. It runs natively alone: upstream's short copies read past
//! their slices (its issue 50), which Miri reports.
#![cfg(not(miri))]

use bitcode_upstream as upstream;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sourcetrait_cereal_bitcode as fork;
use std::collections::{BTreeMap, BTreeSet, HashMap, LinkedList, VecDeque};
use std::fmt::Debug;

mod fork_types {
    use sourcetrait_cereal_bitcode as bitcode;
    include!("upstream/types.rs");
}

mod upstream_types {
    use bitcode_upstream as bitcode;
    include!("upstream/types.rs");
}

use fork_types::{count, narrow, text};

/// The values of each family, one seed a value.
const VALUES: u64 = 400;

/// `value`'s bytes equal in both crates, through the fork's buffer too, and
/// each crate's bytes decoding in the other.
fn same<T>(value: &T)
where
    T: fork::Encode + upstream::Encode + Debug + PartialEq,
    T: for<'a> fork::Decode<'a> + for<'a> upstream::Decode<'a>,
{
    let ours = fork::encode(value);
    let theirs = upstream::encode(value);
    assert_eq!(ours, theirs, "{value:?}");
    assert_eq!(fork::Buffer::new().encode(value), ours.as_slice());
    assert_eq!(&fork::decode::<T>(&theirs).unwrap(), value);
    assert_eq!(&upstream::decode::<T>(&ours).unwrap(), value);
}

/// [`same`] over a family's values.
fn family<T>(make: impl Fn(&mut ChaCha8Rng) -> T)
where
    T: fork::Encode + upstream::Encode + Debug + PartialEq,
    T: for<'a> fork::Decode<'a> + for<'a> upstream::Decode<'a>,
{
    for seed in 0..VALUES {
        same(&make(&mut ChaCha8Rng::seed_from_u64(seed)));
    }
}

/// [`same`] for floats, held by their bits: NaN payloads included.
fn same_bits<T, B>(values: &Vec<T>, bits: impl Fn(&T) -> B)
where
    Vec<T>: fork::Encode + upstream::Encode,
    Vec<T>: for<'a> fork::Decode<'a> + for<'a> upstream::Decode<'a>,
    B: PartialEq + Debug,
{
    let ours = fork::encode(values);
    let theirs = upstream::encode(values);
    assert_eq!(ours, theirs);
    let want: Vec<B> = values.iter().map(&bits).collect();
    let back: Vec<T> = fork::decode(&theirs).unwrap();
    assert_eq!(back.iter().map(&bits).collect::<Vec<_>>(), want);
    let back: Vec<T> = upstream::decode(&ours).unwrap();
    assert_eq!(back.iter().map(&bits).collect::<Vec<_>>(), want);
}

#[test]
fn integers_and_scalars_encode_as_upstream_s() {
    family(|rng| {
        (
            rng.gen::<u8>(),
            rng.gen::<i8>(),
            narrow(rng) as u16,
            narrow(rng) as i16,
            narrow(rng) as u32,
            narrow(rng) as i32,
            narrow(rng),
            narrow(rng) as i64,
        )
    });
    family(|rng| {
        (
            rng.gen::<u128>() >> rng.gen_range(0..128),
            rng.gen::<i128>(),
            narrow(rng) as usize,
            narrow(rng) as isize,
            rng.gen::<bool>(),
            char::from_u32(rng.gen_range(0..0xD800)).unwrap(),
        )
    });
    family(|rng| {
        (0..count(rng, 300))
            .map(|_| narrow(rng) as u32)
            .collect::<Vec<_>>()
    });
    family(|rng| {
        (0..count(rng, 300))
            .map(|_| narrow(rng) as i64)
            .collect::<Vec<_>>()
    });
    family(|rng| {
        (0..count(rng, 50))
            .map(|_| char::from_u32(rng.gen_range(0..0x11000)).unwrap_or('?'))
            .collect::<Vec<_>>()
    });
}

#[test]
fn floats_encode_as_upstream_s_bit_for_bit() {
    for seed in 0..VALUES {
        let rng = &mut ChaCha8Rng::seed_from_u64(seed);
        let singles: Vec<f32> = (0..count(rng, 100))
            .map(|_| f32::from_bits(rng.gen()))
            .collect();
        let doubles: Vec<f64> = (0..count(rng, 100))
            .map(|_| f64::from_bits(rng.gen()))
            .collect();
        same_bits(&singles, |f| f.to_bits());
        same_bits(&doubles, |f| f.to_bits());
    }
}

#[test]
fn collections_encode_as_upstream_s() {
    family(|rng| {
        (0..count(rng, 30))
            .map(|_| (0..count(rng, 20)).map(|_| rng.gen::<u8>()).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    });
    family(text);
    family(|rng| (0..count(rng, 30)).map(|_| text(rng)).collect::<Vec<_>>());
    family(|rng| {
        (0..count(rng, 60))
            .map(|_| rng.gen::<bool>().then(|| narrow(rng) as u16))
            .collect::<Vec<_>>()
    });
    family(|rng| {
        (0..count(rng, 60))
            .map(|_| {
                if rng.gen() {
                    Ok(narrow(rng) as u32)
                } else {
                    Err(rng.gen::<u8>())
                }
            })
            .collect::<Vec<Result<u32, u8>>>()
    });
    family(|rng| {
        (0..count(rng, 60))
            .map(|_| [rng.gen::<u8>(), rng.gen(), rng.gen()])
            .collect::<Vec<_>>()
    });
    family(|rng| {
        (0..count(rng, 60))
            .map(|_| (rng.gen::<bool>(), rng.gen::<u8>()))
            .collect::<Vec<_>>()
    });
    family(|rng| {
        (0..count(rng, 40))
            .map(|_| (narrow(rng) as u16, text(rng)))
            .collect::<BTreeMap<_, _>>()
    });
    family(|rng| {
        (0..count(rng, 40))
            .map(|_| (narrow(rng) as u32, narrow(rng)))
            .collect::<HashMap<_, _>>()
    });
    family(|rng| {
        (0..count(rng, 40))
            .map(|_| rng.gen::<u8>())
            .collect::<BTreeSet<_>>()
    });
    family(|rng| {
        (0..count(rng, 80))
            .map(|_| rng.gen::<u8>())
            .collect::<VecDeque<_>>()
    });
    family(|rng| {
        (0..count(rng, 20))
            .map(|_| narrow(rng) as i16)
            .collect::<LinkedList<_>>()
    });
    family(|rng| {
        (0..count(rng, 20))
            .map(|_| text(rng).into_boxed_str())
            .collect::<Vec<Box<str>>>()
    });
    family(|rng| {
        rng.gen::<bool>().then(|| {
            (0..count(rng, 10))
                .map(|_| rng.gen::<bool>().then(|| text(rng)))
                .collect::<Vec<_>>()
        })
    });
}

/// Each derived type's random vectors, one seed a vector, made in each crate's
/// copy of the type from the same seed: the same bytes, and each crate's bytes
/// decoding in the other.
macro_rules! derived_same {
    ($($ty:ident),+) => {
        $(
            for seed in 0..VALUES {
                let ours: Vec<fork_types::$ty> = {
                    let rng = &mut ChaCha8Rng::seed_from_u64(seed);
                    (0..fork_types::count(rng, 20))
                        .map(|_| fork_types::$ty::random(rng))
                        .collect()
                };
                let theirs: Vec<upstream_types::$ty> = {
                    let rng = &mut ChaCha8Rng::seed_from_u64(seed);
                    (0..upstream_types::count(rng, 20))
                        .map(|_| upstream_types::$ty::random(rng))
                        .collect()
                };
                let our_bytes = fork::encode(&ours);
                let their_bytes = upstream::encode(&theirs);
                assert_eq!(our_bytes, their_bytes, "{ours:?}");
                assert_eq!(
                    fork::decode::<Vec<fork_types::$ty>>(&their_bytes).unwrap(),
                    ours
                );
                assert_eq!(
                    upstream::decode::<Vec<upstream_types::$ty>>(&our_bytes).unwrap(),
                    theirs
                );
                if let (Some(one), Some(other)) = (ours.first(), theirs.first()) {
                    assert_eq!(fork::encode(one), upstream::encode(other));
                }
            }
        )+
    };
}

#[test]
fn derived_types_encode_as_upstream_s() {
    derived_same!(Row, Inner, Kind, Fieldless);
}
