//! The rules of CSS, SCSS and Less: pattern files in `rules/*.yaml`, and the families they name.

mod checks;
mod font;
mod max;
mod model;
mod names;
mod values;

use core_rules::Family;

pub static FAMILIES: &[Family] = &[names::FAMILY, max::FAMILY, checks::FAMILY];
