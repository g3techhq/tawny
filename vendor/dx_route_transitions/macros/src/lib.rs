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
/// - `#[transition(cover)]` marks a sheet/modal route. Navigating from a base
///   route to a cover route returns `NavigationAnimation::CoverUp`; navigating
///   back from cover to base returns `NavigationAnimation::UncoverDown`.
/// - `#[transition(morph)]` marks a route that grows out of a card on a base
///   route. Base-to-morph returns `NavigationAnimation::MorphIn`;
///   morph-to-base returns `NavigationAnimation::MorphOut`.
/// - `push(group = name, order = field)` groups ordered peer routes. The order
///   field must implement `Ord`. Moving to a greater value pushes left; moving
///   to a smaller value pushes right.
/// - `key = field` or `key = (field_a, field_b)` scopes push comparisons to
///   matching route parameters.
///
/// Example:
///
/// ```ignore
/// #[route_transitions]
/// #[derive(Clone, Routable, PartialEq)]
/// enum Route {
///     #[transition(base, push(group = tabs, order = tab))]
///     #[route("/items?:tab")]
///     Items { tab: ItemsTab },
///
///     #[transition(cover, push(group = item_details, key = item_id, order = tab))]
///     #[route("/items/:item_id?:tab")]
///     ItemDetails { item_id: String, tab: ItemTab },
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

    quote! {
        #route_enum

        impl #enum_ident {
            fn transition_layer(&self) -> ::dx_route_transitions::RouteTransitionLayer {
                match self {
                    #(#layer_arms),*
                }
            }

            fn transition_push_ordering_to(&self, next: &#enum_ident) -> Option<std::cmp::Ordering> {
                match (self, next) {
                    #(#push_arms,)*
                    _ => None,
                }
            }

            pub(crate) fn transition_to(
                &self,
                next: &#enum_ident,
            ) -> ::dx_route_transitions::NavigationAnimation {
                <Self as ::dx_route_transitions::RouteTransitions>::transition_to(self, next)
            }
        }

        impl ::dx_route_transitions::RouteTransitions for #enum_ident {
            fn transition_to(
                &self,
                next: &#enum_ident,
            ) -> ::dx_route_transitions::NavigationAnimation {
                if self == next {
                    return ::dx_route_transitions::NavigationAnimation::None;
                }

                match (self.transition_layer(), next.transition_layer()) {
                    (
                        ::dx_route_transitions::RouteTransitionLayer::Base,
                        ::dx_route_transitions::RouteTransitionLayer::Cover,
                    ) => return ::dx_route_transitions::NavigationAnimation::CoverUp,
                    (
                        ::dx_route_transitions::RouteTransitionLayer::Cover,
                        ::dx_route_transitions::RouteTransitionLayer::Base,
                    ) => return ::dx_route_transitions::NavigationAnimation::UncoverDown,
                    (
                        ::dx_route_transitions::RouteTransitionLayer::Base,
                        ::dx_route_transitions::RouteTransitionLayer::Morph,
                    ) => return ::dx_route_transitions::NavigationAnimation::MorphIn,
                    (
                        ::dx_route_transitions::RouteTransitionLayer::Morph,
                        ::dx_route_transitions::RouteTransitionLayer::Base,
                    ) => return ::dx_route_transitions::NavigationAnimation::MorphOut,
                    _ => {}
                }

                if let Some(ordering) = self.transition_push_ordering_to(next) {
                    return match ordering {
                        std::cmp::Ordering::Greater => {
                            ::dx_route_transitions::NavigationAnimation::PushLeft
                        }
                        std::cmp::Ordering::Less => {
                            ::dx_route_transitions::NavigationAnimation::PushRight
                        }
                        std::cmp::Ordering::Equal => {
                            ::dx_route_transitions::NavigationAnimation::None
                        }
                    };
                }

                ::dx_route_transitions::NavigationAnimation::Fade
            }
        }
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
                    quote! { ::dx_route_transitions::RouteTransitionLayer::Base }
                }
                RouteLayer::Cover => {
                    quote! { ::dx_route_transitions::RouteTransitionLayer::Cover }
                }
                RouteLayer::Morph => {
                    quote! { ::dx_route_transitions::RouteTransitionLayer::Morph }
                }
            };
            Ok(quote! { #pattern => #layer })
        })
        .collect()
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
                    (#from_pattern, #to_pattern) if #key_guard => Some(#to_order.cmp(#from_order))
                });
            }
        }
    }

    Ok(arms)
}

fn validate_push_fields(variant: &RouteVariant, push: &PushArgs) -> Result<()> {
    let Fields::Named(fields) = &variant.fields else {
        return Err(Error::new_spanned(
            &variant.ident,
            "push transitions require named route fields",
        ));
    };

    let field_names = fields
        .named
        .iter()
        .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
        .collect::<BTreeSet<_>>();

    for field in push.used_fields() {
        if !field_names.contains(&field.to_string()) {
            return Err(Error::new_spanned(
                field,
                "transition push field is not present on this route variant",
            ));
        }
    }

    Ok(())
}

fn build_layer_pattern(
    enum_ident: &Ident,
    variant_ident: &Ident,
    fields: &Fields,
) -> Result<TokenStream2> {
    match fields {
        Fields::Named(_) => Ok(quote! { #enum_ident::#variant_ident { .. } }),
        Fields::Unnamed(_) => Err(Error::new_spanned(
            variant_ident,
            "route transition macro only supports named or unit route variants",
        )),
        Fields::Unit => Ok(quote! { #enum_ident::#variant_ident }),
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
            Ok(quote! { #enum_ident::#variant_ident { #(#bindings),*, .. } })
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
        .map(|(from, to)| quote! { #from == #to })
        .collect::<Vec<_>>();

    if comparisons.is_empty() {
        quote! { true }
    } else {
        quote! { #(#comparisons)&&* }
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
}

impl Default for TransitionArgs {
    fn default() -> Self {
        Self {
            layer: RouteLayer::Base,
            push: None,
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
                "push" => {
                    let content;
                    parenthesized!(content in input);
                    args.push = Some(content.parse()?);
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
            .map(|(field, alias)| quote! { #field: #alias })
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
