//! bitcode for recursive types: what `#[cereal::derived]` emits for one, run
//! by cereal's `bitcoded` module.
//!
//! bitcode's derive composes a coder from its fields' coders inline, so a
//! recursive type can't take it: its coder would be infinite in size. The
//! attribute emits two mirrors of the type's shape instead, which bitcode
//! derives: an encode mirror whose fields are `Ref`s to the value's, and an
//! owned decode mirror. The type's `Encode` and `Decode` go through
//! `LazyEncoder` and `LazyDecoder`, which box the mirror's coder and convert
//! each value through `EncodeMirror::to_enc` and `DecodeMirror::from_dec`.
//!
//! Each field's `#[bitcode(..)]` attributes go to both mirrors, and each type
//! parameter is bounded as bitcode's derive bounds it, by `Encode` and by
//! `Decode<'__de>`.
use crate::shared::{extend_where, subst_self};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};

/// The runtime's module, by the `cereal` path the attribute is named through.
fn runtime() -> TokenStream {
    quote!(cereal::bitcoded)
}

/// How a struct or a variant lays out its fields.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Named,
    Unnamed,
    Unit,
}

/// A struct's or a variant's fields: a binding, a type, and the bitcode
/// attributes of each.
struct Fields<'a> {
    style: Style,
    binds: Vec<syn::Ident>,
    types: Vec<TokenStream>,
    attrs: Vec<Vec<&'a syn::Attribute>>,
}

impl<'a> Fields<'a> {
    /// The fields, `Self` in their types naming `self_ty`, an unnamed field
    /// bound as `__f<index>`.
    fn new(fields: &'a syn::Fields, self_ty: &TokenStream) -> Self {
        let style = match fields {
            syn::Fields::Named(_) => Style::Named,
            syn::Fields::Unnamed(_) => Style::Unnamed,
            syn::Fields::Unit => Style::Unit,
        };
        let binds = fields
            .iter()
            .enumerate()
            .map(|(i, f)| f.ident.clone().unwrap_or_else(|| format_ident!("__f{}", i)))
            .collect();
        let types = fields
            .iter()
            .map(|f| subst_self(f.ty.to_token_stream(), self_ty))
            .collect();
        let attrs = fields
            .iter()
            .map(|f| f.attrs.iter().filter(|a| a.path().is_ident("bitcode")).collect())
            .collect();
        Self {
            style,
            binds,
            types,
            attrs,
        }
    }

