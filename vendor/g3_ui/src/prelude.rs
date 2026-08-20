//! g3_ui prelude - import all public symbols in one line.

pub use crate::theme::{
    ComponentMode, G3Mode, G3Theme, G3ThemeProvider, Theme, get_mode, init_auto_mode,
    merge_classes, set_mode, use_component_mode,
};
pub use dioxus::prelude::*;

// Components
pub use crate::components::{
    AccordionGroup, AccordionItem, Avatar, AvatarSize, Badge, Button, ButtonSize, ButtonStyle,
    Checkbox, Chip, ControlLabelPlacement, Field, InfoButton, Item, ItemDetail, ItemDivider,
    ItemKind, Line, LineOrientation, List, ListLines, Progress, Radio, RadioGroup, Refresher,
    RefresherState, SegmentButton, SegmentGroup, Skeleton, SkeletonShape, Spinner, StatusColor,
    SwipeAction, SwipeBehavior, SwipeItem, SwipeSide, SwipeState, Toast, ToastPosition, Toggle,
    ToggleSize,
};
pub use crate::{
    G3AccordionGroup, G3AccordionItem, G3Avatar, G3Badge, G3Button, G3Checkbox, G3Chip,
    G3ControlLabelPlacement, G3Field, G3InfoButton, G3Item, G3ItemDivider, G3Line, G3List,
    G3Progress, G3Radio, G3RadioGroup, G3Refresher, G3SegmentButton, G3SegmentGroup, G3Skeleton,
    G3Spinner, G3SwipeAction, G3SwipeItem, G3Toast, G3Toggle, G3ToggleSize,
};

// Component aliases
pub use crate::components::{
    Card, ConfirmModal, Fab, FabButton, FabContainer, FabHorizontal, FabList, FabListSide,
    FabVertical, Modal, Navbar, NavbarTab, NavbarTabBar, RightSlot, Select, SelectOption, Sheet,
    SheetButton, SheetPlacement,
};
pub use crate::{
    G3Card, G3ConfirmModal, G3Fab, G3FabButton, G3FabContainer, G3FabList, G3Modal, G3Navbar,
    G3NavbarTab, G3NavbarTabBar, G3Select, G3Sheet, G3SheetButton, G3SheetPlacement,
};

// Layout components
pub use crate::components::{AppWrapper, Body, Header};
pub use crate::{G3AppWrapper, G3Body, G3Header};

pub use crate::{ComponentDescriptor, component_descriptors};
#[cfg(feature = "playground")]
pub use crate::{ComponentPlaygroundDemo, component_playground_demos};
