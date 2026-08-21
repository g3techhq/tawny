//! General mobile list, item, and swipe row components.

use super::list_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
use dioxus_icons::lucide::ChevronRight;
use std::time::Duration;

/// Which edge of a list row a swipe gesture belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwipeSide {
    /// The leading edge - swiping from it drags content toward the trailing side.
    Start,
    /// The trailing edge - the conventional side for destructive actions.
    End,
}

/// What a swipe does once it passes the commit threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SwipeBehavior {
    /// Swiping uncovers the actions and holds them open until one is tapped
    /// or the row is swiped closed.
    #[default]
    Reveal,
    /// Swiping past the threshold fires the leading action directly, without
    /// leaving the actions on screen.
    Activate,
    /// Swiping past the threshold removes the row entirely.
    Dismiss,
}

pub const DEFAULT_SWIPE_ACTION_WIDTH: f64 = 88.0;
pub const DEFAULT_ACTIVATE_ACTION_WIDTH: f64 = 136.0;
pub const DEFAULT_DISMISS_ACTION_WIDTH: f64 = 104.0;
pub const FULL_SWIPE_MARGIN: f64 = 30.0;
pub const ELASTIC_FACTOR: f64 = 0.55;
pub const ACTIVATE_SWIPE_RATIO: f64 = 0.48;
pub const ACTIVATE_SOFTENING_RATIO: f64 = 0.72;
pub const DISMISS_SWIPE_OFFSET: f64 = 430.0;
pub const DISMISS_EXIT_MS: u64 = 560;
pub const DISMISS_COLLAPSE_MS: u64 = 180;
pub const LONG_PRESS_MS: u64 = 500;
pub const LONG_PRESS_CANCEL_DISTANCE: f64 = 8.0;

pub fn elastic_swipe_offset(raw_offset: f64, action_width: f64) -> f64 {
    let limit = action_width.max(1.0);
    if raw_offset > limit {
        limit + (raw_offset - limit) * ELASTIC_FACTOR
    } else if raw_offset < -limit {
        -limit + (raw_offset + limit) * ELASTIC_FACTOR
    } else {
        raw_offset
    }
}

pub fn should_full_swipe(offset: f64, action_width: f64) -> bool {
    offset.abs() >= action_width.max(1.0) + FULL_SWIPE_MARGIN
}

pub fn swipe_side(offset: f64) -> Option<SwipeSide> {
    if offset > 0.0 {
        Some(SwipeSide::Start)
    } else if offset < 0.0 {
        Some(SwipeSide::End)
    } else {
        None
    }
}

pub fn swipe_ratio(offset: f64, action_width: f64) -> f64 {
    offset / action_width.max(1.0)
}

pub fn should_cancel_long_press(delta_x: f64, delta_y: f64) -> bool {
    delta_x.hypot(delta_y) > LONG_PRESS_CANCEL_DISTANCE
}

pub fn is_swipe_side_available(
    offset: f64,
    has_start_actions: bool,
    has_end_actions: bool,
) -> bool {
    match swipe_side(offset) {
        Some(SwipeSide::Start) => has_start_actions,
        Some(SwipeSide::End) => has_end_actions,
        None => false,
    }
}

pub fn reveal_swipe_offset(raw_offset: f64, action_width: f64) -> f64 {
    raw_offset.clamp(-action_width.max(1.0), action_width.max(1.0))
}

pub fn activate_swipe_offset(raw_offset: f64, action_width: f64) -> f64 {
    let limit = action_width.max(1.0);
    let sign = raw_offset.signum();
    let distance = raw_offset.abs();
    let soften_start = limit * ACTIVATE_SOFTENING_RATIO;
    if distance <= soften_start {
        raw_offset
    } else {
        let extra = distance - soften_start;
        let remaining = (limit - soften_start).max(1.0);
        sign * (soften_start + remaining * (extra / (extra + remaining)))
    }
}

pub fn should_activate_swipe(offset: f64, action_width: f64) -> bool {
    offset.abs() >= action_width.max(1.0) * ACTIVATE_SWIPE_RATIO
}

pub fn swipe_offset_for_behavior(
    raw_offset: f64,
    action_width: f64,
    has_start_actions: bool,
    has_end_actions: bool,
    behavior: SwipeBehavior,
) -> f64 {
    if !is_swipe_side_available(raw_offset, has_start_actions, has_end_actions) {
        return 0.0;
    }
    match behavior {
        SwipeBehavior::Reveal => reveal_swipe_offset(raw_offset, action_width),
        SwipeBehavior::Activate => activate_swipe_offset(raw_offset, action_width),
        SwipeBehavior::Dismiss => elastic_swipe_offset(raw_offset, action_width),
    }
}