    /// The fields declared, each type as `ty` makes it from the field's.
    fn declare(&self, ty: impl Fn(&TokenStream) -> TokenStream) -> TokenStream {
        let Self {
            binds, types, attrs, ..
        } = self;
        let types = types.iter().map(ty);
        match self.style {
            Style::Named => quote!({ #( #(#attrs)* #binds: #types ),* }),
            Style::Unnamed => quote!(( #( #(#attrs)* #types ),* )),
            Style::Unit => quote!(),
        }
    }

    /// The pattern binding each field.
    fn pattern(&self) -> TokenStream {
        let binds = &self.binds;
        match self.style {
            Style::Named => quote!({ #(#binds),* }),
            Style::Unnamed => quote!(( #(#binds),* )),
            Style::Unit => quote!(),
        }
    }

    /// The encode mirror's value at `path`, each field a `Ref` to its binding,
    /// in an unsafe block where there are any: each `Ref` is used while the
    /// value is, as `to_enc`'s caller promises.
    fn refs(&self, path: TokenStream, rt: &TokenStream) -> TokenStream {
        let binds = &self.binds;
        match self.style {
            Style::Named if binds.is_empty() => quote!(#path {}),
            Style::Named => quote!(unsafe { #path { #( #binds: #rt::Ref::new(#binds) ),* } }),
            Style::Unnamed if binds.is_empty() => quote!(#path()),
            Style::Unnamed => quote!(unsafe { #path( #( #rt::Ref::new(#binds) ),* ) }),
            Style::Unit => path,
        }
    }
}

/// `Encode` and `Decode` for a recursive type, with the mirrors they go
/// through.
pub(crate) fn make_recursive_impls(item: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let rt = runtime();
    let target = &item.ident;
    let generics = &item.generics;
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    let self_ty = quote!(#target #type_generics);
    let enc = format_ident!("__CerealBitcodeEnc{}", target);
    let dec = format_ident!("__CerealBitcodeDec{}", target);

    // The mirrors take the type's own generics: a `Ref` has no lifetime to
    // thread through. Decoding adds bitcode's `'__de`, outliving the type's.
    let mut de_generics = generics.clone();
    de_generics.params.insert(0, syn::parse_quote!('__de));
    let (de_impl_generics, _, _) = de_generics.split_for_impl();
    let params: Vec<&syn::Ident> = generics.type_params().map(|p| &p.ident).collect();
    let enc_where = extend_where(
        where_clause,
        params
            .iter()
            .map(|p| -> syn::WherePredicate { syn::parse_quote!(#p: ::bitcode::Encode) }),
    );
    let dec_where = extend_where(
        where_clause,
        params
            .iter()
            .map(|p| -> syn::WherePredicate { syn::parse_quote!(#p: ::bitcode::Decode<'__de>) })
            .chain(generics.lifetimes().map(|l| -> syn::WherePredicate {
                let l = &l.lifetime;
                syn::parse_quote!('__de: #l)
            })),
    );

    let (enc_item, dec_item, to_enc, from_dec) = match &item.data {
        syn::Data::Struct(s) => {
            let fields = Fields::new(&s.fields, &self_ty);
            let enc_fields = fields.declare(|ty| quote!(#rt::Ref<#ty>));
            let dec_fields = fields.declare(TokenStream::clone);
            // A braced struct takes its where-clause before its fields, a tuple
            // or unit struct after them and then a `;`.
            let (enc_item, dec_item) = if fields.style == Style::Named {
                (
                    quote!(pub struct #enc #impl_generics #where_clause #enc_fields),
                    quote!(pub struct #dec #impl_generics #where_clause #dec_fields),
                )
            } else {
                (
                    quote!(pub struct #enc #impl_generics #enc_fields #where_clause;),
                    quote!(pub struct #dec #impl_generics #dec_fields #where_clause;),
                )
            };
            let pattern = fields.pattern();
            let refs = fields.refs(quote!(#enc), &rt);
            // Matching `self`, a reference, binds each field by reference.
            let to_enc = quote!({
                let #target #pattern = self;
                #refs
            });
            let from_dec = quote!({
                let #dec #pattern = dec;
                #target #pattern
            });
            (enc_item, dec_item, to_enc, from_dec)
        }
        syn::Data::Enum(e) => {
            let mut enc_variants = Vec::new();
            let mut dec_variants = Vec::new();
            let mut to_arms = Vec::new();
            let mut from_arms = Vec::new();
            for v in &e.variants {
                let name = &v.ident;
                let fields = Fields::new(&v.fields, &self_ty);
                let enc_fields = fields.declare(|ty| quote!(#rt::Ref<#ty>));
                let dec_fields = fields.declare(TokenStream::clone);
                let pattern = fields.pattern();
                let refs = fields.refs(quote!(#enc::#name), &rt);
                enc_variants.push(quote!(#name #enc_fields));
                dec_variants.push(quote!(#name #dec_fields));
                to_arms.push(quote!(#target::#name #pattern => #refs));
                from_arms.push(quote!(#dec::#name #pattern => #target::#name #pattern));
            }
            let enc_item =
                quote!(pub enum #enc #impl_generics #where_clause { #(#enc_variants),* });
            let dec_item =
                quote!(pub enum #dec #impl_generics #where_clause { #(#dec_variants),* });
            let (to_enc, from_dec) = if e.variants.is_empty() {
                (quote!(match *self {}), quote!(match dec {}))
            } else {
                (
                    quote!(match self { #(#to_arms),* }),
                    quote!(match dec { #(#from_arms),* }),
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
            #enc_item

            // By hand, so no `Copy` bound falls on the type's parameters: every
            // field is a `Ref`, itself `Copy`.
            impl #impl_generics ::core::clone::Clone for #enc #type_generics #where_clause {
                #[inline(always)]
                fn clone(&self) -> Self {
                    *self
                }
            }
            impl #impl_generics ::core::marker::Copy for #enc #type_generics #where_clause {}

            #[derive(::bitcode::Decode)]
            #dec_item

            impl #impl_generics #rt::EncodeMirror for #self_ty #enc_where {
                type Enc = #enc #type_generics;

                #[inline(always)]
                unsafe fn to_enc(&self) -> Self::Enc {
                    #to_enc
                }
            }

            impl #de_impl_generics #rt::DecodeMirror<'__de> for #self_ty #dec_where {
                type Dec = #dec #type_generics;

                #[inline(always)]
                fn from_dec(dec: Self::Dec) -> Self {
                    #from_dec
                }
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
