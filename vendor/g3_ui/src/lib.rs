#![warn(missing_docs)]
//! g3_ui - reusable UI component library.

use manganis::Asset;
use manganis::asset;

/// The component stylesheet. `G3AppWrapper` links this for you; attach it
/// yourself only if the app manages its own `document::Link` tags.
pub static UI_CSS: Asset = asset!("/assets/g3_ui.css");

mod components;
mod descriptor;
mod theme;

pub use components::{
    AccordionGroup, AccordionItem, Avatar, AvatarSize, Badge, Button, ButtonSize, ButtonStyle,
    Checkbox, Chip, ControlLabelPlacement, Field, InfoButton, Item, ItemDetail, ItemDivider,
    ItemKind, Line, LineOrientation, List, ListLines, Progress, Radio, RadioGroup, Refresher,
    RefresherState, SegmentButton, SegmentGroup, Skeleton, SkeletonShape, Spinner, StatusColor,
    SwipeAction, SwipeBehavior, SwipeItem, SwipeSide, SwipeState, Toast, ToastPosition, Toggle,
    ToggleSize,
};

pub use components::{
    AccordionGroup as G3AccordionGroup, AccordionItem as G3AccordionItem, Avatar as G3Avatar,
    Badge as G3Badge, Button as G3Button, Checkbox as G3Checkbox, Chip as G3Chip,
    ControlLabelPlacement as G3ControlLabelPlacement, Field as G3Field, InfoButton as G3InfoButton,
    Item as G3Item, ItemDivider as G3ItemDivider, Line as G3Line, List as G3List,
    Progress as G3Progress, Radio as G3Radio, RadioGroup as G3RadioGroup, Refresher as G3Refresher,
    SegmentButton as G3SegmentButton, SegmentGroup as G3SegmentGroup, Skeleton as G3Skeleton,
    Spinner as G3Spinner, SwipeAction as G3SwipeAction, SwipeItem as G3SwipeItem, Toast as G3Toast,
    Toggle as G3Toggle, ToggleSize as G3ToggleSize,
};

pub use components::{
    Card, ConfirmModal, Fab, FabButton, FabContainer, FabHorizontal, FabList, FabListSide, FabSize,
    FabVertical, Modal, Navbar, NavbarTab, NavbarTabBar, RightSlot, Select, SelectOption, Sheet,
    SheetButton, SheetPlacement,
};

pub use components::{
    Card as G3Card, ConfirmModal as G3ConfirmModal, Fab as G3Fab, FabButton as G3FabButton,
    FabContainer as G3FabContainer, FabList as G3FabList, Modal as G3Modal, Navbar as G3Navbar,
    NavbarTab as G3NavbarTab, NavbarTabBar as G3NavbarTabBar, Select as G3Select, Sheet as G3Sheet,
    SheetButton as G3SheetButton, SheetPlacement as G3SheetPlacement,
};

pub use components::{AppWrapper, Body, Header};

pub use components::{AppWrapper as G3AppWrapper, Body as G3Body, Header as G3Header};

pub use descriptor::{ComponentDescriptor, component_descriptors};
#[cfg(feature = "playground")]
pub use descriptor::{ComponentPlaygroundDemo, PlaygroundDemoFrame, component_playground_demos};

/// Re-export the prelude for convenience.
pub mod prelude;

