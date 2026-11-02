// The types both crates derive, with their random values from one seed.
// `upstream.rs` includes this file once a crate, `bitcode` naming that crate.
use rand::Rng;

#[derive(bitcode::Encode, bitcode::Decode, Clone, Debug, PartialEq)]
pub struct Row {
    pub id: u32,
    pub name: String,
    pub tags: Vec<String>,
    pub flag: Option<bool>,
    pub kind: Kind,
    pub score: i16,
    pub pairs: Vec<(u8, i64)>,
    pub bytes: Vec<u8>,
    pub fixed: [u16; 3],
    pub inner: Option<Box<Inner>>,
    pub fieldless: Fieldless,
    #[bitcode(skip)]
    pub skipped: u32,
}

#[derive(bitcode::Encode, bitcode::Decode, Clone, Debug, PartialEq)]
pub struct Inner {
    pub a: u64,
    pub b: Vec<i32>,
    pub c: (char, Option<u16>),
}

#[derive(bitcode::Encode, bitcode::Decode, Clone, Debug, PartialEq)]
pub enum Kind {
    Unit,
    Small(u8),
    Text { x: i8, y: String },
    List(Vec<u16>),
    Wide(u128, i128),
}

#[derive(bitcode::Encode, bitcode::Decode, Clone, Copy, Debug, PartialEq)]
pub enum Fieldless {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

/// A count skewed small, as most of a wire's lengths are.
pub fn count(rng: &mut impl Rng, most: usize) -> usize {
    match rng.gen_range(0..4) {
        0 => 0,
        1 => rng.gen_range(0..3),
        _ => rng.gen_range(0..=most),
    }
}

/// A random value of a random width, so each width of packing shows.
pub fn narrow(rng: &mut impl Rng) -> u64 {
    rng.gen::<u64>() >> rng.gen_range(0..64)
}

pub fn text(rng: &mut impl Rng) -> String {
    let n = count(rng, 12);
    (0..n)
        .map(|_| match rng.gen_range(0..6) {
            0 => char::from_u32(rng.gen_range(0x80..0x800)).unwrap_or('x'),
            1 => '\u{1F600}',
            _ => rng.gen_range(b'a'..=b'z') as char,
        })
        .collect()
}

impl Inner {
    pub fn random(rng: &mut impl Rng) -> Self {
        Self {
            a: narrow(rng),
            b: (0..count(rng, 6)).map(|_| narrow(rng) as i32).collect(),
            c: (
                char::from_u32(rng.gen_range(0..0xD800)).unwrap_or('c'),
                rng.gen::<bool>().then(|| narrow(rng) as u16),
            ),
        }
    }
}

impl Kind {
    pub fn random(rng: &mut impl Rng) -> Self {
        match rng.gen_range(0..5) {
            0 => Self::Unit,
            1 => Self::Small(rng.gen()),
            2 => Self::Text {
                x: rng.gen(),
                y: text(rng),
            },
            3 => Self::List((0..count(rng, 8)).map(|_| narrow(rng) as u16).collect()),
            _ => Self::Wide(rng.gen::<u128>() >> rng.gen_range(0..128), rng.gen()),
        }
    }
}

impl Fieldless {
    pub fn random(rng: &mut impl Rng) -> Self {
        [Self::A, Self::B, Self::C, Self::D, Self::E, Self::F, Self::G][rng.gen_range(0..7)]
    }
}

impl Row {
    pub fn random(rng: &mut impl Rng) -> Self {
        Self {
            id: narrow(rng) as u32,
            name: text(rng),
            tags: (0..count(rng, 5)).map(|_| text(rng)).collect(),
            flag: match rng.gen_range(0..3) {
                0 => None,
                _ => Some(rng.gen()),
            },
            kind: Kind::random(rng),
            score: narrow(rng) as i16,
            pairs: (0..count(rng, 6))
                .map(|_| (rng.gen(), narrow(rng) as i64))
                .collect(),
            bytes: (0..count(rng, 40)).map(|_| rng.gen()).collect(),
            fixed: [rng.gen(), narrow(rng) as u16, 0],
            inner: rng.gen::<bool>().then(|| Box::new(Inner::random(rng))),
            fieldless: Fieldless::random(rng),
            skipped: 0,
        }
    }
}
