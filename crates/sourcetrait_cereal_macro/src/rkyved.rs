//! rkyv support for recursive types.
//!
//! rkyv's derive performs a "perfect derive": every generated impl gets a `FieldType: Trait`
//! where-clause, so `MyData: Archive` requires `Vec<MyData>: Archive` requires `MyData: Archive`
//! and the solver overflows (E0275). rkyv's documented fix is `#[rkyv(omit_bounds)]` on the
//! offending fields plus explicit `{archive,serialize,deserialize}_bounds(..)`.
//!
//! We apply that unconditionally rather than trying to detect recursion: syntactic detection
//! misses mutual recursion, aliases and projections, and omitting the field bounds costs nothing
//! at runtime. The replacement bounds are the serde-style parameter bounds (`T: Archive`, ...)
//! plus the serializer/deserializer/validator predicates rkyv's own recursive example uses.

use proc_macro2::TokenStream;
use quote::quote;

/// Item-level `#[rkyv(..)]` attributes. Must be emitted *after* the
/// `#[derive(::rkyv::Archive, ...)]` that introduces the `rkyv` helper attribute.
///
/// The `bytecheck(bounds(..))` line assumes rkyv's default `bytecheck` feature; drop it if rkyv
/// is built without validation.
pub(crate) fn make_item_attrs(generics: &syn::Generics) -> TokenStream {
    let params: Vec<&syn::Ident> = generics.type_params().map(|p| &p.ident).collect();

    let archive: Vec<TokenStream> = params
        .iter()
        .map(|p| quote!(#p: ::rkyv::Archive))
        .collect();

    let mut serialize: Vec<TokenStream> = vec![
        quote!(__S: ::rkyv::ser::Writer + ::rkyv::ser::Allocator),
        quote!(__S::Error: ::rkyv::rancor::Source),
    ];
    serialize.extend(params.iter().map(|p| quote!(#p: ::rkyv::Serialize<__S>)));

    let mut deserialize: Vec<TokenStream> = vec![quote!(__D::Error: ::rkyv::rancor::Source)];
    deserialize.extend(params.iter().map(|p| {
        quote!(<#p as ::rkyv::Archive>::Archived: ::rkyv::Deserialize<#p, __D>)
    }));

    let mut bytecheck: Vec<TokenStream> = vec![
        quote!(__C: ::rkyv::validation::ArchiveContext),
        quote!(__C::Error: ::rkyv::rancor::Source),
    ];
    bytecheck.extend(params.iter().map(|p| {
        quote!(<#p as ::rkyv::Archive>::Archived: ::rkyv::bytecheck::CheckBytes<__C>)
    }));

    // Only emit archive_bounds when there is something to say (avoids `archive_bounds()`).
    let archive_attr = (!archive.is_empty()).then(|| {
        quote! { #[rkyv(archive_bounds( #(#archive),* ))] }
    });

    quote! {
        #archive_attr
        #[rkyv(serialize_bounds( #(#serialize),* ))]
        #[rkyv(deserialize_bounds( #(#deserialize),* ))]
        #[rkyv(bytecheck(bounds( #(#bytecheck),* )))]
    }
}

/// Push `#[rkyv(omit_bounds)]` onto every field that doesn't already carry one.
/// The caller must emit the mutated item, not the original tokens.
pub(crate) fn add_omit_bounds(item: &mut syn::DeriveInput) {
    fn each(fields: &mut syn::Fields) {
        for f in fields.iter_mut() {
            let already = f.attrs.iter().any(|a| {
                a.path().is_ident("rkyv")
                    && match &a.meta {
                        syn::Meta::List(l) => l.tokens.to_string().contains("omit_bounds"),
                        _ => false,
                    }
            });
            if !already {
                f.attrs.push(syn::parse_quote!(#[rkyv(omit_bounds)]));
            }
        }
    }
    match &mut item.data {
        syn::Data::Struct(s) => each(&mut s.fields),
        syn::Data::Enum(e) => {
            for v in e.variants.iter_mut() {
                each(&mut v.fields);
            }
        }
        syn::Data::Union(_) => {}
    }
}