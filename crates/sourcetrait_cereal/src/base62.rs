
const ALPHABET: &[u8; 62] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Base62 encoding of a u64
#[sourcetrait_cereal_macro::derived(Data, Copy, Eq)]
pub struct Base62u64(pub [u8; 11]);
impl Base62u64 {
    pub const fn encode(mut n: u64) -> Self {
        let mut buf = [0u8; 11];
        if n == 0 {
            buf[0] = b'0';
            return Self(buf);
        }
    
        let mut pos = 11;
        while n > 0 {
            pos -= 1;
            buf[pos] = ALPHABET[(n % 62) as usize];
            n /= 62;
        }
    
        let len = 11 - pos;
        let mut i = 0;
        while i < len {
            buf[i] = buf[pos + i];
            i += 1;
        }
        
        while i < 11 {
            buf[i] = 0;
            i += 1;
        }
        
        Base62u64(buf)
    }

    pub const fn decode(s: &str) -> Option<u64> {
        let bytes = s.as_bytes();
        if bytes.is_empty() || bytes.len() > 11 {
            return None;
        }
    
        let mut n: u64 = 0;
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            let digit = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'z' => b - b'a' + 10,
                b'A'..=b'Z' => b - b'A' + 36,
                _ => return None,
            } as u64;
    
            // Guard against overflow (inputs longer/larger than u64::MAX).
            n = match n.checked_mul(62) {
                Some(v) => v,
                None => return None,
            };
            n = match n.checked_add(digit) {
                Some(v) => v,
                None => return None,
            };
            i += 1;
        }
        Some(n)
    }
    
    pub const fn as_str(&self) -> &str {
        let mut end = 0;
        while end < 11 && self.0[end] != 0 {
            end += 1;
        }
        
        unsafe {
            std::str::from_utf8_unchecked(self.0.split_at(end).0)
        }
    }

    pub const fn check(s: &str) -> bool {
        let bytes = s.as_bytes();
        if bytes.is_empty() || bytes.len() > 11 {
            return false;
        }
        
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' => {}
                _ => return false,
            }
            i += 1;
        }
        
        true
    }
}

impl std::fmt::Display for Base62u64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn is_base62(s: &str) -> bool {
    !s.is_empty()
    && s.as_bytes()
        .iter()
        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z'))
}