pub fn action_width_for_behavior(behavior: SwipeBehavior) -> f64 {
    match behavior {
        SwipeBehavior::Reveal => DEFAULT_SWIPE_ACTION_WIDTH,
        SwipeBehavior::Activate => DEFAULT_ACTIVATE_ACTION_WIDTH,
        SwipeBehavior::Dismiss => DEFAULT_DISMISS_ACTION_WIDTH,
    }
}

#[allow(dead_code)]
pub fn constrained_swipe_offset(
    raw_offset: f64,
    action_width: f64,
    has_start_actions: bool,
    has_end_actions: bool,
) -> f64 {
    swipe_offset_for_behavior(
        raw_offset,
        action_width,
        has_start_actions,
        has_end_actions,
        SwipeBehavior::Dismiss,
    )
}

fn swipe_state(offset: f64, action_width: f64, full: bool) -> Option<SwipeState> {
    swipe_side(offset).map(|side| SwipeState {
        side,
        offset,
        ratio: swipe_ratio(offset, action_width),
        full,
    })
}

fn settled_swipe_offset(offset: f64, action_width: f64) -> f64 {
    if offset.abs() > action_width.max(1.0) / 2.0 {
        match swipe_side(offset) {
            Some(SwipeSide::Start) => action_width.max(1.0),
            Some(SwipeSide::End) => -action_width.max(1.0),
            None => 0.0,
        }
    } else {
        0.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DismissPhase {
    Idle,
    Exiting,
    Collapsing,
}

impl DismissPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Exiting => "exiting",
            Self::Collapsing => "collapsing",
        }
    }
}

/// How separators are drawn between list rows.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ListLines {
    /// Separators span the full width of the list.
    Full,
    /// Separators are inset to align with row text, leaving leading icons
    /// and avatars clear.
    #[default]
    Inset,
    /// No separators.
    None,
}

impl ListLines {
    fn class(self) -> &'static str {
        match self {
            Self::Full => s::ITEM_LINES_FULL,
            Self::Inset => s::ITEM_LINES_INSET,
            Self::None => s::ITEM_LINES_NONE,
        }
    }
}

/// What a list row behaves as, which determines its semantics and
/// keyboard handling as well as its look.
#[derive(Clone, PartialEq, Eq, Default)]
pub enum ItemKind {
    /// Plain content. Not focusable and not interactive.
    #[default]
    Static,
    /// Behaves as a button: focusable, keyboard-activatable, and it shows
    /// a press state.
    Button,
    /// Navigates to the given href. Renders as an anchor, so it supports
    /// middle-click and open-in-new-tab.
    Link(String),
}

/// Whether a list row shows a trailing detail chevron.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemDetail {
    /// Show the chevron when the row is interactive, hide it otherwise.
    #[default]
    Auto,
    /// Always show the trailing chevron.
    Show,
    /// Never show the trailing chevron.
    Hide,
}

/// Live state of an in-progress swipe, handed to swipe-action render
/// callbacks so they can track the gesture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwipeState {
    /// Which edge the gesture started from.
    pub side: SwipeSide,
    /// Current horizontal displacement of the row, in pixels.
    pub offset: f64,
    /// `offset` as a fraction of the width of the revealed actions, so `1.0`
    /// means the actions are fully open.
    pub ratio: f64,
    /// Whether the swipe has passed the threshold at which `Activate` or
    /// `Dismiss` would commit on release.
    pub full: bool,
}

#[component]
pub fn List(
    inset: Option<bool>,
    lines: Option<ListLines>,
    class: Option<String>,
    mode: Option<ComponentMode>,
    children: Element,
) -> Element {
    let mode = use_component_mode(mode);
    let mode_cls = match mode {
        ComponentMode::Ios => s::LIST_IOS,
        ComponentMode::Md => s::LIST_MD,
    };
    let inset_cls = if inset.unwrap_or(false) {
        s::LIST_INSET
    } else {
        ""
    };
    let lines = match lines.unwrap_or_default() {
        ListLines::Full => "full",
        ListLines::Inset => "inset",
        ListLines::None => "none",
    };
    rsx! {
        div {
            class: merge_classes(format!("{} {mode_cls} {inset_cls}", s::LIST), class.as_deref()),
            role: "list",
            "data-lines": lines,
            {children}
        }
    }
}

