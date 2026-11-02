//! rkyv for recursive types: the bounds `#[cereal::derived]` gives rkyv's
//! derive.
//!
//! rkyv's derive bounds each impl by every field type's own trait, so a
//! recursive type's bounds require themselves and the solver overflows
//! (E0275). rkyv's fix is `#[rkyv(omit_bounds)]` on the fields, the bounds
//! stated by hand, and the attribute applies it to every type it derives rkyv
//! for: recursion found by syntax misses a cycle through another type, and the
//! stated bounds cost nothing at run time. They are each type parameter's
//! trait, with the serializer's, the deserializer's, and the validator's, as
//! rkyv's own recursive example states them.
use proc_macro2::{TokenStream, TokenTree};
use quote::quote;

/// The item's `#[rkyv(..)]` bounds, emitted after the `#[derive]` that brings
/// in rkyv's helper attribute. The validator's assume rkyv's default
/// `bytecheck` feature.
pub(crate) fn make_item_attrs(generics: &syn::Generics) -> TokenStream {
    let params: Vec<&syn::Ident> = generics.type_params().map(|p| &p.ident).collect();

    let archive: Vec<TokenStream> = params
        .iter()
        .map(|p| quote!(#p: ::rkyv::Archive))
        .collect();
    let serialize = [
        quote!(__S: ::rkyv::ser::Writer + ::rkyv::ser::Allocator),
        quote!(__S::Error: ::rkyv::rancor::Source),
    ]
    .into_iter()
    .chain(params.iter().map(|p| quote!(#p: ::rkyv::Serialize<__S>)));
    let deserialize = [quote!(__D::Error: ::rkyv::rancor::Source)]
        .into_iter()
        .chain(params.iter().map(|p| {
            quote!(<#p as ::rkyv::Archive>::Archived: ::rkyv::Deserialize<#p, __D>)
        }));
    let bytecheck = [
        quote!(__C: ::rkyv::validation::ArchiveContext),
        quote!(__C::Error: ::rkyv::rancor::Source),
    ]
    .into_iter()
    .chain(params.iter().map(|p| {
        quote!(<#p as ::rkyv::Archive>::Archived: ::rkyv::bytecheck::CheckBytes<__C>)
    }));

    // An empty `archive_bounds()` is left out.
    let archive = (!archive.is_empty()).then(|| quote! { #[rkyv(archive_bounds( #(#archive),* ))] });

    quote! {
        #archive
        #[rkyv(serialize_bounds( #(#serialize),* ))]
        #[rkyv(deserialize_bounds( #(#deserialize),* ))]
        #[rkyv(bytecheck(bounds( #(#bytecheck),* )))]
    }
}

/// Puts `#[rkyv(omit_bounds)]` on each field that lacks it. The caller emits
/// the item it changed.
pub(crate) fn add_omit_bounds(item: &mut syn::DeriveInput) {
    let fields: Box<dyn Iterator<Item = &mut syn::Field>> = match &mut item.data {
        syn::Data::Struct(s) => Box::new(s.fields.iter_mut()),
        syn::Data::Enum(e) => Box::new(e.variants.iter_mut().flat_map(|v| v.fields.iter_mut())),
        syn::Data::Union(_) => return,
    };
    for field in fields {
        if !field.attrs.iter().any(omits_bounds) {
            field.attrs.push(syn::parse_quote!(#[rkyv(omit_bounds)]));
        }
    }
}

/// Whether an attribute is `#[rkyv(..)]` naming `omit_bounds` among its own
/// arguments.
fn omits_bounds(attr: &syn::Attribute) -> bool {
    let syn::Meta::List(list) = &attr.meta else {
        return false;
    };
    list.path.is_ident("rkyv")
        && list
            .tokens
            .clone()
            .into_iter()
            .any(|tt| matches!(tt, TokenTree::Ident(i) if i == "omit_bounds"))
}
