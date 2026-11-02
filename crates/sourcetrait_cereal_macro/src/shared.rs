//! Syntax the generated derives share: a type's fields, its recursion, `Self`,
//! and where-clauses.
use proc_macro2::{Group, TokenStream, TokenTree};
use quote::ToTokens;

/// Each field's type in declaration order, every variant's for an enum.
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

/// Whether a field's type names the item, by its name or as `Self`.
///
/// By syntax alone: it finds a type's own recursion, `Vec<MyData>` or
/// `Box<Self>`, never a cycle through another type, an alias, or a projection,
/// which takes `Recursive`. A miss fails to compile; it never writes wrong
/// bytes.
pub(crate) fn detect_self_reference(item: &syn::DeriveInput) -> bool {
    fn names(tokens: TokenStream, target: &syn::Ident) -> bool {
        tokens.into_iter().any(|tt| match tt {
            TokenTree::Ident(i) => i == *target || i == "Self",
            TokenTree::Group(g) => names(g.stream(), target),
            _ => false,
        })
    }
    field_types(item)
        .into_iter()
        .any(|ty| names(ty.to_token_stream(), &item.ident))
}

/// `tokens` with each `Self` replaced by `self_ty`: in a generated mirror,
/// `Self` would name the mirror.
pub(crate) fn subst_self(tokens: TokenStream, self_ty: &TokenStream) -> TokenStream {
    tokens
        .into_iter()
        .map(|tt| match tt {
            TokenTree::Ident(i) if i == "Self" => self_ty.clone(),
            TokenTree::Group(g) => {
                let mut group = Group::new(g.delimiter(), subst_self(g.stream(), self_ty));
                group.set_span(g.span());
                TokenTree::Group(group).into()
            }
            other => other.into(),
        })
        .collect()
}

/// `base` with `extra`'s predicates added, or none when that leaves it empty.
pub(crate) fn extend_where(
    base: Option<&syn::WhereClause>,
    extra: impl IntoIterator<Item = syn::WherePredicate>,
) -> Option<syn::WhereClause> {
    let mut clause = base.cloned().unwrap_or_else(|| syn::WhereClause {
        where_token: Default::default(),
        predicates: Default::default(),
    });
    clause.predicates.extend(extra);
    (!clause.predicates.is_empty()).then_some(clause)
}

/// The field types holding the item itself with no indirection, `foo: MyData`
/// or `foo: Self`: a type of infinite size, which no encoding rescues.
pub(crate) fn direct_self_fields(item: &syn::DeriveInput) -> Vec<&syn::Type> {
    field_types(item)
        .into_iter()
        .filter(|ty| is_bare(ty, &item.ident))
        .collect()
}

/// Whether `ty` is the item itself, by its name or as `Self`, parentheses and
/// groups peeled: `Box<MyData>` is not, its outermost type being `Box`.
fn is_bare(ty: &syn::Type, ident: &syn::Ident) -> bool {
    match ty {
        syn::Type::Paren(p) => is_bare(&p.elem, ident),
        syn::Type::Group(g) => is_bare(&g.elem, ident),
        syn::Type::Path(tp) if tp.qself.is_none() && tp.path.segments.len() == 1 => {
            let segment = &tp.path.segments[0].ident;
            segment == ident || segment == "Self"
        }
        _ => false,
    }
}