#[component]
pub fn Item(
    kind: Option<ItemKind>,
    lines: Option<ListLines>,
    selected: Option<bool>,
    disabled: Option<bool>,
    detail: Option<ItemDetail>,
    start: Option<Element>,
    end: Option<Element>,
    /// When `Some`, renders a checkbox-style indicator in the end slot
    /// (taking over from `end`/`detail`) instead of a custom `end` element
    /// or chevron — the row itself is the toggle, driven by the caller's
    /// `onclick`, rather than nesting a second interactive control inside
    /// the row (which `Checkbox` can't do without becoming a button inside
    /// a button when the row also needs to be tappable).
    checked: Option<bool>,
    overline: Option<String>,
    label: Option<String>,
    description: Option<String>,
    metadata: Option<String>,
    class: Option<String>,
    mode: Option<ComponentMode>,
    onclick: Option<Callback<Event<MouseData>>>,
    children: Option<Element>,
) -> Element {
    let mode = use_component_mode(mode);
    let kind = match (kind.unwrap_or_default(), onclick.is_some()) {
        (ItemKind::Static, true) => ItemKind::Button,
        (kind, _) => kind,
    };
    let disabled = disabled.unwrap_or(false);
    let selected = selected.unwrap_or(false);
    let detail = detail.unwrap_or_default();
    let mode_cls = match mode {
        ComponentMode::Ios => s::ITEM_IOS,
        ComponentMode::Md => s::ITEM_MD,
    };
    let interactive = !matches!(kind, ItemKind::Static) || onclick.is_some();
    let show_detail = checked.is_none()
        && (matches!(detail, ItemDetail::Show)
            || (matches!(detail, ItemDetail::Auto) && interactive && mode == ComponentMode::Ios));
    let cls = merge_classes(
        format!(
            "{} {mode_cls} {} {} {} {}",
            s::ITEM,
            lines.map(ListLines::class).unwrap_or(""),
            if interactive { s::ITEM_BUTTON } else { "" },
            if selected { s::ITEM_SELECTED } else { "" },
            if disabled { s::ITEM_DISABLED } else { "" }
        ),
        class.as_deref(),
    );
    let aria_disabled = disabled.then(|| "true".to_string());
    let role = checked.map(|_| "checkbox");
    let aria_checked = checked.map(|value| value.to_string());
    let end = if let Some(checked) = checked {
        Some(rsx! {
            span {
                class: if checked { "g3-item-check checked" } else { "g3-item-check" },
                aria_hidden: "true",
                span { class: "g3-item-check-mark" }
            }
        })
    } else {
        end
    };
    match kind {
        ItemKind::Link(href) if !disabled => {
            rsx! {
                div { class: s::ITEM_ROW, role: "listitem",
                    a { class: cls, href,
                        if let Some(start) = start {
                            span { class: s::ITEM_START, {start} }
                        }
                        span { class: s::ITEM_MAIN,
                            if let Some(overline) = overline {
                                span { class: s::ITEM_OVERLINE, "{overline}" }
                            }
                            if let Some(label) = label {
                                span { class: s::ITEM_LABEL, "{label}" }
                            }
                            if let Some(description) = description {
                                span { class: s::ITEM_DESCRIPTION, "{description}" }
                            }
                            if let Some(children) = children {
                                {children}
                            }
                        }
                        if let Some(metadata) = metadata {
                            span { class: s::ITEM_METADATA, "{metadata}" }
                        }
                        if let Some(end) = end {
                            span { class: s::ITEM_END, {end} }
                        }
                        if show_detail {
                            span { aria_hidden: "true",
                                ChevronRight { class: s::ITEM_DETAIL, size: 18 }
                            }
                        }
                    }
                }
            }
        }
        ItemKind::Link(_) => {
            rsx! {
                div { class: s::ITEM_ROW, role: "listitem",
                    div { class: cls, role: "link", aria_disabled,
                        if let Some(start) = start {
                            span { class: s::ITEM_START, {start} }
                        }
                        span { class: s::ITEM_MAIN,
                            if let Some(overline) = overline {
                                span { class: s::ITEM_OVERLINE, "{overline}" }
                            }
                            if let Some(label) = label {
                                span { class: s::ITEM_LABEL, "{label}" }
                            }
                            if let Some(description) = description {
                                span { class: s::ITEM_DESCRIPTION, "{description}" }
                            }
                            if let Some(children) = children {
                                {children}
                            }
                        }
                        if let Some(metadata) = metadata {
                            span { class: s::ITEM_METADATA, "{metadata}" }
                        }
                        if let Some(end) = end {
                            span { class: s::ITEM_END, {end} }
                        }
                        if show_detail {
                            span { aria_hidden: "true",
                                ChevronRight { class: s::ITEM_DETAIL, size: 18 }
                            }
                        }
                    }
                }
            }
        }
        ItemKind::Button => {
            rsx! {
                div { class: s::ITEM_ROW, role: "listitem",
                    button {
                        class: cls,
                        r#type: "button",
                        disabled,
                        role,
                        aria_checked,
                        onclick: move |event| {
                            if let Some(onclick) = onclick {
                                onclick.call(event);
                            }
                        },
                        if let Some(start) = start {
                            span { class: s::ITEM_START, {start} }
                        }
                        span { class: s::ITEM_MAIN,
                            if let Some(overline) = overline {
                                span { class: s::ITEM_OVERLINE, "{overline}" }
                            }
                            if let Some(label) = label {
                                span { class: s::ITEM_LABEL, "{label}" }
                            }
                            if let Some(description) = description {
                                span { class: s::ITEM_DESCRIPTION, "{description}" }
                            }
                            if let Some(children) = children {
                                {children}
                            }
                        }
                        if let Some(metadata) = metadata {
                            span { class: s::ITEM_METADATA, "{metadata}" }
                        }
                        if let Some(end) = end {
                            span { class: s::ITEM_END, {end} }
                        }
                        if show_detail {
                            span { aria_hidden: "true",
                                ChevronRight { class: s::ITEM_DETAIL, size: 18 }
                            }
                        }
                    }
                }
            }
        }
        ItemKind::Static => {
            rsx! {
                div { class: s::ITEM_ROW, role: "listitem",
                    div { class: cls,
                        if let Some(start) = start {
                            span { class: s::ITEM_START, {start} }
                        }
                        span { class: s::ITEM_MAIN,
                            if let Some(overline) = overline {
                                span { class: s::ITEM_OVERLINE, "{overline}" }
                            }
                            if let Some(label) = label {
                                span { class: s::ITEM_LABEL, "{label}" }
                            }
                            if let Some(description) = description {
                                span { class: s::ITEM_DESCRIPTION, "{description}" }
                            }
                            if let Some(children) = children {
                                {children}
                            }
                        }
                        if let Some(metadata) = metadata {
                            span { class: s::ITEM_METADATA, "{metadata}" }
                        }
                        if let Some(end) = end {
                            span { class: s::ITEM_END, {end} }
                        }
                        if show_detail {
                            span { aria_hidden: "true",
                                ChevronRight { class: s::ITEM_DETAIL, size: 18 }
                            }
                        }
                    }
                }
            }
        }
    }
}
#[component]
pub fn ItemDivider(class: Option<String>, children: Element) -> Element {
    rsx! {
        div {
            class: merge_classes(s::ITEM_DIVIDER, class.as_deref()),
            role: "separator",
            {children}
        }
    }
}

