use proc_macro2::{Group, TokenStream, TokenTree};
use quote::ToTokens;

/// Every field type of the item, in declaration order (all variants for enums).
pub(crate) fn field_types(item: &syn::DeriveInput) -> Vec<&syn::Type> {
    match &item.data {
        syn::Data::Struct(s) => s.fields.iter().map(|f| &f.ty).collect(),
        syn::Data::Enum(e) => e
            .variants
            .iter()
            .flat_map(|v| v.fields.iter().map(|f| &f.ty))
            .collect(),
        syn::Data::Union(u) => u.fields.named.iter().map(|f| &f.ty).collect(),
    }
}

/// True if any field type mentions the item's own name or `Self`.
///
/// Purely syntactic: it catches direct recursion (`Vec<MyData>`, `Box<Self>`) but not cycles
/// through other types, aliases, or projections. False positives only cost an extra indirection;
/// false negatives reproduce the original struct-tail compile error, never wrong bytes.
pub(crate) fn detect_self_reference(item: &syn::DeriveInput) -> bool {
    fn mentions(ts: TokenStream, target: &syn::Ident) -> bool {
        ts.into_iter().any(|tt| match tt {
            TokenTree::Ident(i) => i == *target || i == "Self",
            TokenTree::Group(g) => mentions(g.stream(), target),
            _ => false,
        })
    }
    field_types(item)
        .into_iter()
        .any(|ty| mentions(ty.to_token_stream(), &item.ident))
}

/// Replace every `Self` token with `self_ty`. Needed because a `Self` inside a generated mirror
/// type would name the mirror; bitcode's own derive rewrites `Self` to the item ident, which for
/// a mirror is also the wrong type.
pub(crate) fn subst_self(ts: TokenStream, self_ty: &TokenStream) -> TokenStream {
    ts.into_iter()
        .map(|tt| -> TokenStream {
            match tt {
                TokenTree::Ident(i) if i == "Self" => self_ty.clone(),
                TokenTree::Group(g) => {
                    let mut ng = Group::new(g.delimiter(), subst_self(g.stream(), self_ty));
                    ng.set_span(g.span());
                    TokenTree::Group(ng).into()
                }
                other => other.into(),
            }
        })
        .collect()
}

/// `base` where-clause plus `extra` predicates; `None` if the result would be empty.
pub(crate) fn extend_where(
    base: Option<&syn::WhereClause>,
    extra: impl IntoIterator<Item = syn::WherePredicate>,
) -> Option<syn::WhereClause> {
    let mut wc = base.cloned().unwrap_or_else(|| syn::WhereClause {
        where_token: Default::default(),
        predicates: Default::default(),
    });
    wc.predicates.extend(extra);
    if wc.predicates.is_empty() {
        None
    } else {
        Some(wc)
    }
}

/// True if `ty`'s outermost type is exactly `ident` (self-inclusion with no indirection),
/// e.g. `foo: OtherData` inside `struct OtherData`. `Box<OtherData>`, `Vec<OtherData>`, etc.
/// are *not* matches: their outermost type is `Box`/`Vec`, not `OtherData`.
fn is_bare_self(ty: &syn::Type, ident: &syn::Ident) -> bool {
    match ty {
        // Peel `(T)` and grouping so `foo: (OtherData)` still counts.
        syn::Type::Paren(p) => is_bare_self(&p.elem, ident),
        syn::Type::Group(g) => is_bare_self(&g.elem, ident),
        syn::Type::Path(tp) if tp.qself.is_none() => {
            // The whole path must be a single segment equal to the ident.
            // `Box<OtherData>` is a path too, but its segment ident is `Box`, so no match.
            tp.path.get_ident().map_or(false, |id| id == ident)
                || (tp.path.segments.len() == 1 && tp.path.segments[0].ident == *ident)
        }
        _ => false,
    }
}

/// Fields that include the type directly (no indirection) — an infinitely-sized type that
/// no amount of encoding machinery can rescue. Returns the offending field types for diagnostics.
pub(crate) fn direct_self_fields<'a>(item: &'a syn::DeriveInput) -> Vec<&'a syn::Type> {
    // `Self` inside a field resolves to the item; check for it too.
    let ident = &item.ident;
    field_types(item)
        .into_iter()
        .filter(|ty| {
            is_bare_self(ty, ident)
                || matches!(ty, syn::Type::Path(tp) if tp.qself.is_none() && tp.path.is_ident("Self"))
        })
        .collect()
}