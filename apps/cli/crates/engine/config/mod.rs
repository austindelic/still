//! Config discovery, parsing, mutation, and persistence boundaries.

pub mod discovery;
pub mod edit;
pub mod model;
pub mod parse;
pub mod store;

pub use discovery::*;
pub use edit::{
    RemoveItemTarget, add_install_items, add_install_items_with_force, remove_item,
    remove_item_target,
};
pub use model::*;
pub use store::*;