/// Re-export theme utilities.
pub use theme::{
    ComponentMode, G3Mode, G3Theme, G3ThemeProvider, Theme, get_mode, init_auto_mode,
    merge_classes, set_mode, use_component_mode,
};

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::prelude::*;

    const SEGMENT_SOURCE: &str = include_str!("components/segment.rs");

    fn render(app: fn() -> Element) {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
    }

    #[component]
    fn PrimitiveSmokeApp() -> Element {
        let checked = use_signal(|| false);
        let active = use_signal(|| 0_usize);
        let field_value = use_signal(String::new);

        rsx! {
            G3ThemeProvider { mode: ComponentMode::Ios,
                G3Button { onclick: |_| {}, "Button" }
                G3Button {
                    style: ButtonStyle::Neutral,
                    start: rsx! {
                        span { "G" }
                    },
                    onclick: |_| {},
                    "Provider"
                }
                G3Toggle { checked }
                G3SegmentGroup { active,
                    G3SegmentButton { index: 0, "One" }
                    G3SegmentButton { index: 1, "Two" }
                }
                G3Field {
                    label: "Name",
                    value: field_value,
                    onchange: |_| {},
                    placeholder: "Name",
                }
                G3Spinner {}
                G3InfoButton {}
                G3Line {}
            }
        }
    }

    #[component]
    fn CompositeSmokeApp() -> Element {
        let sheet_open = use_signal(|| false);
        let modal_open = use_signal(|| false);
        let select_value = use_signal(|| "One".to_string());

        rsx! {
            G3Card { title: "Card", "Body" }
            G3Card {
                G3List { inset: true,
                    G3Item { label: "Setting", metadata: "Value" }
                }
            }
            G3Card {
                title: "Inset choice",
                inset: true,
                selected: true,
                onclick: |_| {},
                "Choice body"
            }
            G3Sheet { is_open: sheet_open, "Sheet body" }
            G3SheetButton { description: "Sheet button body" }
            G3ConfirmModal { open: modal_open, title: "Confirm", on_confirm: |_| {} }
            G3Select {
                value: select_value,
                options: vec![SelectOption::from("One"), SelectOption::from(("Two", "Second"))],
                onchange: |_| {},
            }
            G3List { inset: true,
                G3Item { label: "Setting", metadata: "Value" }
                G3Item {
                    kind: ItemKind::Button,
                    label: "Action",
                    onclick: |_| {},
                }
                G3Item {
                    kind: ItemKind::Link("https://example.com".to_string()),
                    label: "Link",
                }
            }
            G3Fab {
                G3FabButton { onclick: |_| {}, "Fab" }
                G3FabList { activated: true,
                    G3FabButton { onclick: |_| {}, size: FabSize::Small, "Mini" }
                }
            }
            G3FabContainer {
                main_button: rsx! { "Open" },
                list_buttons: rsx! {
                    G3FabButton { onclick: |_| {}, size: FabSize::Small, "A" }
                },
            }
        }
    }

    #[component]
    fn LayoutSmokeApp() -> Element {
        rsx! {
            G3AppWrapper {
                G3Navbar {
                    G3Header { title: "Header" }
                    G3Body {
                        Spinner { center: true }
                    }
                }
            }
        }
    }
    #[component]
    fn MobilePrimitiveAliasSmokeApp() -> Element {
        rsx! {
            G3Badge { color: StatusColor::Accent, "Live" }
            G3Avatar { fallback: "GP" }
            G3Chip { selected: true, onclick: |_| {}, "Walking" }
            G3Progress { value: 50.0 }
            G3Skeleton { shape: SkeletonShape::Row }
        }
    }

    #[test]
    fn primitives_render() {
        render(PrimitiveSmokeApp);
    }

    #[test]
    fn composites_render() {
        render(CompositeSmokeApp);
    }

    #[test]
    fn layouts_render() {
        render(LayoutSmokeApp);
    }
    #[test]
    fn mobile_primitive_aliases_render() {
        render(MobilePrimitiveAliasSmokeApp);
    }

    #[test]
    fn playground_source_include_uses_explicit_manifest_relative_paths() {
        let descriptor_source = include_str!("descriptor.rs");

        let primitives_source = include_str!("components/primitives.rs");

        assert!(descriptor_source.contains("source: $source:literal"));
        assert!(
            descriptor_source
                .contains("include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/\", $source))")
        );
        assert!(primitives_source.contains("source: \"src/components/primitives.rs\""));
        assert!(!descriptor_source.contains("include_str!(file!())"));
    }
    #[test]
    fn mobile_primitives_are_public_and_registered() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let components_mod =
            std::fs::read_to_string(crate_root.join("src/components/mod.rs")).unwrap();
        let public_source = include_str!("lib.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("library source should have a public section");
        let prelude_source = std::fs::read_to_string(crate_root.join("src/prelude.rs")).unwrap();

        assert!(crate_root.join("src/components/primitives.rs").exists());
        assert!(
            crate_root
                .join("src/components/primitives_styles.rs")
                .exists()
        );
        assert!(components_mod.contains("mod primitives;"));
        assert!(components_mod.contains("mod primitives_styles;"));
        assert!(components_mod.contains("primitives::DESCRIPTOR"));
        for symbol in [
            "Badge",
            "Avatar",
            "Chip",
            "Progress",
            "Skeleton",
            "SkeletonShape",
            "StatusColor",
            "G3Badge",
            "G3Avatar",
            "G3Chip",
            "G3Progress",
            "G3Skeleton",
        ] {
            assert!(
                public_source.contains(symbol),
                "{symbol} missing from lib exports"
            );
            assert!(
                prelude_source.contains(symbol),
                "{symbol} missing from prelude"
            );
        }
    }

    #[test]
    fn checkbox_is_public_registered_and_accessible() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let components_mod =
            std::fs::read_to_string(crate_root.join("src/components/mod.rs")).unwrap();
        let public_source = include_str!("lib.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("library source should have a public section");
        let checkbox_source =
            std::fs::read_to_string(crate_root.join("src/components/checkbox.rs"))
                .unwrap_or_default();

        assert!(crate_root.join("src/components/checkbox.rs").exists());
        assert!(
            crate_root
                .join("src/components/checkbox_styles.rs")
                .exists()
        );
        assert!(components_mod.contains("mod checkbox;"));
        assert!(components_mod.contains("checkbox::DESCRIPTOR"));
        assert!(public_source.contains("Checkbox"));
        assert!(public_source.contains("G3Checkbox"));
        assert!(public_source.contains("ControlLabelPlacement"));
        assert!(checkbox_source.contains("role: \"checkbox\""));
        assert!(checkbox_source.contains("aria_checked"));
        assert!(checkbox_source.contains("indeterminate"));
    }

    #[test]
    fn radio_group_is_public_registered_and_accessible() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let components_mod =
            std::fs::read_to_string(crate_root.join("src/components/mod.rs")).unwrap();
        let public_source = include_str!("lib.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("library source should have a public section");
        let radio_source =
            std::fs::read_to_string(crate_root.join("src/components/radio.rs")).unwrap_or_default();

        assert!(crate_root.join("src/components/radio.rs").exists());
        assert!(crate_root.join("src/components/radio_styles.rs").exists());
        assert!(components_mod.contains("mod radio;"));
        assert!(components_mod.contains("radio::DESCRIPTOR"));
        for symbol in ["RadioGroup", "Radio", "G3RadioGroup", "G3Radio"] {
            assert!(
                public_source.contains(symbol),
                "{symbol} missing from lib exports"
            );
        }
        assert!(radio_source.contains("role: \"radiogroup\""));
        assert!(radio_source.contains("aria_disabled"));
        assert!(radio_source.contains("r#type: \"radio\""));
        assert!(radio_source.contains("name: context.name.clone()"));
        assert!(radio_source.contains("checked: selected"));
        assert!(radio_source.contains("allow_empty_selection"));
        assert!(radio_source.contains("next_radio_value"));
        assert!(!radio_source.contains("let tab_index = if is_disabled"));
        assert!(!radio_source.contains("group_has_selection"));
    }

    #[test]
    fn mobile_polish_regressions_are_guarded_in_css() {
        let stylesheet = include_str!("../assets/g3_ui.css");
        let playground_stylesheet = include_str!("../playground/assets/playground.css");
        assert!(stylesheet.contains(".g3-switch-ios.checked .g3-switch-thumb-ios"));
        assert!(stylesheet.contains("transform: translate3d(20px, -50%, 0)"));
        assert!(stylesheet.contains("right: 1rem;"));
        assert!(stylesheet.contains(".g3-list .g3-item-row:last-child .g3-item::after"));
        assert!(stylesheet.contains("backface-visibility: hidden"));
        assert!(playground_stylesheet.contains(".playground-selector-sheet .g3-sheet-content"));
        assert!(playground_stylesheet.contains("overflow-y: auto"));
        assert!(stylesheet.contains(".g3-checkbox:active:not(:disabled)"));
        assert!(stylesheet.contains("padding: 0.625rem 0.75rem"));
        assert!(stylesheet.contains("overflow: hidden"));
    }
    #[test]
    fn feedback_disclosure_and_refresh_components_are_public_registered() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let components_mod =
            std::fs::read_to_string(crate_root.join("src/components/mod.rs")).unwrap();
        let public_source = include_str!("lib.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("library source should have a public section");
        let prelude_source = std::fs::read_to_string(crate_root.join("src/prelude.rs")).unwrap();
        let stylesheet = include_str!("../assets/g3_ui.css");

        for component in ["accordion", "refresher", "toast"] {
            assert!(
                crate_root
                    .join(format!("src/components/{component}.rs"))
                    .exists()
            );
            assert!(
                crate_root
                    .join(format!("src/components/{component}_styles.rs"))
                    .exists()
            );
            assert!(components_mod.contains(&format!("mod {component};")));
            assert!(components_mod.contains(&format!("{component}::DESCRIPTOR")));
        }

        for symbol in [
            "AccordionGroup",
            "AccordionItem",
            "Refresher",
            "RefresherState",
            "Toast",
            "ToastPosition",
            "G3AccordionGroup",
            "G3AccordionItem",
            "G3Refresher",
            "G3Toast",
        ] {
            assert!(
                public_source.contains(symbol),
                "{symbol} missing from lib exports"
            );
            assert!(
                prelude_source.contains(symbol),
                "{symbol} missing from prelude"
            );
        }

        assert!(stylesheet.contains("--g3-refresher-pull"));
        let toast_source = include_str!("components/toast.rs");
        assert!(stylesheet.contains(".g3-toast"));
        assert!(stylesheet.contains(".g3-toast-close-icon"));
        assert!(stylesheet.contains(".g3-accordion-group"));
        assert!(toast_source.contains("dioxus_icons::lucide::X"));
        assert!(toast_source.contains("duration_ms.unwrap_or(3000)"));
    }
    #[test]
    fn list_family_is_public_registered_and_uses_swipe_contracts() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let components_mod =
            std::fs::read_to_string(crate_root.join("src/components/mod.rs")).unwrap();
        let public_source = include_str!("lib.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("library source should have a public section");
        let prelude_source = std::fs::read_to_string(crate_root.join("src/prelude.rs")).unwrap();
        let list_source =
            std::fs::read_to_string(crate_root.join("src/components/list.rs")).unwrap_or_default();
        let stylesheet = include_str!("../assets/g3_ui.css");

        assert!(crate_root.join("src/components/list.rs").exists());
        assert!(crate_root.join("src/components/list_styles.rs").exists());
        assert!(components_mod.contains("mod list;"));
        assert!(components_mod.contains("list::DESCRIPTOR"));
        for symbol in [
            "List",
            "Item",
            "ItemDivider",
            "SwipeItem",
            "SwipeAction",
            "ListLines",
            "ItemKind",
            "ItemDetail",
            "SwipeSide",
            "SwipeBehavior",
            "SwipeState",
            "G3List",
            "G3Item",
            "G3SwipeItem",
            "G3SwipeAction",
        ] {
            assert!(
                public_source.contains(symbol),
                "{symbol} missing from lib exports"
            );
            assert!(
                prelude_source.contains(symbol),
                "{symbol} missing from prelude"
            );
        }
        assert!(list_source.contains("elastic_swipe_offset"));
        assert!(list_source.contains("should_full_swipe"));
        assert!(list_source.contains("SwipeBehavior::Reveal"));
        assert!(list_source.contains("on_swipe_action"));
        assert!(list_source.contains("LONG_PRESS_MS"));
        assert!(stylesheet.contains("--g3-swipe-offset"));
        assert!(stylesheet.contains("--g3-swipe-progress"));
        assert!(stylesheet.contains("--g3-swipe-action-width"));
    }
    #[test]
    fn followup_mobile_polish_contracts_are_enforced() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let stylesheet = include_str!("../assets/g3_ui.css");
        let playground_stylesheet = include_str!("../playground/assets/playground.css");
        let playground_source =
            std::fs::read_to_string(crate_root.join("playground/src/main.rs")).unwrap();
        let toast_source = include_str!("components/toast.rs");
        let toast_styles = include_str!("components/toast_styles.rs");
        let accordion_source = include_str!("components/accordion.rs");
        let line_source = include_str!("components/line.rs");
        let list_source = include_str!("components/list.rs");
        let fab_source = include_str!("components/fab.rs");
        let sheet_source = include_str!("components/sheet.rs");
        let modal_source = include_str!("components/modal.rs");

        assert!(toast_source.contains("s::TIMER"));
        assert!(toast_styles.contains("g3-toast-timer"));
        assert!(stylesheet.contains(".g3-toast-timer"));
        assert!(accordion_source.contains("value: Option<Signal<Vec<String>>>"));
        assert!(accordion_source.contains("use_signal(Vec::<String>::new)"));
        assert!(!accordion_source.contains("hidden: !expanded"));
        assert!(stylesheet.contains("grid-template-rows: 0fr"));
        assert!(stylesheet.contains("grid-template-rows: 1fr"));
        assert!(stylesheet.contains(".g3-accordion-panel[data-state=\"closed\"]"));
        assert!(stylesheet.contains("user-select: none"));
        assert!(stylesheet.contains("-webkit-user-select: none"));
        assert!(stylesheet.contains(".g3-list > .g3-swipe-item:last-child .g3-item::after"));
        assert!(list_source.contains("DEFAULT_ACTIVATE_ACTION_WIDTH"));
        assert!(list_source.contains("DEFAULT_DISMISS_ACTION_WIDTH"));
        assert!(list_source.contains("action_width_for_behavior"));
        assert!(!list_source.contains("action_width: Option<f64>"));
        assert!(list_source.contains("crate::Checkbox { checked: inset"));
        assert!(list_source.contains("crate::SegmentGroup { active: lines_index"));
        assert!(stylesheet.contains(":has(+ .g3-item-divider)"));
        assert!(stylesheet.contains(".g3-swipe-item[data-behavior=\"dismiss\"] .g3-swipe-actions"));
        assert!(stylesheet.contains("transition-duration: 560ms"));
        assert!(stylesheet.contains(".g3-navbar-md .g3-navbar-tab-selected"));
        assert!(stylesheet.contains(".g3-navbar {"));
        assert!(stylesheet.contains("flex-direction: column"));
        assert!(stylesheet.contains("-webkit-tap-highlight-color: transparent"));
        assert!(stylesheet.contains(".g3-navbar-tab:active:not(:disabled)::before"));
        assert!(stylesheet.contains("transition: opacity 220ms ease-out"));
        assert!(fab_source.contains("s::FAB_CONTAINER"));
        assert!(fab_source.contains("fab: rsx!"));
        assert!(stylesheet.contains(".g3-fab-container"));
        assert!(sheet_source.contains("let has_handle = placement == SheetPlacement::Bottom"));
        assert!(stylesheet.contains("overscroll-behavior: contain"));
        assert!(stylesheet.contains(".g3-select-sheet.g3-sheet-bottom .g3-sheet-content"));
        assert!(modal_source.contains("onclick: move |_| open.set(false)"));
        assert!(stylesheet.contains(".g3-modal-overlay[data-state=\"closed\"]"));
        assert!(stylesheet.contains("pointer-events: none"));
        assert!(playground_stylesheet.contains("justify-items: center"));
        assert!(playground_stylesheet.contains("calc((100dvh - 180px) * 390 / 844)"));
        assert!(playground_stylesheet.contains("touch-action: pan-y"));
        assert!(playground_source.contains("g3_ui::List"));
        assert!(!playground_source.contains("nav-button"));
        assert!(!playground_stylesheet.contains(".nav-button"));
        assert!(playground_source.contains("g3_ui::AccordionGroup"));
        assert!(playground_source.contains("playground_demo_source"));
        assert!(!playground_source.contains("details { class: \"source-panel\""));
        assert!(stylesheet.contains("color-mix(in srgb, var(--color-label-secondary"));
        assert!(stylesheet.contains("min-width: 1px"));
        assert!(line_source.contains("crate::Checkbox"));
        assert!(line_source.contains("g3-line-demo-surface-horizontal"));
        assert!(stylesheet.contains(".g3-line-demo-surface-horizontal"));
    }

    #[test]
    fn playground_has_no_component_clone_controls() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let stylesheet = include_str!("../assets/g3_ui.css");
        let playground_stylesheet = include_str!("../playground/assets/playground.css");
        let button_source = include_str!("components/button.rs");
        let field_source = include_str!("components/field.rs");
        let toggle_source = include_str!("components/toggle.rs");
        let spinner_source = include_str!("components/spinner.rs");
        let line_source = include_str!("components/line.rs");
        let card_source = include_str!("components/card.rs");
        let sheet_source = include_str!("components/sheet.rs");
        let list_source = include_str!("components/list.rs");
        let toast_source = include_str!("components/toast.rs");
        let fab_source = include_str!("components/fab.rs");
        let body_styles_source = include_str!("components/body_styles.rs");
        let component_sources = std::fs::read_dir(crate_root.join("src/components"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
            .map(|entry| std::fs::read_to_string(entry.path()).unwrap())
            .collect::<Vec<_>>()
            .join("\n");

        for forbidden in [
            "g3-playground-control",
            "g3-playground-check",
            "g3-playground-segments",
            "g3-playground-button",
        ] {
            assert!(!component_sources.contains(forbidden), "found {forbidden}");
            assert!(
                !playground_stylesheet.contains(forbidden),
                "stylesheet still defines {forbidden}"
            );
        }

        assert!(button_source.contains("crate::Field"));
        assert!(field_source.contains("crate::Field"));
        assert!(toggle_source.contains("crate::Checkbox"));
        assert!(spinner_source.contains("crate::Checkbox"));
        assert!(line_source.contains("crate::Checkbox"));
        assert!(card_source.contains("crate::Field"));
        assert!(sheet_source.contains("crate::SegmentGroup"));
        assert!(list_source.contains("crate::SegmentGroup"));
        assert!(toast_source.contains("Duration"));
        assert!(toast_source.contains("duration_ms"));
        assert!(stylesheet.contains(".g3-toast-success"));
        assert!(toast_source.contains("let color_index = use_signal(|| 1_usize)"));
        assert!(stylesheet.contains(".g3-switch"));
        assert!(stylesheet.contains("-webkit-tap-highlight-color: transparent"));
        assert!(!stylesheet.contains(".g3-fab-ios:active"));
        assert!(!stylesheet.contains(".g3-fab-md:active"));
        assert!(stylesheet.contains(".info-btn"));
        assert!(stylesheet.contains(".info-btn:active"));
        assert!(list_source.contains("DISMISS_SWIPE_OFFSET: f64 = 430.0"));
        assert!(list_source.contains("DISMISS_EXIT_MS: u64 = 560"));
        assert!(stylesheet.contains("transition-duration: 560ms"));
        assert!(stylesheet.contains(
            ".g3-swipe-item[data-behavior=\"dismiss\"] .g3-swipe-actions-end .g3-swipe-action"
        ));
        assert!(stylesheet.contains("justify-content: flex-end"));
        assert!(sheet_source.contains("crate::List"));
        assert!(stylesheet.contains(".g3-sheet-handle-wrap-ios"));
        assert!(stylesheet.contains("min-height: 44px"));
        assert!(!stylesheet.contains(".g3-sheet.g3-sheet-closed .g3-sheet-content"));
        assert!(fab_source.contains("s::FAB_CONTAINER"));
        assert!(body_styles_source.contains("g3-body"));
        assert!(!body_styles_source.contains(" relative"));
    }
    #[test]
    fn settings_group_is_removed_from_public_surface() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let components_mod =
            std::fs::read_to_string(crate_root.join("src/components/mod.rs")).unwrap();
        let public_source = include_str!("lib.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("library source should have a public section");
        let prelude_source = std::fs::read_to_string(crate_root.join("src/prelude.rs")).unwrap();

        assert!(!crate_root.join("src/components/settings_group.rs").exists());
        assert!(
            !crate_root
                .join("src/components/settings_group_styles.rs")
                .exists()
        );
        assert!(!components_mod.contains("settings_group"));
        for symbol in [
            "SettingsGroup",
            "SettingAction",
            "SettingLink",
            "G3SettingsGroup",
        ] {
            assert!(
                !public_source.contains(symbol),
                "{symbol} should not be public"
            );
            assert!(
                !prelude_source.contains(symbol),
                "{symbol} should not be in prelude"
            );
        }
    }

    #[test]
    fn mobile_primitive_styles_use_shared_theme_tokens() {
        let stylesheet = include_str!("../assets/g3_ui.css");
        for selector in [
            ".g3-badge",
            ".g3-avatar",
            ".g3-chip",
            ".g3-progress",
            ".g3-skeleton",
        ] {
            assert!(stylesheet.contains(selector), "{selector} style missing");
        }
        assert!(stylesheet.contains("var(--color-focused)"));
        assert!(stylesheet.contains("var(--color-success)"));
        assert!(stylesheet.contains("var(--color-warning)"));
        assert!(stylesheet.contains("var(--color-danger)"));
    }
    #[test]
    fn mobile_primitives_keep_accessible_defaults() {
        let primitives_source = include_str!("components/primitives.rs");

        assert!(primitives_source.contains("unwrap_or(100.0)"));
        assert!(primitives_source.contains("let label = alt"));
        assert!(primitives_source.contains("or_else(|| fallback.clone())"));
        assert!(primitives_source.contains("unwrap_or_else(|| \"Avatar\".to_string())"));
        assert!(primitives_source.contains("aria_label: label"));
    }

    #[test]
    fn mobile_primitive_motion_respects_reduced_motion() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        assert!(stylesheet.contains("@media (prefers-reduced-motion: reduce)"));
        assert!(stylesheet.contains(".g3-progress-indeterminate .g3-progress-fill"));
        assert!(stylesheet.contains(".g3-skeleton"));
        assert!(stylesheet.contains("animation: none"));
    }
    #[test]
    fn button_styles_leave_layout_spacing_to_the_caller() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        for selector in [
            ".g3-btn-outline",
            ".g3-btn-clear",
            ".g3-btn-sm",
            ".g3-btn-md-size",
            ".g3-btn-lg",
        ] {
            let block = stylesheet
                .split(selector)
                .nth(1)
                .and_then(|rest| rest.split('}').next())
                .unwrap_or_else(|| panic!("missing {selector} style block"));
            assert!(
                !block.contains("margin:"),
                "{selector} should not add outside margins"
            );
        }

        assert!(stylesheet.contains(".g3-btn-neutral"));
        assert!(stylesheet.contains(".g3-btn-content-start"));
    }

    #[test]
    fn select_trigger_keeps_placeholder_on_one_line() {
        let stylesheet = include_str!("../assets/g3_ui.css");
        let select_block = stylesheet
            .split(".g3-select-btn")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing select button style block");

        assert!(select_block.contains("white-space: nowrap"));
        assert!(select_block.contains("line-height: 1"));
    }
    #[test]
    fn buttons_have_obvious_disabled_state() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        let disabled_block = stylesheet
            .split(".g3-btn:disabled")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing disabled button style block");

        assert!(disabled_block.contains("opacity: 0.45"));
        assert!(disabled_block.contains("cursor: not-allowed"));
        assert!(disabled_block.contains("filter: grayscale"));
        assert!(disabled_block.contains("box-shadow: none"));
        assert!(stylesheet.contains(".g3-btn:disabled:hover"));
        assert!(stylesheet.contains(".g3-btn:disabled:active"));
    }

    #[test]
    fn provider_buttons_match_google_typography_and_alignment() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        assert!(stylesheet.contains("font-family: \"G3 Provider Roboto\""));
        let provider_font = stylesheet
            .split("@font-face")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing provider font face");
        assert!(provider_font.contains("font-weight: 500"));

        let neutral_block = stylesheet
            .split(".g3-btn-neutral")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing neutral button style block");
        assert!(neutral_block.contains("background: var(--color-card)"));
        assert!(neutral_block.contains("color: var(--color-text)"));
        assert!(neutral_block.contains("border: 1px solid var(--color-card-border)"));
        assert!(!neutral_block.contains("background: white"));

        let neutral_button_block = stylesheet
            .split(".g3-btn.g3-btn-neutral")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing neutral button typography block");
        assert!(neutral_button_block.contains("font-weight: 500"));

        let content_block = stylesheet
            .split(".g3-btn-content-start")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing start-content style block");
        assert!(content_block.contains("justify-content: center"));
        assert!(content_block.contains("position: relative"));

        let start_block = stylesheet
            .split(".g3-btn-start {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing start-icon style block");
        assert!(start_block.contains("left: 0.75rem"));
        assert!(start_block.contains("position: absolute"));
    }

    #[test]
    fn layout_surfaces_have_material_contracts() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        assert!(stylesheet.contains("border-radius: 0.25rem"));
        assert!(stylesheet.contains(".g3-card-control"));
        assert!(stylesheet.contains(".g3-card-inset"));
        assert!(stylesheet.contains(".g3-list-inset"));
        assert!(stylesheet.contains(".g3-item"));
        assert!(stylesheet.contains(".g3-list-inset .g3-item:not(.g3-item-selected)"));
        assert!(!stylesheet.contains(".g3-settings-group-inset"));
    }

    #[test]
    fn component_source_tree_is_flat() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

        assert!(crate_root.join("components").is_dir());
        assert!(!crate_root.join("atoms").exists());
        assert!(!crate_root.join("molecules").exists());
        assert!(!crate_root.join("organisms").exists());
    }

    #[test]
    fn settings_card_is_not_a_public_component() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let components_mod =
            std::fs::read_to_string(crate_root.join("src/components/mod.rs")).unwrap();
        let lib_source = std::fs::read_to_string(crate_root.join("src/lib.rs")).unwrap();
        let public_source = lib_source
            .split("#[cfg(test)]")
            .next()
            .expect("library source should have a public section");

        assert!(!crate_root.join("src/components/settings_card.rs").exists());
        assert!(
            !crate_root
                .join("src/components/settings_card_styles.rs")
                .exists()
        );
        assert!(!components_mod.contains("settings_card"));
        assert!(!public_source.contains("SettingsCard"));
        assert!(!public_source.contains("G3SettingsCard"));
    }

    #[test]
    fn navbar_is_public_layout_component_and_owns_transition_base_marker() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let components_mod =
            std::fs::read_to_string(crate_root.join("src/components/mod.rs")).unwrap();
        let lib_source = std::fs::read_to_string(crate_root.join("src/lib.rs")).unwrap();
        let prelude_source = std::fs::read_to_string(crate_root.join("src/prelude.rs")).unwrap();
        let public_source = lib_source
            .split("#[cfg(test)]")
            .next()
            .expect("library source should have a public section");
        let navbar_source = std::fs::read_to_string(crate_root.join("src/components/navbar.rs"))
            .unwrap_or_default();

        assert!(crate_root.join("src/components/navbar.rs").exists());
        assert!(crate_root.join("src/components/navbar_styles.rs").exists());
        assert!(components_mod.contains("mod navbar;"));
        assert!(components_mod.contains("pub(crate) mod navbar_styles;"));
        assert!(components_mod.contains("pub use navbar::*;"));
        for symbol in [
            "Navbar",
            "NavbarTab",
            "NavbarTabBar",
            "G3Navbar",
            "G3NavbarTab",
            "G3NavbarTabBar",
        ] {
            assert!(
                public_source.contains(symbol),
                "{symbol} missing from lib exports"
            );
            assert!(
                prelude_source.contains(symbol),
                "{symbol} missing from prelude"
            );
        }
        assert!(navbar_source.contains("#[cfg(feature = \"transitions\")]"));
        let stylesheet = include_str!("../assets/g3_ui.css");
        assert!(navbar_source.contains("ROUTE_TRANSITION_BASE_CLASS"));
        assert!(navbar_source.contains("pub fn NavbarTabBar"));
        assert!(navbar_source.contains("role: \"tab\""));
        assert!(stylesheet.contains(".g3-navbar-tab-bar"));
        assert!(stylesheet.contains(".g3-navbar-tab-selected"));
    }

    #[test]
    fn app_shell_navigation_adapts_to_desktop_without_duplicate_state() {
        let stylesheet = include_str!("../assets/g3_ui.css");
        let navbar_source = include_str!("components/navbar.rs");

        assert!(stylesheet.contains("container-name: g3-app-shell"));
        assert!(stylesheet.contains("@container g3-app-shell (min-width: 48rem)"));
        assert!(stylesheet.contains("--g3-navbar-rail-width: 4rem"));
        assert!(stylesheet.contains("grid-row: 1 / -1"));
        assert!(stylesheet.contains(".g3-header .g3-header-toolbar"));
        assert!(stylesheet.contains("scrollbar-width: thin"));
        assert!(navbar_source.contains("aria_label: aria_label.unwrap_or_else"));
        assert!(navbar_source.contains("aria_label: label.clone()"));
    }
    #[test]
    fn transitions_feature_is_optional_and_drives_shell_and_body_markers() {
        let cargo = include_str!("../Cargo.toml");
        let app_wrapper_source = include_str!("components/app_wrapper.rs");
        let body_source = include_str!("components/body.rs");
        let body_styles = include_str!("components/body_styles.rs");

        assert!(cargo.contains("transitions = [\"dep:dx-route-transitions\"]"));
        assert!(cargo.contains("dx-route-transitions = { version = \"0.1.0\", optional = true }"));
        assert!(!body_styles.contains("route-transition-segment"));
        assert!(app_wrapper_source.contains("RouteTransitionProvider"));
        assert!(app_wrapper_source.contains("ROUTE_TRANSITION_COVER_CLASS"));
        assert!(body_source.contains("ROUTE_TRANSITION_SEGMENT_CLASS"));
    }
    #[test]
    fn app_wrapper_bundles_library_stylesheet() {
        let source = include_str!("components/app_wrapper.rs");

        assert!(source.contains("UI_CSS"));
        assert!(source.contains("document::Link"));
    }

    #[test]
    fn theme_defaults_are_configurable_without_mode() {
        let theme = Theme::default_light().with_focused("#22c55e");

        assert_eq!(theme.focused, "#22c55e");
        assert_eq!(theme.bg, "#f8f8f8");
    }

    #[test]
    fn app_wrapper_accepts_custom_theme_tokens() {
        let source = include_str!("components/app_wrapper.rs");

        assert!(source.contains("theme: Option<Theme>"));
        assert!(source.contains("effective_theme.to_style_attr()"));
        assert!(!source.contains("G3Theme { mode }"));
    }

    #[test]
    fn app_wrapper_falls_back_to_an_ambient_theme_for_nested_wrappers() {
        let source = include_str!("components/app_wrapper.rs");

        assert!(source.contains("try_use_context::<Theme>"));
        assert!(source.contains("layout: Option<bool>"));
    }
    #[test]
    fn focused_theme_color_drives_tint_styles() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        assert!(!stylesheet.contains("rgba(0, 122, 255"));
        assert!(stylesheet.contains("color-mix(in srgb, var(--color-focused) 8%"));
        assert!(stylesheet.contains("color-mix(in srgb, var(--color-focused) 20%"));
    }

    #[test]
    fn color_scheme_follows_theme_not_mode() {
        let stylesheet = include_str!("../assets/g3_ui.css");
        let theme_source = include_str!("theme.rs");

        // The baseline lives on :root and is emitted by every theme's inline style;
        // mode selectors must not force a scheme (that broke dark themes in iOS mode).
        let ios_mode_block = stylesheet
            .split("[data-g3-mode=\"ios\"]")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing iOS mode block");
        assert!(!ios_mode_block.contains("color-scheme"));

        let root_block = stylesheet
            .split(":root {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing :root block");
        assert!(root_block.contains("color-scheme: light"));
        assert!(theme_source.contains("color-scheme: {}"));
    }

    #[test]
    fn component_colors_reference_theme_tokens_not_literals() {
        let stylesheet = include_str!("../assets/g3_ui.css").replace("\r\n", "\n");

        // Previously hardcoded component colors that ignored custom themes.
        for literal in [
            "#e9e9eb",                   // iOS switch off-track
            "#f7f7f7",                   // iOS card pressed
            "#4b5563",                   // message-text muted
            "#15803d",                   // message-text success
            "#dc2626",                   // message-text danger
            "#b45309",                   // message-text warning
            "#374151",                   // status-pill text
            "#e5e7eb",                   // status-pill outline
            "#c7c7cc",                   // iOS sheet handle
            "rgba(255, 255, 255, 0.94)", // iOS sheet surface
            "rgba(255,255,255,0.85)",    // translucent FAB surface
        ] {
            assert!(
                !stylesheet.contains(literal),
                "{literal} should be replaced with a theme token"
            );
        }

        // Semantic message utilities now inherit the theme's semantic palette.
        assert!(
            stylesheet
                .contains(".g3-message-text-subtle {\n    color: var(--color-label-secondary);")
        );
        assert!(
            stylesheet.contains(".g3-message-text-success {\n    color: var(--color-success);")
        );
        assert!(stylesheet.contains(".g3-message-text-danger {\n    color: var(--color-danger);"));
        assert!(
            stylesheet.contains(".g3-message-text-warning {\n    color: var(--color-warning);")
        );
        // Translucent iOS surfaces adapt to the theme's card color.
        assert!(stylesheet.contains("color-mix(in srgb, var(--color-card) 94%, transparent)"));
        assert!(stylesheet.contains("color-mix(in srgb, var(--color-card) 85%, transparent)"));
    }
    #[test]
    fn segments_follow_ionic_mode_contracts() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        let ios_button_block = stylesheet
            .split(".g3-segment-btn-ios {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing iOS segment button style block");
        assert!(ios_button_block.contains("min-height: 28px"));
        assert!(ios_button_block.contains("font-size: 13px"));

        let md_button_block = stylesheet
            .split(".g3-segment-btn-md {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing MD segment button style block");
        assert!(md_button_block.contains("min-height: 48px"));
        assert!(md_button_block.contains("font-size: 14px"));
        assert!(md_button_block.contains("font-weight: 500"));
        assert!(md_button_block.contains("letter-spacing: 0.06em"));
        assert!(md_button_block.contains("text-transform: uppercase"));
        assert!(stylesheet.contains(".g3-segment-toolbar"));
        assert!(stylesheet.contains(".g3-segment-standalone"));
        assert!(stylesheet.contains(".g3-segment-route-wrapper"));
        assert!(stylesheet.contains(".g3-segment-btn-ios:focus"));
        assert!(stylesheet.contains(".g3-segment-btn-ios:focus-visible"));
        assert!(stylesheet.contains(".g3-segment-standalone .g3-segment-btn-md"));
        assert!(stylesheet.contains(".g3-segment-standalone .g3-segment-btn-ios"));
        assert!(stylesheet.contains("min-width: 0"));
        assert!(stylesheet.contains(".g3-segment-standalone.g3-segment-ios .g3-segment-btn-ios"));
        assert!(stylesheet.contains(
            "--g3-segment-ios-background, color-mix(in srgb, var(--color-text) 7%, transparent)"
        ));
        assert!(stylesheet.contains("transform 320ms cubic-bezier(0.4, 0, 0.2, 1)"));
    }

    #[test]
    fn closed_sheets_do_not_paint_offscreen_shadows() {
        let stylesheet = include_str!("../assets/g3_ui.css");
        let sheet_source = include_str!("components/sheet.rs");

        assert!(sheet_source.contains("STATE_CLOSED"));
        assert!(sheet_source.contains("let mut ever_opened = use_signal(|| false);"));
        assert!(sheet_source.contains("let mut presented_open = use_signal(|| false);"));
        assert!(sheet_source.contains("requestAnimationFrame(() => dioxus.send(true))"));
        assert!(sheet_source.contains("if !is_open_now && !ever_opened()"));
        assert!(sheet_source.contains("return rsx! {};"));
        assert!(sheet_source.contains("let visual_open = is_open_now && presented_open();"));
        assert!(stylesheet.contains(".g3-sheet.g3-sheet-closed"));
        assert!(stylesheet.contains("box-shadow: none"));
    }

    #[test]
    fn header_slots_own_top_bar_edge_spacing() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        assert!(stylesheet.contains(".g3-header-row"));
        assert!(
            stylesheet.contains("grid-template-columns: minmax(44px, 1fr) auto minmax(44px, 1fr)")
        );
        assert!(stylesheet.contains(".g3-header-start-slot"));
        assert!(stylesheet.contains(".g3-header-end-slot"));
        assert!(stylesheet.contains("padding: 0 0.5rem"));
    }

    #[test]
    fn header_toolbar_and_ios_scrollbar_contracts_are_mobile_clean() {
        let stylesheet = include_str!("../assets/g3_ui.css");
        let header_source = include_str!("components/header.rs");

        let ios_header_block = stylesheet
            .split(".g3-header-ios")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing iOS header block");
        assert!(ios_header_block.contains("background: var(--color-card)"));

        let md_toolbar_block = stylesheet
            .split(".g3-header-md .g3-header-toolbar")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing MD header toolbar block");
        assert!(md_toolbar_block.contains("padding: 0"));

        assert!(stylesheet.contains(".g3-header-ios .g3-header-slot .g3-btn"));
        assert!(stylesheet.contains(".g3-app-shell .g3-body-content"));
        assert!(stylesheet.contains(".g3-app-shell .g3-body-content::-webkit-scrollbar"));
        assert!(stylesheet.contains("scrollbar-width: none"));
        assert!(stylesheet.contains(".g3-shell-md .g3-body-content"));
        assert!(stylesheet.contains("-ms-overflow-style: none"));
        assert!(header_source.contains("HeaderToolbarContext"));
        assert!(header_source.contains("provide_context(HeaderToolbarContext)"));
        assert!(SEGMENT_SOURCE.contains("try_consume_context::<HeaderToolbarContext>()"));
        assert!(!SEGMENT_SOURCE.contains("toolbar: Option<bool>"));
        assert!(!SEGMENT_SOURCE.contains("toolbar.unwrap_or"));
    }

    #[test]
    fn body_padding_can_be_disabled_without_custom_classes() {
        let body_source = include_str!("components/body.rs");
        let body_styles = include_str!("components/body_styles.rs");
        let stylesheet = include_str!("../assets/g3_ui.css");

        assert!(body_source.contains("padding: Option<bool>"));
        assert!(body_source.contains("padding.unwrap_or(true)"));
        assert!(body_source.contains("--g3-body-padding"));
        assert!(body_source.contains("data-padding"));
        assert!(body_source.contains("BODY_CONTENT_NO_PADDING"));
        assert!(body_styles.contains("g3-body-content-no-padding"));
        assert!(stylesheet.contains(".g3-body-content-no-padding"));
        assert!(stylesheet.contains("padding: var(--g3-body-padding, 1.5rem)"));
        assert!(stylesheet.contains("--g3-body-padding: 0"));
    }

    #[test]
    fn body_loading_uses_the_centered_shared_spinner() {
        let source = include_str!("components/body.rs");

        assert!(source.contains("Spinner { center: true }"));
        assert!(!source.contains("ResourceLoading"));
    }

    #[test]
    fn resource_helpers_and_timeout_are_not_public_g3_ui_api() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let public_source = include_str!("lib.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("library source should have a public section");
        let prelude_source = std::fs::read_to_string(crate_root.join("src/prelude.rs")).unwrap();
        let util_source =
            std::fs::read_to_string(crate_root.join("src/util/mod.rs")).unwrap_or_default();

        for symbol in [
            "ResourceLoading",
            "ResourceError",
            "G3ResourceLoading",
            "G3ResourceError",
            "TimeoutError",
            "with_timeout",
        ] {
            assert!(
                !public_source.contains(symbol),
                "{symbol} is still exported"
            );
            assert!(
                !prelude_source.contains(symbol),
                "{symbol} is still in the prelude"
            );
            assert!(
                !util_source.contains(symbol),
                "{symbol} is still in util exports"
            );
        }

        assert!(!crate_root.join("src/util/resource_view.rs").exists());
        assert!(!crate_root.join("src/util/timeout.rs").exists());
    }

    #[test]
    fn spinner_has_centering_option_for_resource_fallbacks() {
        let source = include_str!("components/spinner.rs");
        let styles = include_str!("components/spinner_styles.rs");

        assert!(source.contains("center: Option<bool>"));
        assert!(source.contains("if center.unwrap_or(false)"));
        assert!(styles.contains("CENTERED"));
        assert!(styles.contains("g3-spinner-centered"));
        assert!(include_str!("../assets/g3_ui.css").contains(".g3-spinner-centered"));
    }

    #[test]
    fn sheets_hide_scrollbars_without_skipping_close_animation() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        let closed_block = stylesheet
            .split(".g3-sheet.g3-sheet-closed")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing closed sheet style block");
        assert!(closed_block.contains("visibility: hidden"));
        assert!(stylesheet.contains("visibility 0s linear var(--transition-normal)"));
        assert!(stylesheet.contains(".g3-sheet-open"));
        assert!(stylesheet.contains("transition-delay: 0s"));
        assert!(stylesheet.contains(".g3-sheet-content::-webkit-scrollbar"));
        assert!(stylesheet.contains(".g3-sheet-content"));
        assert!(stylesheet.contains("scrollbar-width: none"));
    }

    #[test]
    fn sheets_keep_side_open_selectors_distinct_from_closed_content_rules() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        assert!(stylesheet.contains(".g3-sheet-bottom.g3-sheet-open"));
        assert!(stylesheet.contains(".g3-sheet-left.g3-sheet-open,"));
        assert!(stylesheet.contains(".g3-sheet-right.g3-sheet-open"));
        assert!(!stylesheet.contains(".g3-sheet.g3-sheet-closed .g3-sheet-content"));
        assert!(!stylesheet.contains(".g3-sheet-right.g3-sheet.g3-sheet-closed .g3-sheet-content"));
        assert!(
            !stylesheet.contains(
                ".g3-sheet-left.g3-sheet-open,\n.g3-sheet-right.g3-sheet.g3-sheet-closed"
            )
        );
    }
    #[test]
    fn overlays_lock_background_scroll_while_open() {
        let stylesheet = include_str!("../assets/g3_ui.css");
        let sheet_source = include_str!("components/sheet.rs");
        let modal_source = include_str!("components/modal.rs");

        assert!(sheet_source.contains(r#"use_lock_body_scroll(is_open);"#));
        assert!(modal_source.contains(r#"use_lock_body_scroll(open);"#));
        assert!(stylesheet.contains("body.g3-overlay-scroll-locked"));
        assert!(stylesheet.contains("overscroll-behavior: none"));
    }

    #[test]
    fn select_sheet_drag_is_limited_to_the_handle() {
        let sheet_source = include_str!("components/sheet.rs");

        assert!(sheet_source.contains("draggable: Option<bool>"));
        assert!(sheet_source.contains("let is_draggable = draggable.unwrap_or(true);"));
        assert!(sheet_source.contains("if !(is_open() && is_draggable && has_handle)"));
        assert!(sheet_source.contains("const SHEET_DISMISS_DISTANCE: f64 = 96.0;"));
        assert!(sheet_source.contains("handle.dataset.g3SheetDragBound"));
        assert!(sheet_source.contains("r#type: \"button\""));
        assert!(sheet_source.contains("aria_hidden: (!is_open_now).to_string()"));
        assert!(sheet_source.contains("inert: (!is_open_now).then"));
        assert!(include_str!("components/select.rs").contains("draggable: false"));
    }

    #[test]
    fn ios_segment_buttons_do_not_use_sibling_border_separators_or_press_flash() {
        let stylesheet = include_str!("../assets/g3_ui.css").replace("\r\n", "\n");

        assert!(!stylesheet.contains(".g3-segment-btn-ios + .g3-segment-btn-ios"));
        assert!(!stylesheet.contains(".g3-segment-btn-ios:active"));
        assert!(stylesheet.contains(".g3-segment-btn-ios {\n"));
        assert!(stylesheet.contains("-webkit-tap-highlight-color: transparent"));
    }

    #[test]
    fn playground_menu_button_suppresses_global_button_press_fill() {
        let stylesheet = include_str!("../playground/assets/playground.css");

        assert!(stylesheet.contains(".playground-menu-button:active"));
        assert!(stylesheet.contains("background: transparent"));
        assert!(stylesheet.contains("-webkit-tap-highlight-color: transparent"));
    }

    #[test]
    fn toast_color_is_shown_by_a_status_dot_and_surface_tint() {
        let stylesheet = include_str!("../assets/g3_ui.css");
        let toast_source = include_str!("components/toast.rs");

        // The colour signal is a leading status dot plus a faint surface tint —
        // not the old coloured outline / left accent bar. Matches the card
        // treatment shared by items, cards and the accordion.
        assert!(
            !stylesheet.contains("border-inline-start: 6px solid var(--g3-toast-accent-color)")
        );
        assert!(toast_source.contains("s::INDICATOR"));
        assert!(stylesheet.contains(".g3-toast-indicator"));
        assert!(stylesheet.contains("background: var(--g3-toast-accent-color)"));
        assert!(stylesheet.contains("--g3-toast-surface: color-mix(in srgb, var(--color-success)"));
    }

    #[test]
    fn toast_has_distinct_ios_and_md_treatments() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        let ios = stylesheet
            .split(".g3-toast-ios {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing .g3-toast-ios block");
        let md = stylesheet
            .split(".g3-toast-md {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing .g3-toast-md block");

        // iOS is a frosted, translucent pill; MD is an opaque elevated slab.
        assert!(ios.contains("backdrop-filter"));
        assert!(ios.contains("border-radius: 0.875rem"));
        assert!(!md.contains("backdrop-filter"));
        assert!(md.contains("border-radius: 4px"));
        assert!(md.contains("border: 0"));
    }

    #[test]
    fn checkbox_single_line_rows_center_label_and_control() {
        let checkbox_source = include_str!("components/checkbox.rs");
        let stylesheet = include_str!("../assets/g3_ui.css");

        let control_block = stylesheet
            .split(".g3-checkbox-control {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing checkbox control block");

        assert!(checkbox_source.contains("g3-checkbox-single-line"));
        assert!(stylesheet.contains(".g3-checkbox-single-line"));
        assert!(stylesheet.contains("align-items: center"));
        assert!(stylesheet.contains(".g3-checkbox-single-line.g3-control-label-start"));
        assert!(stylesheet.contains("row-gap: 0"));
        assert!(stylesheet.contains("min-height: 22px"));
        assert!(control_block.contains("background: var(--color-card)"));
        assert!(!control_block.contains("background: var(--color-control)"));
        assert!(stylesheet.contains(".g3-checkbox-single-line .g3-checkbox-label {"));
        assert!(stylesheet.contains("min-height: 22px;"));
        assert!(stylesheet.contains("line-height: 22px;"));
        assert!(stylesheet.contains("transform: translateY(1px);"));
    }

    #[test]
    fn confirm_modal_playground_controls_use_shared_fields_only() {
        let source = include_str!("components/confirm_modal.rs");

        assert!(source.contains("crate::Field"));
        assert!(!source.contains("g3-playground-control"));
        assert!(!source.contains("g3-playground-input"));
        assert!(!source.contains("g3-playground-field"));
    }

    #[test]
    fn sheets_support_bottom_and_side_placements_without_changing_default() {
        let sheet_source = include_str!("components/sheet.rs");
        let sheet_styles = include_str!("components/sheet_styles.rs");
        let stylesheet = include_str!("../assets/g3_ui.css");
        let public_source = include_str!("lib.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("library source should have a public section");

        assert!(public_source.contains("SheetPlacement"));
        assert!(sheet_source.contains("pub enum SheetPlacement"));
        assert!(sheet_source.contains("placement: Option<SheetPlacement>"));
        assert!(sheet_source.contains("unwrap_or_default()"));
        assert!(sheet_styles.contains("SHEET_BOTTOM"));
        assert!(sheet_styles.contains("SHEET_LEFT"));
        assert!(sheet_styles.contains("SHEET_RIGHT"));
        assert!(stylesheet.contains(".g3-sheet-bottom"));
        assert!(stylesheet.contains(".g3-sheet-left"));
        assert!(stylesheet.contains(".g3-sheet-right"));
        assert!(stylesheet.contains(".g3-sheet-handle-wrap-ios"));
    }

    #[test]
    fn route_sheet_page_is_not_public_component_api() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let public_source = include_str!("lib.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("library source should have a public section");
        let prelude_source = std::fs::read_to_string(crate_root.join("src/prelude.rs")).unwrap();
        let descriptor_source = include_str!("descriptor.rs");

        assert!(!public_source.contains("SheetPage"));
        assert!(!public_source.contains("G3SheetPage"));
        assert!(!public_source.contains("RouteSheetSurface"));
        assert!(!public_source.contains("G3RouteSheetSurface"));
        assert!(!prelude_source.contains("SheetPage"));
        assert!(!prelude_source.contains("G3SheetPage"));
        assert!(!prelude_source.contains("RouteSheetSurface"));
        assert!(!prelude_source.contains("G3RouteSheetSurface"));
        assert!(!descriptor_source.contains("G3SheetPage"));
        assert!(!descriptor_source.contains("RouteSheetSurface"));
        assert!(!crate_root.join("src/components/sheet_page.rs").exists());
        assert!(
            !crate_root
                .join("src/components/sheet_page_styles.rs")
                .exists()
        );
    }

    #[test]
    fn sheet_backdrops_can_cover_the_full_app_shell() {
        let stylesheet = include_str!("../assets/g3_ui.css");
        let body_block = stylesheet
            .split(".g3-body {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing body style block");
        let backdrop_block = stylesheet
            .split(".g3-sheet-backdrop")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing sheet backdrop style block");

        assert!(!body_block.contains("isolation: isolate"));
        assert!(backdrop_block.contains("position: fixed"));
        assert!(backdrop_block.contains("inset: 0"));
        assert!(backdrop_block.contains("z-index"));
    }

    #[test]
    fn shared_message_text_styles_are_component_owned() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        assert!(stylesheet.contains(".g3-message-text"));
        assert!(stylesheet.contains(".g3-message-text-success"));
        assert!(stylesheet.contains(".g3-message-text-danger"));
        assert!(stylesheet.contains(".g3-message-text-warning"));
    }
    #[test]
    fn playground_uses_component_owned_demos_instead_of_auto_specs() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let playground_root = crate_root.join("playground/src");
        let playground_main = std::fs::read_to_string(playground_root.join("main.rs")).unwrap();
        let components_mod = include_str!("components/mod.rs");
        let button_source = include_str!("components/button.rs");

        assert!(
            !playground_root
                .join("playground_gen/spec_generator.rs")
                .exists()
        );
        assert!(!playground_main.contains("auto_spec"));
        assert!(!playground_main.contains("auto_specs"));
        assert!(playground_main.contains("component_playground_demos"));
        assert!(components_mod.contains("button::PLAYGROUND"));
        let descriptor_source = include_str!("descriptor.rs");
        assert!(button_source.contains("crate::g3_playground!"));
        assert!(button_source.contains("ButtonPlaygroundDemo"));
        assert!(button_source.contains("crate::PlaygroundDemoFrame"));
        assert!(descriptor_source.contains("pub fn PlaygroundDemoFrame"));
        assert!(descriptor_source.contains("rsx! { $demo {} }"));
        assert!(!descriptor_source.contains("$demo()"));
        assert!(!playground_main.contains("PhoneFrame { {rendered_demo} }"));
        assert!(!components_mod.contains("fn render_button_demo"));
        assert!(!components_mod.contains("ComponentCategory"));
        assert!(!playground_main.contains("CategoryNav"));
        assert!(!playground_main.contains("category"));
        assert!(!playground_main.contains("gallery"));
        assert!(!playground_main.contains("Gallery"));
    }
    #[test]
    fn segment_panel_is_removed_from_public_surface() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let lib_source = include_str!("lib.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("library source should have a public section");
        let prelude_source = std::fs::read_to_string(crate_root.join("src/prelude.rs")).unwrap();

        assert!(!SEGMENT_SOURCE.contains("pub fn SegmentPanel"));
        assert!(!SEGMENT_SOURCE.contains("render: Callback"));
        assert!(!SEGMENT_SOURCE.contains("render.call"));
        assert!(!lib_source.contains("SegmentPanel"));
        assert!(!lib_source.contains("G3SegmentPanel"));
        assert!(!prelude_source.contains("SegmentPanel"));
        assert!(!prelude_source.contains("G3SegmentPanel"));
    }

    #[test]
    fn segment_group_is_not_route_or_transition_aware() {
        assert!(!SEGMENT_SOURCE.contains("SegmentRouteTarget"));
        assert!(!SEGMENT_SOURCE.contains("HashMap"));
        assert!(!SEGMENT_SOURCE.contains("animated_update"));
        assert!(!SEGMENT_SOURCE.contains("NavigationAnimation"));
        assert!(!SEGMENT_SOURCE.contains("navigator.push"));
        assert!(!SEGMENT_SOURCE.contains("route: Option"));
    }
    #[test]
    fn segment_group_can_defer_active_for_animated_callers() {
        assert!(SEGMENT_SOURCE.contains("defer_active: Option<bool>"));
        assert!(SEGMENT_SOURCE.contains("defer_active: defer_active.unwrap_or(false)"));
        assert!(SEGMENT_SOURCE.contains("if !context.defer_active"));
        assert!(SEGMENT_SOURCE.contains("(context.active).set(index)"));
    }

    #[test]
    fn segments_expose_indicator_and_child_view_contracts() {
        let stylesheet = include_str!("../assets/g3_ui.css");
        let segment_styles = include_str!("components/segment_styles.rs");

        assert!(SEGMENT_SOURCE.contains("\"data-active\""));
        assert!(stylesheet.contains(".g3-segment-md::after"));
        assert!(stylesheet.contains(".g3-segment-ios::before"));
        assert!(stylesheet.contains("--g3-segment-count"));
        assert!(stylesheet.contains("--g3-segment-active"));
        assert!(
            stylesheet.contains("transform: translateX(calc(var(--g3-segment-active, 0) * 100%))")
        );
        assert!(!SEGMENT_SOURCE.contains("s::VIEWPORT"));
        assert!(!segment_styles.contains("VIEWPORT"));
        assert!(!stylesheet.contains(".g3-segment-viewport"));
        assert!(!stylesheet.contains(".g3-segment-view"));
        assert!(!stylesheet.contains("data-g3-segment-animation"));
        assert!(!stylesheet.contains("@keyframes g3-segment-in-left"));
        assert!(!stylesheet.contains("@keyframes g3-segment-in-right"));
        assert!(!stylesheet.contains("g3-segment-slide-left"));
        assert!(!stylesheet.contains("g3-segment-slide-right"));
    }

    #[test]
    fn segment_on_change_observes_previous_active_index() {
        let callback_index = SEGMENT_SOURCE
            .find("on_change.call(index)")
            .expect("missing segment on_change callback");
        let active_set_index = SEGMENT_SOURCE
            .find("(context.active).set(index)")
            .expect("missing segment active mutation");

        assert!(
            callback_index < active_set_index,
            "segment on_change must run before active changes so callers can compute slide direction"
        );
    }

    #[test]
    fn segment_group_does_not_own_child_panel_layout() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        assert!(!stylesheet.contains(".g3-segment-viewport"));
        assert!(!stylesheet.contains(".g3-segment-view"));
        assert!(!stylesheet.contains(".g3-segment-view-exiting"));
        assert!(!SEGMENT_SOURCE.contains("{children}\n            }\n        }\n    }\n}"));
    }

    #[test]
    fn modal_card_owns_fixed_centering_and_exit_motion() {
        let stylesheet = include_str!("../assets/g3_ui.css");
        let modal_source = include_str!("components/modal.rs");

        let card_block = stylesheet
            .split(".g3-modal-card")
            .nth(1)
            .expect("missing modal card style block")
            .split('}')
            .next()
            .expect("missing modal card declaration block");

        assert!(card_block.contains("position: fixed"));
        assert!(card_block.contains("top: 50%"));
        assert!(card_block.contains("left: 50%"));
        assert!(stylesheet.contains(".g3-modal[data-state=\"closed\"]"));
        assert!(stylesheet.contains("@keyframes g3-modal-out"));
        assert!(!modal_source.contains("top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2"));
    }

    #[test]
    fn modal_close_motion_follows_overlay_closed_state() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        assert!(stylesheet.contains(".g3-modal-overlay[data-state=\"closed\"] .g3-modal"));
        assert!(stylesheet.contains("g3-modal-out var(--g3-modal-duration)"));
        assert!(stylesheet.contains("--g3-modal-duration: 220ms"));
        assert!(!stylesheet.contains("--g3-modal-debug-duration: 600ms"));
        assert!(stylesheet.contains("calc(-50% + 2rem)"));
    }

    #[test]
    fn segment_panel_edge_clipping_styles_are_removed() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        assert!(!stylesheet.contains(".g3-body-content > .g3-segment-viewport"));
        assert!(!stylesheet.contains("width: calc(100% + 3rem)"));
        assert!(!stylesheet.contains("width: calc(100% + 20rem)"));
        assert!(!stylesheet.contains("width: calc(100% + 40rem)"));
    }

    #[test]
    fn segments_use_custom_properties_for_animated_any_count_indicators() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        assert!(!SEGMENT_SOURCE.contains("count: Option<usize>"));
        assert!(SEGMENT_SOURCE.contains("count_segment_children"));
        assert!(SEGMENT_SOURCE.contains("count_dynamic_components"));
        assert!(SEGMENT_SOURCE.contains("DynamicNode::Fragment"));
        assert!(SEGMENT_SOURCE.contains("--g3-segment-count"));
        assert!(SEGMENT_SOURCE.contains("--g3-segment-active"));
        assert!(stylesheet.contains("width: calc(100% / var(--g3-segment-count, 3))"));
        assert!(stylesheet.contains("width: calc((100% - 8px) / var(--g3-segment-count, 3))"));

        assert!(
            stylesheet.contains("transform: translateX(calc(var(--g3-segment-active, 0) * 100%))")
        );
        assert!(!stylesheet.contains(":nth-child(5):last-child"));
        assert!(!stylesheet.contains("translateX(400%)"));
    }

    #[test]
    fn segment_demo_shows_toolbar_context_and_body_card() {
        assert!(SEGMENT_SOURCE.contains("crate::Header"));
        assert!(SEGMENT_SOURCE.contains("toolbar: rsx!"));
        assert!(SEGMENT_SOURCE.contains("Body {"));
        assert!(SEGMENT_SOURCE.contains("crate::Card { title: \"Standalone\""));
        assert!(!SEGMENT_SOURCE.contains("label: \"Toolbar\""));
    }

    #[test]
    fn swipe_items_draw_parent_list_dividers_between_swipe_rows() {
        let stylesheet = include_str!("../assets/g3_ui.css");
        let swipe_content_block = stylesheet
            .split(".g3-swipe-content {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing swipe content style block");

        assert!(stylesheet.contains(".g3-list[data-lines=\"inset\"] > .g3-swipe-item"));
        assert!(stylesheet.contains(".g3-list[data-lines=\"full\"] > .g3-swipe-item"));
        assert!(stylesheet.contains(".g3-list[data-lines=\"none\"] > .g3-swipe-item::after"));
        assert!(stylesheet.contains("pointer-events: none"));
        assert!(swipe_content_block.contains("background: var(--color-card)"));
    }

    #[test]
    fn grouped_surfaces_share_ionic_platform_elevation_contract() {
        let stylesheet = include_str!("../assets/g3_ui.css");

        for (selector, label) in [
            (".g3-card-ios {", "iOS card"),
            (".g3-list-ios.g3-list-inset {", "iOS inset list"),
            (".g3-accordion-group-ios {", "iOS accordion group"),
        ] {
            let block = stylesheet
                .split(selector)
                .nth(1)
                .unwrap_or_else(|| panic!("missing {label} style block"))
                .split('}')
                .next()
                .unwrap_or_else(|| panic!("missing {label} declaration block"));

            assert!(
                block.contains("border-radius: 8px"),
                "{label} radius differs from card"
            );
            assert!(
                block.contains("box-shadow: var(--g3-shadow-ios-card)"),
                "{label} shadow differs from card"
            );
            assert!(
                block.contains("border: 0"),
                "{label} should not use a colored border"
            );
        }

        for (selector, label) in [
            (".g3-card-md {", "MD card"),
            (".g3-list-md.g3-list-inset {", "MD inset list"),
            (".g3-accordion-group-md {", "MD accordion group"),
        ] {
            let block = stylesheet
                .split(selector)
                .nth(1)
                .unwrap_or_else(|| panic!("missing {label} style block"))
                .split('}')
                .next()
                .unwrap_or_else(|| panic!("missing {label} declaration block"));

            assert!(
                block.contains("border-radius: 4px"),
                "{label} radius differs from card"
            );
            assert!(
                block.contains("box-shadow: var(--g3-shadow-md-elevation-1)"),
                "{label} shadow differs from card"
            );
        }
    }
    #[test]
    fn timing_uses_dioxus_sdk_time_instead_of_custom_target_split() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let cargo = std::fs::read_to_string(crate_root.join("Cargo.toml")).unwrap();
        let field_source = include_str!("components/field.rs");

        assert!(cargo.contains("dioxus-sdk-time"));
        assert!(field_source.contains("dioxus_sdk_time::sleep"));
        assert!(!field_source.contains("gloo_timers"));
        assert!(!field_source.contains("tokio::time"));
    }
}