#[component]
pub fn SwipeAction(
    side: SwipeSide,
    destructive: Option<bool>,
    accent: Option<bool>,
    class: Option<String>,
    onclick: Option<Callback<Event<MouseData>>>,
    children: Element,
) -> Element {
    let variant = if destructive.unwrap_or(false) {
        s::SWIPE_ACTION_DESTRUCTIVE
    } else if accent.unwrap_or(false) {
        s::SWIPE_ACTION_ACCENT
    } else {
        ""
    };
    let side = match side {
        SwipeSide::Start => "start",
        SwipeSide::End => "end",
    };
    rsx! {
        button {
            class: merge_classes(format!("{} {variant}", s::SWIPE_ACTION), class.as_deref()),
            r#type: "button",
            "data-side": side,
            onclick: move |event| {
                if let Some(onclick) = onclick {
                    onclick.call(event);
                }
            },
            {children}
        }
    }
}

#[component]
pub fn SwipeItem(
    start_actions: Option<Element>,
    end_actions: Option<Element>,
    behavior: Option<SwipeBehavior>,
    disabled: Option<bool>,
    class: Option<String>,
    on_drag: Option<Callback<SwipeState>>,
    on_full_swipe: Option<Callback<SwipeState>>,
    on_swipe_action: Option<Callback<SwipeState>>,
    on_long_press: Option<Callback<()>>,
    children: Element,
) -> Element {
    let behavior = behavior.unwrap_or_default();
    let action_width = action_width_for_behavior(behavior);
    let has_start_actions = start_actions.is_some();
    let has_end_actions = end_actions.is_some();
    let disabled = disabled.unwrap_or(false);
    let mut start_x = use_signal(|| 0.0);
    let mut start_y = use_signal(|| 0.0);
    let mut offset = use_signal(|| 0.0);
    let mut dragging = use_signal(|| false);
    let mut long_press_generation = use_signal(|| 0_u64);
    let mut dismiss_phase = use_signal(|| DismissPhase::Idle);
    // A pointerdown+move+up sequence that actually dragged still fires a
    // synthetic click on release (standard DOM behavior — browsers don't
    // suppress click after a drag on their own). Without this, swiping a
    // row and releasing fires the row's own `onclick` (e.g. "open detail")
    // right on top of the swipe gesture. Disable pointer-events on the
    // content briefly after a real drag so that ghost click has no target.
    let mut suppress_click = use_signal(|| false);
    let mut click_suppress_generation = use_signal(|| 0_u64);
    let on_drag_move = on_drag;
    let on_long_press_down = on_long_press;
    let on_drag_up = on_drag;
    let on_full_swipe_up = on_full_swipe;
    let on_swipe_action_up = on_swipe_action;
    let start_actions_hidden = offset() <= 0.0;
    let end_actions_hidden = offset() >= 0.0;
    let start_actions_inert = start_actions_hidden.then(|| "".to_string());
    let end_actions_inert = end_actions_hidden.then(|| "".to_string());

    rsx! {
        div {
            class: merge_classes(s::SWIPE_ITEM, class.as_deref()),
            style: format!(
                "--g3-swipe-offset: {}px; --g3-swipe-progress: {}; --g3-swipe-action-width: {}px;",
                offset(),
                swipe_ratio(offset(), action_width).abs().min(1.4),
                action_width,
            ),
            "data-behavior": match behavior {
                SwipeBehavior::Reveal => "reveal",
                SwipeBehavior::Activate => "activate",
                SwipeBehavior::Dismiss => "dismiss",
            },
            "data-state": dismiss_phase().as_str(),
            onpointerdown: move |event: PointerEvent| {
                if disabled || dismiss_phase() != DismissPhase::Idle {
                    return;
                }
                dragging.set(true);
                start_x.set(event.client_coordinates().x);
                start_y.set(event.client_coordinates().y);
                click_suppress_generation.with_mut(|value| *value += 1);
                let generation = long_press_generation
                    .with_mut(|value| {
                        *value += 1;
                        *value
                    });
                if let Some(on_long_press) = on_long_press_down {
                    spawn(async move {
                        dioxus_sdk_time::sleep(Duration::from_millis(LONG_PRESS_MS)).await;
                        if long_press_generation() == generation && dragging() {
                            on_long_press.call(());
                        }
                    });
                }
            },
            onpointermove: move |event: PointerEvent| {
                if !dragging() || disabled || dismiss_phase() != DismissPhase::Idle {
                    return;
                }
                let dx = event.client_coordinates().x - start_x();
                let dy = event.client_coordinates().y - start_y();
                if should_cancel_long_press(dx, dy) {
                    long_press_generation.with_mut(|value| *value += 1);
                    suppress_click.set(true);
                }
                let next = swipe_offset_for_behavior(
                    dx,
                    action_width,
                    has_start_actions,
                    has_end_actions,
                    behavior,
                );
                offset.set(next);
                let active = match behavior {
                    SwipeBehavior::Reveal | SwipeBehavior::Dismiss => {
                        should_full_swipe(next, action_width)
                    }
                    SwipeBehavior::Activate => should_activate_swipe(next, action_width),
                };
                if let Some(state) = swipe_state(next, action_width, active)
                    && let Some(on_drag) = on_drag_move {
                        on_drag.call(state);
                    }
            },
            onpointerup: move |_| {
                if disabled || dismiss_phase() != DismissPhase::Idle {
                    return;
                }
                dragging.set(false);
                long_press_generation.with_mut(|value| *value += 1);
                let current = offset();
                match behavior {
                    SwipeBehavior::Reveal => {
                        offset.set(settled_swipe_offset(current, action_width));
                    }
                    SwipeBehavior::Activate => {
                        if should_activate_swipe(current, action_width)
                            && is_swipe_side_available(
                                current,
                                has_start_actions,
                                has_end_actions,
                            )
                            && let Some(state) = swipe_state(current, action_width, true) {
                                if let Some(on_drag) = on_drag_up {
                                    on_drag.call(state);
                                }
                                if let Some(on_swipe_action) = on_swipe_action_up {
                                    on_swipe_action.call(state);
                                }
                            }
                        offset.set(0.0);
                    }
                    SwipeBehavior::Dismiss => {
                        if should_full_swipe(current, action_width)
                            && is_swipe_side_available(
                                current,
                                has_start_actions,
                                has_end_actions,
                            )
                        {
                            if let Some(state) = swipe_state(current, action_width, true) {
                                if let Some(on_drag) = on_drag_up {
                                    on_drag.call(state);
                                }
                                let direction = if state.side == SwipeSide::Start {
                                    1.0
                                } else {
                                    -1.0
                                };
                                offset.set(direction * DISMISS_SWIPE_OFFSET);
                                dismiss_phase.set(DismissPhase::Exiting);
                                spawn(async move {
                                    dioxus_sdk_time::sleep(
                                            Duration::from_millis(DISMISS_EXIT_MS),
                                        )
                                        .await;
                                    dismiss_phase.set(DismissPhase::Collapsing);
                                    dioxus_sdk_time::sleep(
                                            Duration::from_millis(DISMISS_COLLAPSE_MS),
                                        )
                                        .await;
                                    if let Some(on_full_swipe) = on_full_swipe_up {
                                        on_full_swipe.call(state);
                                    }
                                });
                            }
                        } else {
                            offset.set(0.0);
                        }
                    }
                }
                if suppress_click() {
                    let generation = click_suppress_generation.with_mut(|value| {
                        *value += 1;
                        *value
                    });
                    spawn(async move {
                        dioxus_sdk_time::sleep(Duration::from_millis(300)).await;
                        if click_suppress_generation() == generation {
                            suppress_click.set(false);
                        }
                    });
                }
            },
            onpointercancel: move |_| {
                if dismiss_phase() != DismissPhase::Idle {
                    return;
                }
                dragging.set(false);
                long_press_generation.with_mut(|value| *value += 1);
                offset.set(0.0);
                suppress_click.set(false);
            },
            onpointerleave: move |_| {
                if dismiss_phase() != DismissPhase::Idle {
                    return;
                }
                if !dragging() {
                    return;
                }
                dragging.set(false);
                long_press_generation.with_mut(|value| *value += 1);
                offset
                    .set(
                        if behavior == SwipeBehavior::Reveal {
                            settled_swipe_offset(offset(), action_width)
                        } else {
                            0.0
                        },
                    );
            },
            if let Some(start_actions) = start_actions {
                div {
                    class: format!("{} {}", s::SWIPE_ACTIONS, s::SWIPE_ACTIONS_START),
                    aria_hidden: start_actions_hidden.to_string(),
                    inert: start_actions_inert,
                    {start_actions}
                }
            }
            if let Some(end_actions) = end_actions {
                div {
                    class: format!("{} {}", s::SWIPE_ACTIONS, s::SWIPE_ACTIONS_END),
                    aria_hidden: end_actions_hidden.to_string(),
                    inert: end_actions_inert,
                    {end_actions}
                }
            }
            div {
                class: s::SWIPE_CONTENT,
                style: if suppress_click() { "pointer-events: none;" } else { "" },
                {children}
            }
        }
    }
}

