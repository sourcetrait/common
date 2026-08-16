use crate::*;

#[cereal::derived(Data, Copy, Eq, Hash)]
pub struct Nonce(pub(crate) u64);
impl Nonce {
    pub const fn new_unchecked(hash: u64) -> Self { Self(hash) }
    pub const fn get(&self) -> u64 { self.0 }
    pub const fn take(self) -> u64 { self.0 }
    
    pub fn generate(generator: &NonceGenerator) -> Self { generator.generate() }
    pub fn generate_with<T: Hash>(generator: &NonceGenerator, hashable: &T) -> Self {
        generator.generate_with(&hashable)
    }
    
    pub const fn parse_str(s: &str) -> Self { Self(cereal::hash64(s.as_bytes())) }
    
    pub fn into_pair(self) -> NoncePair { NoncePair(self.0, cereal::Base62u64::encode(self.0)) }
}

impl std::fmt::Display for Nonce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.into_pair().as_str())
    }
}

impl std::str::FromStr for Nonce {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match cereal::Base62u64::decode(s) {
            Some(b) => Ok(Self(b)),
            None => Err("Invalid base62"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoncePair(pub(crate) u64, pub(crate) cereal::Base62u64);
impl NoncePair {
    pub const fn new_unchecked(nonce: u64, base62: cereal::Base62u64) -> Self { Self(nonce, base62) }
    pub const fn from_tuple_unchecked(tuple: (u64, cereal::Base62u64)) -> Self { Self(tuple.0, tuple.1) }
    pub const fn take(self) -> (u64, cereal::Base62u64) { (self.0, self.1) }
    pub const fn nom(&self) -> u64 { self. 0 }
    pub const fn as_str(&self) -> &str { self.1.as_str() }
    pub const fn base62(&self) -> &cereal::Base62u64 { &self.1 }
    pub const fn to_nonce(&self) -> Nonce { Nonce(self.0) }
    pub const fn into_nonce(self) -> Nonce { Nonce(self.0) }
}

pub struct NonceGenerator {
    counter: AtomicUsize,
}

impl NonceGenerator {
    pub const fn new() -> Self { Self { counter: AtomicUsize::new(0) } }

    pub fn generate_with<T: Hash>(&self, hashable: &T) -> Nonce {
        let mut hasher = cereal::DefaultHasher::new();
        hashable.hash(&mut hasher);
        self.counter
            .fetch_add(1, atomic::Ordering::SeqCst)
            .hash(&mut hasher);
        let time_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        time_ns.hash(&mut hasher);
        Nonce(hasher.digest())
    }
    
    pub fn generate(&self) -> Nonce { self.generate_with(&()) }
}
