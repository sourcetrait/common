use proc_macro;
use proc_macro2;
use quote::quote;
use syn::{self, parse::Parser};

mod bitcoded;
mod rkyved;
mod shared;

#[proc_macro_attribute]
pub fn derived(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let attr = proc_macro2::TokenStream::from(attr);
    let item = proc_macro2::TokenStream::from(item);
    match parse_derived(attr, item) {
        Ok(token_stream) => proc_macro::TokenStream::from(token_stream),
        Err(err) => proc_macro::TokenStream::from(err.to_compile_error())
    }
}
fn make_derives(cereal_attr: &CerealAttr, recursive: bool) -> syn::Result<proc_macro2::TokenStream> {
    // bitcode's derive generates a coder struct that structurally mirrors the type, so a
    // self-referential type yields an infinitely-sized coder ("reached the recursion limit
    // finding the struct tail"). For recursive types the Encode/Decode impls are emitted by
    // `bitcoded::make_recursive_impls` instead of being derived.
    let derive_bitcode = match (cereal_attr.bitcode.derive, recursive) {
        (true, false) => Some(quote! { ::bitcode::Encode, ::bitcode::Decode, }),
        _ => None,
    };
    let derive_debug = match cereal_attr.debug.derive {
        true => Some(quote! { ::std::fmt::Debug, }),
        false => None,
    };
    let derive_clone = match cereal_attr.clone.derive {
        true => Some(quote! { ::core::clone::Clone, }),
        false => None,
    };
    let derive_copy = match cereal_attr.copy.derive {
        true => Some(quote! { ::core::marker::Copy, }),
        false => None,
    };
    let derive_partial_eq = match cereal_attr.partial_eq.derive {
        true => Some(quote! { ::core::cmp::PartialEq, }),
        false => None,
    };
    let derive_eq = match cereal_attr.eq.derive {
        true => Some(quote! { ::core::cmp::Eq, }),
        false => None,
    };
    let derive_hash = match cereal_attr.hash.derive {
        true => Some(quote! { ::std::hash::Hash, }),
        false => None,
    };
    let derive_rkyv = match cereal_attr.rkyv.derive {
        true => Some(quote! { ::rkyv::Archive, ::rkyv::Serialize, ::rkyv::Deserialize, }),
        false => None,
    };
    let derive_serde = match cereal_attr.serde.derive {
        true => Some(quote! { ::serde::Serialize, ::serde::Deserialize, }),
        false => None,
    };
    
    let output = quote! {
        #[derive(
            #derive_debug
            #derive_clone
            #derive_copy
            #derive_partial_eq
            #derive_eq
            #derive_hash
            #derive_serde
            #derive_bitcode
            #derive_rkyv
        )]
    };
    
    Ok(output)
}
fn parse_derived(attr: proc_macro2::TokenStream, item: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    let mut item = syn::parse2::<syn::DeriveInput>(item)?;
    let cereal_attr = parse_attr(attr)?;
    // `Recursive` / `not(Recursive)` override; otherwise detect direct self-reference syntactically.
    // Indirect cycles (A -> B -> A) need `Recursive` on one type of the cycle: the indirection is
    // type-level, so one is enough. A missed cycle is a compile error, never wrong bytes.
    let recursive = cereal_attr.recursive.unwrap_or_else(|| shared::detect_self_reference(&item));
    
    let derives = make_derives(&cereal_attr, recursive)?;
    // rkyv: replace perfect-derive field bounds (which cycle on recursive types) with
    // parameter/serializer bounds. Item attrs must follow the #[derive] that introduces `rkyv`.
    let rkyv_attrs = match cereal_attr.rkyv.derive {
        true => rkyved::make_item_attrs(&item.generics),
        false => quote! {},
    };
    if cereal_attr.rkyv.derive {
        rkyved::add_omit_bounds(&mut item);
    }
    
    if let Some(bad) = shared::direct_self_fields(&item).into_iter().next() {
        return Err(syn::Error::new_spanned(
            bad,
            format!(
                "cereal: self-referential type usage: `{}`. possibly use `Box<{}>`",
                item.ident, item.ident,
            ),
        ));
    }
    
    let bitcode_recursive = match cereal_attr.bitcode.derive && recursive {
        true => bitcoded::make_recursive_impls(&item)?,
        false => quote! {},
    };
    
    Ok(quote! {
        #derives
        #rkyv_attrs
        #item
        #bitcode_recursive
    })
}
struct CerealAttr {
    /// `Some(true)` = `Recursive`, `Some(false)` = `not(Recursive)`, `None` = auto-detect.
    recursive: Option<bool>,
    bitcode: AttrOpt,
    clone: AttrOpt,
    copy: AttrOpt,
    debug: AttrOpt,
    hash: AttrOpt,
    partial_eq: AttrOpt,
    eq: AttrOpt,
    rkyv: AttrOpt,
    serde: AttrOpt,
}
struct AttrOpt {
    derive: bool,
}
impl AttrOpt {
    const DERIVE: Self = Self { derive: true };
    const NOT: Self = Self { derive: false };
}
impl CerealAttr {
    const DEFAULT: Self = Self {
        recursive: None,
        bitcode: AttrOpt::DERIVE,
        clone: AttrOpt::DERIVE,
        copy: AttrOpt::NOT,
        debug: AttrOpt::DERIVE,
        hash: AttrOpt::NOT,
        partial_eq: AttrOpt::DERIVE,
        eq: AttrOpt::NOT,
        rkyv: AttrOpt::DERIVE,
        serde: AttrOpt::DERIVE,
    };
}
fn parse_attr(attrs: proc_macro2::TokenStream) -> syn::Result<CerealAttr> {
    const HAS: &'static str = "has";
    const NOT: &'static str = "not";
    
    let metas: syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>
        = syn::punctuated::Punctuated::parse_terminated.parse2(attrs)?;
    
    let mut cereal_attrs = CerealAttr::DEFAULT;
    for meta in metas {
        let ident = meta.path().require_ident()?;
        if ident == HAS {
            meta.require_list()?.parse_nested_meta(|meta_has| {
                let ident_has = meta_has.path.require_ident()?;
                if ident_has == "Debug" {
                    cereal_attrs.debug.derive = false;
                } else if ident_has == "Clone" {
                    cereal_attrs.clone.derive = false;
                } else if ident_has == "Copy" {
                    cereal_attrs.copy.derive = false;
                } else if ident_has == "Hash" {
                    cereal_attrs.hash.derive = false;
                } else if ident_has == "PartialEq" {
                    cereal_attrs.partial_eq.derive = false;
                } else if ident_has == "Rkyv" {
                    cereal_attrs.rkyv.derive = false;
                } else if ident_has == "Serde" {
                    cereal_attrs.serde.derive = false;
                } else if ident_has == "Bitcode" {
                    cereal_attrs.bitcode.derive = false;
                } else {
                    return Err(meta_has.error("unknown cereal attribute"));
                }
                
                Ok(())
            })?;
        } else if ident == NOT {
            meta.require_list()?.parse_nested_meta(|meta_not| {
                let ident_not = meta_not.path.require_ident()?;
                if ident_not == "Debug" {
                    cereal_attrs.debug = AttrOpt::NOT;
                } else if ident_not == "Clone" {
                    cereal_attrs.clone = AttrOpt::NOT; 
                } else if ident_not == "Copy" {
                    cereal_attrs.copy = AttrOpt::NOT;
                } else if ident_not == "Hash" {
                    cereal_attrs.hash = AttrOpt::NOT;
                } else if ident_not == "PartialEq" {
                    cereal_attrs.partial_eq = AttrOpt::NOT;
                } else if ident_not == "Rkyv" {
                    cereal_attrs.rkyv = AttrOpt::NOT;
                } else if ident_not == "Serde" {
                    cereal_attrs.serde = AttrOpt::NOT;
                } else if ident_not == "Bitcode" {
                    cereal_attrs.bitcode = AttrOpt::NOT;
                } else if ident_not == "Recursive" {
                    cereal_attrs.recursive = Some(false);
                } else {
                    return Err(meta_not.error("unknown cereal attribute"));
                }
                
                Ok(())
            })?;
        } else if ident == "Eq" {
            cereal_attrs.eq = AttrOpt::DERIVE;
        } else if ident == "Hash" {
            cereal_attrs.hash = AttrOpt::DERIVE;
        } else if ident == "Copy" {
            cereal_attrs.copy = AttrOpt::DERIVE;
        } else if ident == "Recursive" {
            cereal_attrs.recursive = Some(true);
        } else if ident == "Data" {
            // Marker traits are blanket-implemented in the runtime crate; nothing to emit.
        } else {
            return Err(syn::Error::new_spanned(ident, "unknown cereal attribute"));
        }
    }
    
    Ok(cereal_attrs)
}
#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    //use pretty_assertions::assert_eq;
    
    #[test]
    fn test_derive_default() {
        let item = quote! {
            struct TestStruct {
                foo: bool,
            }
        };
        let attr = quote! {};
        
        parse_derived(attr, item).unwrap();
    }
    
    #[test]
    fn test_derive_has() {
        let item = quote! {
            struct TestStruct {
                foo: bool,
            }
        };
        let attr = quote! { has(Debug, PartialEq) };
        
        parse_derived(attr, item).unwrap();
    }
    
    #[test]
    fn test_derive_not() {
        let item = quote! {
            struct TestStruct {
                foo: bool,
            }
        };
        let attr = quote! { not(Debug, Serde) };
        
        parse_derived(attr, item).unwrap();
    }
    
    #[test]
    fn test_non_recursive_derives_bitcode() {
        let item = quote! {
            struct TestStruct {
                foo: bool,
            }
        };
        let out = parse_derived(quote! { Data }, item).unwrap().to_string();
        assert!(out.contains(":: serde :: Deserialize , :: bitcode :: Encode , :: bitcode :: Decode , :: rkyv :: Archive"), "{out}");
        assert!(!out.contains("LazyEncoder"), "{out}");
        assert!(out.contains("omit_bounds"), "{out}");
        assert!(out.contains("serialize_bounds"), "{out}");
    }
    
    #[test]
    fn test_recursive_enum_detected() {
        let item = quote! {
            pub enum MyData {
                Bool(bool),
                Vector(Vec<MyData>),
            }
        };
        let out = parse_derived(quote! { Data }, item).unwrap().to_string();
        // bitcode is not in the item's derive list (serde is directly followed by rkyv);
        // the mirrors carry their own #[derive(::bitcode::Encode/Decode)].
        assert!(out.contains(":: serde :: Deserialize , :: rkyv :: Archive"), "{out}");
        assert!(out.contains("LazyEncoder"), "{out}");
        assert!(out.contains("LazyDecoder"), "{out}");
        assert!(out.contains("__CerealBitcodeEncMyData"), "{out}");
        assert!(out.contains("__CerealBitcodeDecMyData"), "{out}");
    }
    
    #[test]
    fn test_recursive_via_self() {
        let item = quote! {
            struct List {
                head: u32,
                tail: Option<Box<Self>>,
            }
        };
        let out = parse_derived(quote! { Data }, item).unwrap().to_string();
        assert!(out.contains("LazyEncoder"), "{out}");
        // `Self` in the mirrors must name the real type, not the mirror.
        assert!(out.contains("Option < Box < List > >"), "{out}");
    }
    
    #[test]
    fn test_recursive_explicit_and_negated() {
        let item = quote! {
            struct A { b: Vec<B> }
        };
        let out = parse_derived(quote! { Data, Recursive }, item.clone()).unwrap().to_string();
        assert!(out.contains("LazyEncoder"), "{out}");
        let out = parse_derived(quote! { Data, not(Recursive) }, item).unwrap().to_string();
        assert!(!out.contains("LazyEncoder"), "{out}");
    }
    
    #[test]
    fn test_recursive_generic_struct() {
        let item = quote! {
            struct Tree<T> {
                value: T,
                children: Vec<Tree<T>>,
            }
        };
        let out = parse_derived(quote! { Data }, item).unwrap().to_string();
        assert!(out.contains("T : :: bitcode :: Encode"), "{out}");
        assert!(out.contains("T : :: bitcode :: Decode < '__de >"), "{out}");
    }
}