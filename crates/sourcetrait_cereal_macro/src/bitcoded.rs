//! bitcode support for recursive types.
//!
//! bitcode 0.6's derive generates a coder struct that structurally mirrors the type: one column
//! coder per field, composed inline. A self-referential type therefore has an infinitely-sized
//! coder ("reached the recursion limit finding the struct tail"). No attribute changes that; the
//! coder for the recursive type must itself sit behind a pointer.
//!
//! So for recursive types we don't derive on the user's type. Instead:
//!
//! 1. Generate two private mirror types with the same shape and let bitcode's *own* derive
//!    generate their coders:
//!      - an encode mirror `__CerealBitcodeEnc<T>` whose fields are `Ref<F>`, a lifetime-free
//!        borrow (`NonNull<F>`) with an `Encode` impl (bitcode has none for `&T`, only `&str`);
//!      - an owned decode mirror `__CerealBitcodeDec<T>` with the original field types.
//! 2. Implement `Encode`/`Decode` for the user type with the runtime's `LazyEncoder` /
//!    `LazyDecoder`, which lazily box the mirror coder (breaking the size cycle) and convert
//!    through `EncodeMirror::to_enc` / `DecodeMirror::from_dec`.
//!
//! Only bitcode's own `#[bitcode(..)]` attributes (`skip`, `bound_type`) travel to the mirrors.
//! Bounds follow bitcode's policy: `T: Encode` / `T: Decode<'__de>` per type parameter.

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};

use crate::shared::{extend_where, subst_self};

/// Path of the runtime support module in the `sourcetrait_cereal` crate.
fn rt() -> TokenStream {
    quote!(cereal::bitcoded)
}

/// Field-shape fragments for one struct, or one enum variant.
struct FieldsGen {
    /// Encode-mirror declaration: `{ a: Ref<A> }`, `( Ref<A> )`, or nothing.
    enc_decl: TokenStream,
    /// Decode-mirror declaration: `{ a: A }`, `( A )`, or nothing.
    dec_decl: TokenStream,
    /// Pattern binding every field: `{ a }`, `( __f0 )`, or nothing.
    pat: TokenStream,
    /// Encode-mirror constructor from those bindings: `{ a: Ref::new(a) }`, `( Ref::new(__f0) )`, or nothing.
    enc_build: TokenStream,
    unit: bool,
    named: bool,
}

fn bitcode_attrs(f: &syn::Field) -> Vec<&syn::Attribute> {
    f.attrs.iter().filter(|a| a.path().is_ident("bitcode")).collect()
}

