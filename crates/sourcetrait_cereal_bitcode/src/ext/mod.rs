#[cfg(feature = "arrayvec")]
mod arrayvec;
#[cfg(feature = "glam")]
#[rustfmt::skip] // Makes impl_struct! calls way longer.
mod glam;
#[cfg(feature = "rust_decimal")]
mod rust_decimal;
#[cfg(feature = "time")]
mod time;
#[cfg(feature = "uuid")]
mod uuid;

#[allow(unused)]
macro_rules! impl_struct {
    ($t:ident, $new:ident, $($f:ident, $ft:ty),+) => {
        const _: () = {
            #[derive(Default)]
            pub struct StructEncoder {
                $(
                    $f: <$ft as crate::Encode>::Encoder,
                )+
            }
            unsafe impl crate::coder::Encoder<$t> for StructEncoder {
                #[inline(always)]
                unsafe fn encode(&mut self, t: &$t) {
                    $(
                        self.$f.encode(&t.$f);
                    )+
                }
            }
            impl crate::coder::EncoderBuffer for StructEncoder {
                fn collect_into(&mut self, out: &mut Vec<u8>) {
                    $(
                        self.$f.collect_into(out);
                    )+
                }

                fn reserve(&mut self, additional: core::num::NonZeroUsize) {
                    $(
                        self.$f.reserve(additional);
                    )+
                }
            }
            impl crate::Encode for $t {
                type Encoder = StructEncoder;
            }

            #[derive(Default)]
            pub struct StructDecoder<'a> {
                $(
                    $f: <$ft as crate::Decode<'a>>::Decoder,
                )+
            }
            impl<'a> crate::coder::View<'a> for StructDecoder<'a> {
                fn populate(&mut self, input: &mut &'a [u8], length: usize) -> crate::coder::Result<()> {
                    $(
                        self.$f.populate(input, length)?;
                    )+
                    Ok(())
                }
            }
            unsafe impl<'a> crate::coder::Decoder<'a, $t> for StructDecoder<'a> {
                const CACHE_SAFE: bool = true $(
                    && <<$ft as crate::Decode<'a>>::Decoder as crate::coder::Decoder<'a, $ft>>::CACHE_SAFE
                )+;

                // TODO use decode_in_place instead.
                #[inline(always)]
                unsafe fn decode(&mut self) -> $t {
                    $t::$new($(self.$f.decode()),+)
                }
            }
            impl<'a> crate::Decode<'a> for $t {
                type Decoder = StructDecoder<'a>;
            }
        };
    }
}
#[allow(unused)]
pub(crate) use impl_struct;