#[cfg(feature = "playground")]
#[component]
pub fn ListPlaygroundDemo() -> Element {
    let mut last_action = use_signal(|| "Long-press the reveal row or swipe any row".to_string());
    let mut dismiss_visible = use_signal(|| true);
    let inset = use_signal(|| true);
    let lines_index = use_signal(|| 1_usize);
    let lines = match lines_index() {
        0 => ListLines::Full,
        2 => ListLines::None,
        _ => ListLines::Inset,
    };
    rsx! {
        crate::PlaygroundDemoFrame {
            center: false,
            controls: rsx! {
                crate::Checkbox { checked: inset, label: "Inset".to_string() }
                div {
                    span { "Dividers" }
                    crate::SegmentGroup { active: lines_index,
                        crate::SegmentButton { index: 0, "Full" }
                        crate::SegmentButton { index: 1, "Inset" }
                        crate::SegmentButton { index: 2, "None" }
                    }
                }
            },
            div { class: "g3-list-demo-stack",
                List { inset: inset(), lines,
                    ItemDivider { "Round" }
                    Item {
                        start: rsx! {
                            crate::Avatar { fallback: "MW" }
                        },
                        label: "Matthew Weisfeld",
                        description: "Walking 18 holes",
                        metadata: "9:40",
                    }
                    Item {
                        kind: ItemKind::Link("https://example.com".to_string()),
                        label: "Link row",
                        description: "Opens a destination",
                    }
                    Item {
                        kind: ItemKind::Button,
                        label: "Button row",
                        description: "Tap action",
                        detail: ItemDetail::Show,
                        onclick: move |_| last_action.set("Tapped button row".to_string()),
                    }
                    ItemDivider { "Swipe" }
                    SwipeItem {
                        behavior: SwipeBehavior::Reveal,
                        start_actions: rsx! {
                            SwipeAction {
                                side: SwipeSide::Start,
                                accent: true,
                                onclick: move |_| last_action.set("Pinned from revealed action".to_string()),
                                "Pin"
                            }
                        },
                        end_actions: rsx! {
                            SwipeAction {
                                side: SwipeSide::End,
                                destructive: true,
                                onclick: move |_| last_action.set("Deleted from revealed action".to_string()),
                                "Delete"
                            }
                        },
                        on_long_press: move |_| last_action.set("Long press fired".to_string()),
                        Item {
                            kind: ItemKind::Button,
                            label: "Reveal actions",
                            description: "Long press or expose side buttons",
                            onclick: |_| {},
                        }
                    }
                    SwipeItem {
                        behavior: SwipeBehavior::Activate,
                        start_actions: rsx! {
                            SwipeAction { side: SwipeSide::Start, accent: true, "Archive" }
                        },
                        end_actions: rsx! {
                            SwipeAction { side: SwipeSide::End, destructive: true, "Flag" }
                        },
                        on_swipe_action: move |state: SwipeState| {
                            last_action.set(format!("Quick swipe activated: {:?}", state.side))
                        },
                        Item {
                            label: "Quick swipe",
                            description: "Stops early and emits the side action",
                        }
                    }
                    if dismiss_visible() {
                        SwipeItem {
                            behavior: SwipeBehavior::Dismiss,
                            end_actions: rsx! {
                                SwipeAction { side: SwipeSide::End, destructive: true, "Remove" }
                            },
                            on_full_swipe: move |state: SwipeState| {
                                last_action.set(format!("Dismissed by {:?} swipe", state.side));
                                dismiss_visible.set(false);
                            },
                            Item {
                                label: "Dismiss swipe",
                                description: "Full swipe removes the row",
                            }
                        }
                    }
                }
                crate::Badge { color: crate::StatusColor::Neutral, "{last_action()}" }
                if !dismiss_visible() {
                    crate::Button {
                        style: crate::ButtonStyle::Clear,
                        onclick: move |_| dismiss_visible.set(true),
                        "Restore dismiss row"
                    }
                }
            }
        }
    }
}
crate::g3_playground! {
    name: "List",
    description: "Mobile list rows with slots, dividers, and swipe actions.",
    demo: ListPlaygroundDemo,
    source: "src/components/list.rs",
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::G3ThemeProvider;

    fn render(app: fn() -> Element) {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
    }

    #[test]
    fn elastic_swipe_offset_slows_after_action_width() {
        assert_eq!(elastic_swipe_offset(44.0, 88.0), 44.0);
        assert_eq!(elastic_swipe_offset(188.0, 88.0), 143.0);
        assert_eq!(elastic_swipe_offset(-188.0, 88.0), -143.0);
    }

    #[test]
    fn full_swipe_requires_action_width_plus_margin() {
        assert!(!should_full_swipe(117.0, 88.0));
        assert!(should_full_swipe(118.0, 88.0));
        assert!(should_full_swipe(-118.0, 88.0));
    }

    #[test]
    fn swipe_side_follows_offset_direction() {
        assert_eq!(swipe_side(12.0), Some(SwipeSide::Start));
        assert_eq!(swipe_side(-12.0), Some(SwipeSide::End));
        assert_eq!(swipe_side(0.0), None);
    }

    #[test]
    fn swipe_ratio_follows_action_width() {
        assert_eq!(swipe_ratio(44.0, 88.0), 0.5);
        assert_eq!(swipe_ratio(-88.0, 88.0), -1.0);
    }

    #[test]
    fn long_press_cancels_after_movement_threshold() {
        assert!(!should_cancel_long_press(4.0, 4.0));
        assert!(should_cancel_long_press(9.0, 0.0));
    }

    #[test]
    fn reveal_swipe_offsets_stop_at_action_width() {
        assert_eq!(
            swipe_offset_for_behavior(188.0, 88.0, true, true, SwipeBehavior::Reveal),
            88.0
        );
        assert_eq!(
            swipe_offset_for_behavior(-188.0, 88.0, true, true, SwipeBehavior::Reveal),
            -88.0
        );
    }

    #[test]
    fn activate_swipe_offsets_slow_toward_limit() {
        let offset = swipe_offset_for_behavior(400.0, 88.0, true, true, SwipeBehavior::Activate);
        assert!(offset > 44.0);
        assert!(offset < 88.0);
    }

    #[test]
    fn activate_swipe_follows_farther_before_soft_limit() {
        assert_eq!(activate_swipe_offset(80.0, 136.0), 80.0);
        let midpoint = activate_swipe_offset(136.0, 136.0);
        assert!(midpoint > 110.0);
        assert!(midpoint < 136.0);
        let long_drag = activate_swipe_offset(400.0, 136.0);
        assert!(long_drag > 130.0);
        assert!(long_drag < 136.0);
    }

    #[test]
    fn dismiss_swipe_delays_callback_until_exit_and_gap_collapse() {
        let source = include_str!("list.rs");
        let stylesheet = include_str!("../../assets/g3_ui.css");

        assert!(source.contains("DismissPhase::Exiting"));
        assert!(source.contains("DISMISS_EXIT_MS"));
        assert!(source.contains("DISMISS_COLLAPSE_MS"));
        assert!(source.contains("dioxus_sdk_time::sleep(Duration::from_millis(DISMISS_EXIT_MS))"));
        assert!(
            source.contains("dioxus_sdk_time::sleep(Duration::from_millis(DISMISS_COLLAPSE_MS))")
        );
        assert!(
            source
                .find("offset.set(direction * DISMISS_SWIPE_OFFSET)")
                .unwrap()
                < source.find("on_full_swipe.call(state)").unwrap()
        );
        assert!(stylesheet.contains(".g3-swipe-item[data-state=\"exiting\"]"));
        assert!(stylesheet.contains(".g3-swipe-item[data-state=\"collapsing\"]"));
        assert!(stylesheet.contains("max-height"));
    }

    #[test]
    fn activate_swipe_uses_early_threshold() {
        assert!(!should_activate_swipe(39.0, 88.0));
        assert!(should_activate_swipe(44.0, 88.0));
    }
    #[test]
    fn constrained_swipe_offset_ignores_missing_action_sides() {
        assert_eq!(constrained_swipe_offset(44.0, 88.0, false, true), 0.0);
        assert_eq!(constrained_swipe_offset(-44.0, 88.0, true, false), 0.0);
        assert_eq!(constrained_swipe_offset(44.0, 88.0, true, false), 44.0);
        assert_eq!(constrained_swipe_offset(-44.0, 88.0, false, true), -44.0);
    }

    #[test]
    fn item_lines_can_defer_to_parent_list() {
        let source = include_str!("list.rs");
        let stylesheet = include_str!("../../assets/g3_ui.css");
        assert!(source.contains("lines.map(ListLines::class).unwrap_or"));
        assert!(stylesheet.contains(".g3-list[data-lines=\"full\"] .g3-item"));
        assert!(stylesheet.contains(
            ":not(.g3-item-lines-full):not(.g3-item-lines-inset):not(.g3-item-lines-none)"
        ));
        assert!(
            stylesheet.contains(".g3-list > :is(.g3-item-row, .g3-swipe-item):last-child .g3-item")
        );
        assert!(
            stylesheet.contains(
                ".g3-list > :is(.g3-item-row, .g3-swipe-item):last-child .g3-item::after"
            )
        );
    }

    #[test]
    fn swipe_actions_are_hidden_from_keyboard_when_closed() {
        let source = include_str!("list.rs");
        assert!(source.contains("start_actions_hidden"));
        assert!(source.contains("inert: start_actions_inert"));
        assert!(source.contains("aria_hidden: start_actions_hidden.to_string()"));
    }

    #[test]
    fn item_controls_keep_native_roles() {
        let source = include_str!("list.rs");
        assert!(!source.contains("a { class: cls, href, role: \"listitem\""));
        assert!(!source.contains("button { class: cls, r#type: \"button\", role: \"listitem\""));
        assert!(source.contains("div { class: s::ITEM_ROW, role: \"listitem\""));
    }

    #[test]
    fn default_item_with_onclick_promotes_to_button_semantics() {
        let source = include_str!("list.rs");
        assert!(source.contains("(ItemKind::Static, true) => ItemKind::Button"));
        assert!(source.contains("button { class: cls"));
    }

    #[test]
    fn disabled_links_drop_anchor_navigation() {
        let source = include_str!("list.rs");
        assert!(source.contains("ItemKind::Link(href) if !disabled"));
        assert!(source.contains("ItemKind::Link(_)"));
        assert!(source.contains("role: \"link\", aria_disabled"));
    }

    #[test]
    fn full_swipe_requires_available_action_side() {
        let source = include_str!("list.rs");
        assert!(
            source.contains("should_full_swipe(current, action_width) && is_swipe_side_available")
        );
        assert!(!is_swipe_side_available(44.0, false, true));
        assert!(is_swipe_side_available(-44.0, false, true));
    }

    #[test]
    fn pointer_leave_cleans_up_drag_state() {
        let source = include_str!("list.rs");
        assert!(source.contains("onpointerleave"));
        assert!(source.contains("if !dragging() { return; }"));
        assert!(source.contains("offset.set(settled_swipe_offset(offset(), action_width))"));
    }

    #[test]
    fn settled_swipe_offset_opens_after_midpoint() {
        assert_eq!(settled_swipe_offset(45.0, 88.0), 88.0);
        assert_eq!(settled_swipe_offset(-45.0, 88.0), -88.0);
        assert_eq!(settled_swipe_offset(40.0, 88.0), 0.0);
    }

    #[component]
    fn ListSmokeApp() -> Element {
        rsx! {
            G3ThemeProvider { mode: ComponentMode::Ios,
                List { inset: true,
                    Item { label: "Static", description: "Description" }
                    SwipeItem {
                        start_actions: rsx! {
                            SwipeAction { side: SwipeSide::Start, accent: true, "Pin" }
                        },
                        end_actions: rsx! {
                            SwipeAction { side: SwipeSide::End, destructive: true, "Delete" }
                        },
                        Item { label: "Swipe", description: "Drag row" }
                    }
                }
            }
        }
    }

    #[test]
    fn list_family_renders() {
        render(ListSmokeApp);
    }
}