fn gen_fields(fields: &syn::Fields, self_ty: &TokenStream) -> FieldsGen {
    let rt = rt();
    match fields {
        syn::Fields::Named(n) => {
            let names: Vec<&syn::Ident> = n
                .named
                .iter()
                .map(|f| f.ident.as_ref().expect("named field"))
                .collect();
            let attrs: Vec<Vec<&syn::Attribute>> = n.named.iter().map(bitcode_attrs).collect();
            let tys: Vec<TokenStream> = n
                .named
                .iter()
                .map(|f| subst_self(f.ty.to_token_stream(), self_ty))
                .collect();
            FieldsGen {
                enc_decl: quote!({ #( #(#attrs)* #names: #rt::Ref<#tys> ),* }),
                dec_decl: quote!({ #( #(#attrs)* #names: #tys ),* }),
                pat: quote!({ #(#names),* }),
                enc_build: quote!({ #( #names: #rt::Ref::new(#names) ),* }),
                unit: false,
                named: true,
            }
        }
        syn::Fields::Unnamed(u) => {
            let binds: Vec<syn::Ident> = (0..u.unnamed.len())
                .map(|i| format_ident!("__f{}", i))
                .collect();
            let attrs: Vec<Vec<&syn::Attribute>> = u.unnamed.iter().map(bitcode_attrs).collect();
            let tys: Vec<TokenStream> = u
                .unnamed
                .iter()
                .map(|f| subst_self(f.ty.to_token_stream(), self_ty))
                .collect();
            FieldsGen {
                enc_decl: quote!(( #( #(#attrs)* #rt::Ref<#tys> ),* )),
                dec_decl: quote!(( #( #(#attrs)* #tys ),* )),
                pat: quote!(( #(#binds),* )),
                enc_build: quote!(( #( #rt::Ref::new(#binds) ),* )),
                unit: false,
                named: false,
            }
        }
        syn::Fields::Unit => FieldsGen {
            enc_decl: quote!(),
            dec_decl: quote!(),
            pat: quote!(),
            enc_build: quote!(),
            unit: true,
            named: false,
        },
    }
}

/// `impl Encode`/`impl Decode` for a recursive type, plus the mirrors they route through.
pub(crate) fn make_recursive_impls(item: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let rt = rt();
    let target = &item.ident;
    let generics = &item.generics;
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    let self_ty = quote!(#target #type_generics);

    let enc_ident = format_ident!("__CerealBitcodeEnc{}", target);
    let dec_ident = format_ident!("__CerealBitcodeDec{}", target);

    // The encode mirror has the SAME generics as the user type: `Ref<F>` is lifetime-free
    // (it holds a `NonNull<F>`), so there is no borrow lifetime to thread through.

    // Decode impls: user generics plus bitcode's `'__de`.
    let mut de_generics = generics.clone();
    de_generics.params.insert(0, syn::parse_quote!('__de));
    let (de_impl_generics, _, _) = de_generics.split_for_impl();

    // Bounds, same policy as bitcode's derive: per type parameter, not per field type.
    let type_params: Vec<&syn::Ident> = generics.type_params().map(|p| &p.ident).collect();
    let lifetimes: Vec<&syn::Lifetime> = generics.lifetimes().map(|l| &l.lifetime).collect();
    let enc_where = extend_where(
        where_clause,
        type_params
            .iter()
            .map(|p| -> syn::WherePredicate { syn::parse_quote!(#p: ::bitcode::Encode) }),
    );
    let dec_where = extend_where(
        where_clause,
        type_params
            .iter()
            .map(|p| -> syn::WherePredicate { syn::parse_quote!(#p: ::bitcode::Decode<'__de>) })
            .chain(
                lifetimes
                    .iter()
                    .map(|l| -> syn::WherePredicate { syn::parse_quote!('__de: #l) }),
            ),
    );

    let (enc_item, dec_item, to_enc_body, from_dec_body) = match &item.data {
        syn::Data::Struct(s) => {
            let FieldsGen { enc_decl, dec_decl, pat, enc_build, unit, named } =
                gen_fields(&s.fields, &self_ty);
            // Named structs put the where-clause before the braces; tuple/unit after, then `;`.
            let (enc_item, dec_item) = if named {
                (
                    quote!(pub struct #enc_ident #impl_generics #where_clause #enc_decl),
                    quote!(pub struct #dec_ident #impl_generics #where_clause #dec_decl),
                )
            } else {
                (
                    quote!(pub struct #enc_ident #impl_generics #enc_decl #where_clause;),
                    quote!(pub struct #dec_ident #impl_generics #dec_decl #where_clause;),
                )
            };
            let to_enc = if unit {
                quote!(#enc_ident)
            } else {
                // `self: &Self` — match ergonomics bind every field as `&F`.
                quote!({ let #target #pat = self; unsafe { #enc_ident #enc_build } })
            };
            let from_dec = if unit {
                quote!({ let _ = m; #target })
            } else {
                // by-value move out of the owned mirror
                quote!({ let #dec_ident #pat = m; #target #pat })
            };
            (enc_item, dec_item, to_enc, from_dec)
        }
        syn::Data::Enum(e) => {
            let mut enc_variants = Vec::new();
            let mut dec_variants = Vec::new();
            let mut to_arms = Vec::new();
            let mut from_arms = Vec::new();
            for v in &e.variants {
                let vi = &v.ident;
                let FieldsGen { enc_decl, dec_decl, pat, enc_build, .. } =
                    gen_fields(&v.fields, &self_ty);
                enc_variants.push(quote!(#vi #enc_decl));
                dec_variants.push(quote!(#vi #dec_decl));
                to_arms.push(quote!(#target::#vi #pat => unsafe { #enc_ident::#vi #enc_build }));
                from_arms.push(quote!(#dec_ident::#vi #pat => #target::#vi #pat));
            }
            let enc_item = quote!(pub enum #enc_ident #impl_generics #where_clause { #(#enc_variants),* });
            let dec_item = quote!(pub enum #dec_ident #impl_generics #where_clause { #(#dec_variants),* });
            let (to_enc, from_dec) = if e.variants.is_empty() {
                (quote!(match *self {}), quote!(match m {}))
            } else {
                (
                    quote!(match self { #(#to_arms),* }),
                    quote!(match m { #(#from_arms),* }),
                )
            };
            (enc_item, dec_item, to_enc, from_dec)
        }
        syn::Data::Union(u) => {
            return Err(syn::Error::new_spanned(
                u.union_token,
                "cereal: recursive unions are not supported",
            ));
        }
    };

    Ok(quote! {
        #[allow(non_camel_case_types, dead_code, clippy::all)]
        const _: () = {
            #[derive(::bitcode::Encode)]
            #[allow(non_camel_case_types, dead_code)]
            #enc_item

            // Manual, so no `T: Copy` bound is inferred: every field is a `Ref`, which is Copy.
            impl #impl_generics ::core::clone::Clone for #enc_ident #type_generics #where_clause {
                #[inline(always)]
                fn clone(&self) -> Self { *self }
            }
            impl #impl_generics ::core::marker::Copy for #enc_ident #type_generics #where_clause {}

            #[derive(::bitcode::Decode)]
            #[allow(non_camel_case_types, dead_code)]
            #dec_item

            impl #impl_generics #rt::EncodeMirror for #self_ty #enc_where {
                type Enc = #enc_ident #type_generics;
                #[inline(always)]
                fn to_enc(&self) -> Self::Enc { #to_enc_body }
            }

            impl #de_impl_generics #rt::DecodeMirror<'__de> for #self_ty #dec_where {
                type Dec = #dec_ident #type_generics;
                #[inline(always)]
                fn from_dec(m: Self::Dec) -> Self { #from_dec_body }
            }

            impl #impl_generics ::bitcode::Encode for #self_ty #enc_where {
                type Encoder = #rt::LazyEncoder<#self_ty>;
            }

            impl #de_impl_generics ::bitcode::Decode<'__de> for #self_ty #dec_where {
                type Decoder = #rt::LazyDecoder<'__de, #self_ty>;
            }
        };
    })
}