//! Shared management UI. Features own data, permissions and mutations.

mod buttons;
pub(crate) use buttons::*;
mod controls;
pub(crate) use controls::*;
mod dialogs;
pub(crate) use dialogs::*;
mod forms;
pub(crate) use forms::*;
mod info;
pub(crate) use info::*;
mod inputs;
pub(crate) use inputs::*;
mod surfaces;
pub(crate) use surfaces::*;
mod tabs;
pub(crate) use tabs::*;
mod layout;
pub(crate) use layout::*;
mod lists;
pub(crate) use lists::*;
mod multi_select;
pub(crate) use multi_select::*;
