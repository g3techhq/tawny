//! Proc-macro support for [`g3-route-transitions`](https://docs.rs/g3-route-transitions).
//!
//! This crate is an implementation detail; use `g3_route_transitions::route_transitions`
//! rather than depending on it directly.
#![warn(missing_docs)]
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Error, Fields, Ident, ItemEnum, Result, Token, parenthesized, parse_macro_input};
/// Adds route-owned View Transition metadata to a Dioxus `Routable` enum.
///
/// Place this attribute on the same enum that derives `Routable`, then add
/// `#[transition(...)]` metadata to individual route variants.
///
/// Supported variant metadata:
///
/// - `#[transition(base)]` marks a normal page. This is also the default.
/// - `#[transition(root)]` marks a stable application root such as a
///   bottom-tab destination.
/// - `#[transition(pushed)]` marks a full-screen page pushed above a root.
/// - `#[transition(cover)]` marks a sheet/modal route. Navigating into a cover
///   returns `NavigationAnimation::CoverUp`; navigating out returns
///   `NavigationAnimation::UncoverDown`.
/// - `#[transition(morph)]` marks a route that grows out of a card on a base
///   route. Base-to-morph returns `NavigationAnimation::MorphIn`;
///   morph-to-base returns `NavigationAnimation::MorphOut`.
/// - `push(group = name, order = field)` groups ordered peer routes. The order
///   field must implement `Ord`. Moving to a greater value pushes left; moving
///   to a smaller value pushes right.
/// - `key = field` or `key = (field_a, field_b)` scopes push comparisons to
///   matching route parameters.
/// - `forward = Route` or `forward = (RouteA, RouteB)` declares directed
///   drill-down destinations. Forward navigation pushes left; its reverse
///   pushes right.
/// - `replace` makes changes between values of the same variant update the
///   current browser-history entry without a page animation.
///   `replace(key = field)` scopes that behavior to a logical record.
///
/// Example:
///
/// ```ignore
/// #[route_transitions]
/// #[derive(Clone, Routable, PartialEq)]
/// enum Route {
///     #[transition(root, replace)]
///     #[route("/items?:tab")]
///     Items { tab: ItemsTab },
///
///     #[transition(pushed, replace(key = item_id), forward = ItemComments)]
///     #[route("/items/:item_id?:tab")]
///     ItemDetails { item_id: String, tab: ItemTab },
///
///     #[transition(pushed)]
///     #[route("/items/:item_id/comments")]
///     ItemComments { item_id: String },
/// }
/// ```
#[proc_macro_attribute]
pub fn route_transitions(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut route_enum = parse_macro_input!(item as ItemEnum);
    let mut route_variants = Vec::new();
    for variant in &mut route_enum.variants {
        let transition = match take_transition_attr(variant) {
            Ok(transition) => transition,
            Err(error) => return error.to_compile_error().into(),
        };
        route_variants.push(RouteVariant {
            ident: variant.ident.clone(),
            fields: variant.fields.clone(),
            transition: transition.unwrap_or_default(),
        });
    }
    let enum_ident = &route_enum.ident;
    let layer_arms = match build_layer_arms(enum_ident, &route_variants) {
        Ok(arms) => arms,
        Err(error) => return error.to_compile_error().into(),
    };
    let push_arms = match build_push_ordering_arms(enum_ident, &route_variants) {
        Ok(arms) => arms,
        Err(error) => return error.to_compile_error().into(),
    };
    let forward_arms = match build_forward_arms(enum_ident, &route_variants) {
        Ok(arms) => arms,
        Err(error) => return error.to_compile_error().into(),
    };
    let replace_arms = match build_replace_arms(enum_ident, &route_variants) {
        Ok(arms) => arms,
        Err(error) => return error.to_compile_error().into(),
    };
    quote! {
        # route_enum impl # enum_ident { fn transition_layer(& self) ->
        ::g3_route_transitions::RouteTransitionLayer { match self { # (# layer_arms),* }
        } fn transition_push_ordering_to(& self, next : &# enum_ident) -> Option <
        std::cmp::Ordering > { match (self, next) { # (# push_arms,) * _ => None, } } fn
        transition_pushes_forward_to(& self, next : &# enum_ident) -> bool { match (self,
        next) { # (# forward_arms,) * _ => false, } } fn transition_replaces_to(& self,
        next : &# enum_ident) -> bool { match (self, next) { # (# replace_arms,) * _ =>
        false, } } pub (crate) fn transition_to(& self, next : &# enum_ident,) ->
        ::g3_route_transitions::NavigationAnimation { < Self as
        ::g3_route_transitions::RouteTransitions >::transition_to(self, next) } } impl
        ::g3_route_transitions::RouteTransitions for # enum_ident { fn transition_to(&
        self, next : &# enum_ident,) -> ::g3_route_transitions::NavigationAnimation { if
        self == next { return ::g3_route_transitions::NavigationAnimation::None; } if
        self.transition_replaces_to(next) { return
        ::g3_route_transitions::NavigationAnimation::None; } if self
        .transition_pushes_forward_to(next) { return
        ::g3_route_transitions::NavigationAnimation::PushLeft; } if next
        .transition_pushes_forward_to(self) { return
        ::g3_route_transitions::NavigationAnimation::PushRight; } match (self
        .transition_layer(), next.transition_layer()) { (current,
        ::g3_route_transitions::RouteTransitionLayer::Cover,) if current !=
        ::g3_route_transitions::RouteTransitionLayer::Cover => { return
        ::g3_route_transitions::NavigationAnimation::CoverUp },
        (::g3_route_transitions::RouteTransitionLayer::Cover, next,) if next !=
        ::g3_route_transitions::RouteTransitionLayer::Cover => { return
        ::g3_route_transitions::NavigationAnimation::UncoverDown },
        (::g3_route_transitions::RouteTransitionLayer::Base,
        ::g3_route_transitions::RouteTransitionLayer::Morph,) => return
        ::g3_route_transitions::NavigationAnimation::MorphIn,
        (::g3_route_transitions::RouteTransitionLayer::Morph,
        ::g3_route_transitions::RouteTransitionLayer::Base,) => return
        ::g3_route_transitions::NavigationAnimation::MorphOut,
        (::g3_route_transitions::RouteTransitionLayer::Root,
        ::g3_route_transitions::RouteTransitionLayer::Pushed,) => return
        ::g3_route_transitions::NavigationAnimation::PushLeft,
        (::g3_route_transitions::RouteTransitionLayer::Pushed,
        ::g3_route_transitions::RouteTransitionLayer::Root,) => return
        ::g3_route_transitions::NavigationAnimation::PushRight, _ => {} } if let
        Some(ordering) = self.transition_push_ordering_to(next) { return match ordering {
        std::cmp::Ordering::Greater => {
        ::g3_route_transitions::NavigationAnimation::PushLeft } std::cmp::Ordering::Less
        => { ::g3_route_transitions::NavigationAnimation::PushRight }
        std::cmp::Ordering::Equal => { ::g3_route_transitions::NavigationAnimation::None
        } }; } ::g3_route_transitions::NavigationAnimation::Fade } fn replaces_history(&
        self, next : &# enum_ident) -> bool { self.transition_replaces_to(next) } fn
        transition_back(& self) -> ::g3_route_transitions::NavigationAnimation { match
        self.transition_layer() { ::g3_route_transitions::RouteTransitionLayer::Cover =>
        { ::g3_route_transitions::NavigationAnimation::UncoverDown }
        ::g3_route_transitions::RouteTransitionLayer::Pushed => {
        ::g3_route_transitions::NavigationAnimation::PushRight }
        ::g3_route_transitions::RouteTransitionLayer::Morph => {
        ::g3_route_transitions::NavigationAnimation::MorphOut } _ =>
        ::g3_route_transitions::NavigationAnimation::Fade, } } }
    }
    .into()
}
fn take_transition_attr(variant: &mut syn::Variant) -> Result<Option<TransitionArgs>> {
    let mut transition = None;
    let mut attrs = Vec::new();
    for attr in variant.attrs.drain(..) {
        if attr.path().is_ident("transition") {
            if transition.is_some() {
                return Err(Error::new_spanned(attr, "duplicate transition attribute"));
            }
            transition = Some(attr.parse_args::<TransitionArgs>()?);
        } else {
            attrs.push(attr);
        }
    }
    variant.attrs = attrs;
    Ok(transition)
}
fn build_layer_arms(
    enum_ident: &Ident,
    route_variants: &[RouteVariant],
) -> Result<Vec<TokenStream2>> {
    route_variants
        .iter()
        .map(|variant| {
            let pattern = build_layer_pattern(enum_ident, &variant.ident, &variant.fields)?;
            let layer = match variant.transition.layer {
                RouteLayer::Base => {
                    quote! {
                        ::g3_route_transitions::RouteTransitionLayer::Base
                    }
                }
                RouteLayer::Cover => {
                    quote! {
                        ::g3_route_transitions::RouteTransitionLayer::Cover
                    }
                }
                RouteLayer::Morph => {
                    quote! {
                        ::g3_route_transitions::RouteTransitionLayer::Morph
                    }
                }
                RouteLayer::Root => {
                    quote! {
                        ::g3_route_transitions::RouteTransitionLayer::Root
                    }
                }
                RouteLayer::Pushed => {
                    quote! {
                        ::g3_route_transitions::RouteTransitionLayer::Pushed
                    }
                }
            };
            Ok(quote! {
                # pattern => # layer
            })
        })
        .collect()
}
fn build_forward_arms(
    enum_ident: &Ident,
    route_variants: &[RouteVariant],
) -> Result<Vec<TokenStream2>> {
    let variants_by_name = route_variants
        .iter()
        .map(|variant| (variant.ident.to_string(), variant))
        .collect::<BTreeMap<_, _>>();
    let mut arms = Vec::new();
    for from in route_variants {
        let from_pattern = build_layer_pattern(enum_ident, &from.ident, &from.fields)?;
        for target in &from.transition.forward {
            let Some(to) = variants_by_name.get(&target.to_string()) else {
                return Err(Error::new_spanned(
                    target,
                    "forward transition target is not a route variant",
                ));
            };
            let to_pattern = build_layer_pattern(enum_ident, &to.ident, &to.fields)?;
            arms.push(quote! {
                (# from_pattern, # to_pattern) => true
            });
        }
    }
    Ok(arms)
}
fn build_replace_arms(
    enum_ident: &Ident,
    route_variants: &[RouteVariant],
) -> Result<Vec<TokenStream2>> {
    let mut arms = Vec::new();
    for variant in route_variants {
        let Some(replace) = &variant.transition.replace else {
            continue;
        };
        validate_named_fields(variant, replace.key.iter().collect(), "replace")?;
        if replace.key.is_empty() {
            let from = build_layer_pattern(enum_ident, &variant.ident, &variant.fields)?;
            let to = build_layer_pattern(enum_ident, &variant.ident, &variant.fields)?;
            arms.push(quote! {
                (# from, # to) => true
            });
            continue;
        }
        let from_aliases = key_aliases("__route_transition_replace_from", &replace.key);
        let to_aliases = key_aliases("__route_transition_replace_to", &replace.key);
        let from = build_key_pattern(enum_ident, &variant.ident, &variant.fields, &from_aliases)?;
        let to = build_key_pattern(enum_ident, &variant.ident, &variant.fields, &to_aliases)?;
        let guard = from_aliases
            .iter()
            .zip(&to_aliases)
            .map(|((_, from), (_, to))| {
                quote! {
                    # from == # to
                }
            })
            .collect::<Vec<_>>();
        arms.push(quote! {
            (# from, # to) if # (# guard) &&* => true
        });
    }
    Ok(arms)
}
fn build_push_ordering_arms(
    enum_ident: &Ident,
    route_variants: &[RouteVariant],
) -> Result<Vec<TokenStream2>> {
    let mut groups: BTreeMap<String, Vec<&RouteVariant>> = BTreeMap::new();
    for variant in route_variants {
        if let Some(push) = &variant.transition.push {
            validate_push_fields(variant, push)?;
            groups
                .entry(push.group.to_string())
                .or_default()
                .push(variant);
        }
    }
    let mut arms = Vec::new();
    for variants in groups.values() {
        for from in variants {
            for to in variants {
                let from_push = from.transition.push.as_ref().expect("grouped push variant");
                let to_push = to.transition.push.as_ref().expect("grouped push variant");
                let from_aliases = FieldAliases::new("__route_transition_from", from_push);
                let to_aliases = FieldAliases::new("__route_transition_to", to_push);
                let from_pattern =
                    build_push_pattern(enum_ident, &from.ident, &from.fields, &from_aliases)?;
                let to_pattern =
                    build_push_pattern(enum_ident, &to.ident, &to.fields, &to_aliases)?;
                let key_guard = build_key_guard(&from_aliases, &to_aliases);
                let from_order = from_aliases.order_alias();
                let to_order = to_aliases.order_alias();
                arms.push(quote! {
                    (# from_pattern, # to_pattern) if # key_guard => Some(# to_order
                    .cmp(# from_order))
                });
            }
        }
    }
    Ok(arms)
}
fn validate_push_fields(variant: &RouteVariant, push: &PushArgs) -> Result<()> {
    validate_named_fields(variant, push.used_fields(), "push")
}
fn validate_named_fields(
    variant: &RouteVariant,
    used_fields: Vec<&Ident>,
    transition_name: &str,
) -> Result<()> {
    if used_fields.is_empty() {
        return Ok(());
    }
    let Fields::Named(fields) = &variant.fields else {
        return Err(Error::new_spanned(
            &variant.ident,
            format!("{transition_name} transitions require named route fields"),
        ));
    };
    let field_names = fields
        .named
        .iter()
        .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
        .collect::<BTreeSet<_>>();
    for field in used_fields {
        if !field_names.contains(&field.to_string()) {
            return Err(Error::new_spanned(
                field,
                format!("transition {transition_name} field is not present on this route variant",),
            ));
        }
    }
    Ok(())
}
fn key_aliases(prefix: &str, fields: &[Ident]) -> Vec<(Ident, Ident)> {
    fields
        .iter()
        .cloned()
        .map(|field| {
            let alias = format_ident!("{}_{}", prefix, field);
            (field, alias)
        })
        .collect()
}
fn build_key_pattern(
    enum_ident: &Ident,
    variant_ident: &Ident,
    fields: &Fields,
    aliases: &[(Ident, Ident)],
) -> Result<TokenStream2> {
    match fields {
        Fields::Named(_) => {
            let bindings = aliases
                .iter()
                .map(|(field, alias)| {
                    quote! {
                        # field : # alias
                    }
                })
                .collect::<Vec<_>>();
            Ok(quote! {
                # enum_ident::# variant_ident { # (# bindings),*, .. }
            })
        }
        Fields::Unnamed(_) | Fields::Unit => Err(Error::new_spanned(
            variant_ident,
            "keyed replace transitions require named route fields",
        )),
    }
}
fn build_layer_pattern(
    enum_ident: &Ident,
    variant_ident: &Ident,
    fields: &Fields,
) -> Result<TokenStream2> {
    match fields {
        Fields::Named(_) => Ok(quote! {
            # enum_ident::# variant_ident { .. }
        }),
        Fields::Unnamed(_) => Err(Error::new_spanned(
            variant_ident,
            "route transition macro only supports named or unit route variants",
        )),
        Fields::Unit => Ok(quote! {
            # enum_ident::# variant_ident
        }),
    }
}
fn build_push_pattern(
    enum_ident: &Ident,
    variant_ident: &Ident,
    fields: &Fields,
    aliases: &FieldAliases,
) -> Result<TokenStream2> {
    match fields {
        Fields::Named(_) => {
            let bindings = aliases.bindings();
            Ok(quote! {
                # enum_ident::# variant_ident { # (# bindings),*, .. }
            })
        }
        Fields::Unnamed(_) => Err(Error::new_spanned(
            variant_ident,
            "route transition macro only supports named or unit route variants",
        )),
        Fields::Unit => Err(Error::new_spanned(
            variant_ident,
            "push transitions require named route fields",
        )),
    }
}
fn build_key_guard(from_aliases: &FieldAliases, to_aliases: &FieldAliases) -> TokenStream2 {
    let comparisons = from_aliases
        .key_aliases()
        .into_iter()
        .zip(to_aliases.key_aliases())
        .map(|(from, to)| {
            quote! {
                # from == # to
            }
        })
        .collect::<Vec<_>>();
    if comparisons.is_empty() {
        quote! {
            true
        }
    } else {
        quote! {
            # (# comparisons) &&*
        }
    }
}
#[derive(Clone)]
struct RouteVariant {
    ident: Ident,
    fields: Fields,
    transition: TransitionArgs,
}
#[derive(Clone)]
struct TransitionArgs {
    layer: RouteLayer,
    push: Option<PushArgs>,
    forward: Vec<Ident>,
    replace: Option<ReplaceArgs>,
}
impl Default for TransitionArgs {
    fn default() -> Self {
        Self {
            layer: RouteLayer::Base,
            push: None,
            forward: Vec::new(),
            replace: None,
        }
    }
}
impl Parse for TransitionArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut args = Self::default();
        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            match ident.to_string().as_str() {
                "base" => args.layer = RouteLayer::Base,
                "cover" => args.layer = RouteLayer::Cover,
                "morph" => args.layer = RouteLayer::Morph,
                "root" => args.layer = RouteLayer::Root,
                "pushed" => args.layer = RouteLayer::Pushed,
                "push" => {
                    let content;
                    parenthesized!(content in input);
                    args.push = Some(content.parse()?);
                }
                "forward" => {
                    input.parse::<Token![=]>()?;
                    args.forward = parse_ident_list(input)?;
                }
                "replace" => {
                    args.replace = if input.peek(syn::token::Paren) {
                        let content;
                        parenthesized!(content in input);
                        Some(content.parse()?)
                    } else {
                        Some(ReplaceArgs::default())
                    };
                }
                _ => return Err(Error::new_spanned(ident, "unknown transition argument")),
            }
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(args)
    }
}
#[derive(Clone, Copy)]
enum RouteLayer {
    Base,
    Cover,
    Morph,
    Root,
    Pushed,
}
#[derive(Clone, Default)]
struct ReplaceArgs {
    key: Vec<Ident>,
}
impl Parse for ReplaceArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name: Ident = input.parse()?;
        if name.to_string() != "key" {
            return Err(Error::new_spanned(name, "unknown replace argument"));
        }
        input.parse::<Token![=]>()?;
        let key = parse_key(input)?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(input.error("replace accepts only a key argument"));
        }
        Ok(Self { key })
    }
}
#[derive(Clone)]
struct PushArgs {
    group: Ident,
    key: Vec<Ident>,
    order: Ident,
}
impl PushArgs {
    fn used_fields(&self) -> Vec<&Ident> {
        let mut fields = self.key.iter().collect::<Vec<_>>();
        if !fields.iter().any(|field| *field == &self.order) {
            fields.push(&self.order);
        }
        fields
    }
}
impl Parse for PushArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut group = None;
        let mut key = Vec::new();
        let mut order = None;
        while !input.is_empty() {
            let name: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            match name.to_string().as_str() {
                "group" => group = Some(input.parse()?),
                "key" => key = parse_key(input)?,
                "order" => order = Some(input.parse()?),
                _ => return Err(Error::new_spanned(name, "unknown push argument")),
            }
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self {
            group: group.ok_or_else(|| input.error("missing push group"))?,
            key,
            order: order.ok_or_else(|| input.error("missing push order"))?,
        })
    }
}
fn parse_key(input: ParseStream<'_>) -> Result<Vec<Ident>> {
    if input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in input);
        Ok(Punctuated::<Ident, Token![,]>::parse_terminated(&content)?
            .into_iter()
            .collect())
    } else {
        Ok(vec![input.parse()?])
    }
}
fn parse_ident_list(input: ParseStream<'_>) -> Result<Vec<Ident>> {
    if input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in input);
        Ok(Punctuated::<Ident, Token![,]>::parse_terminated(&content)?
            .into_iter()
            .collect())
    } else {
        Ok(vec![input.parse()?])
    }
}
struct FieldAliases {
    aliases: Vec<(Ident, Ident)>,
    key_len: usize,
}
impl FieldAliases {
    fn new(prefix: &str, push: &PushArgs) -> Self {
        let mut fields = push.key.clone();
        if !fields.iter().any(|field| field == &push.order) {
            fields.push(push.order.clone());
        }
        let aliases = fields
            .into_iter()
            .map(|field| {
                let alias = format_ident!("{}_{}", prefix, field);
                (field, alias)
            })
            .collect();
        Self {
            aliases,
            key_len: push.key.len(),
        }
    }
    fn bindings(&self) -> Vec<TokenStream2> {
        self.aliases
            .iter()
            .map(|(field, alias)| {
                quote! {
                    # field : # alias
                }
            })
            .collect()
    }
    fn key_aliases(&self) -> Vec<&Ident> {
        self.aliases
            .iter()
            .take(self.key_len)
            .map(|(_, alias)| alias)
            .collect()
    }
    fn order_alias(&self) -> &Ident {
        &self.aliases.last().expect("push aliases include order").1
    }
}
