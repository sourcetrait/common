use crate::coder::{Decoder, Encoder, EncoderBuffer, Result, View};
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::num::NonZeroUsize;

#[derive(Default)]
pub struct EmptyCoder;

unsafe impl<T> Encoder<PhantomData<T>> for EmptyCoder {
    unsafe fn encode(&mut self, _: &PhantomData<T>) {}
}

impl EncoderBuffer for EmptyCoder {
    fn collect_into(&mut self, _: &mut Vec<u8>) {}
    fn reserve(&mut self, _: NonZeroUsize) {}
}

impl<'a> View<'a> for EmptyCoder {
    fn populate(&mut self, _: &mut &'a [u8], _: usize) -> Result<()> {
        Ok(())
    }
}

unsafe impl<'a, T> Decoder<'a, PhantomData<T>> for EmptyCoder {
    const CACHE_SAFE: bool = true;

    unsafe fn decode(&mut self) -> PhantomData<T> {
        PhantomData
    }
}

#[cfg(all(test, feature = "nightly"))]
mod tests {
    use alloc::vec::Vec;
    use core::marker::PhantomData;

    fn bench_data() -> Vec<PhantomData<()>> {
        vec![PhantomData; 100]
    }
    crate::bench_encode_decode!(phantom_data_vec: Vec<_>);
}
