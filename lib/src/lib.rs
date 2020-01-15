use std::result;

#[macro_use]
extern crate handlebars;

use toml::{self};

#[macro_use]
extern crate lazy_static;

pub mod config;
mod consts;
pub mod error;
mod helpers;
pub mod masternote;
pub mod note;
pub mod profile;
pub mod shelf;
pub mod subjects;
pub mod templates;

use crate::error::Error;

pub type Result<T> = result::Result<T, Error>;

// Making it static since it does not handle any templates anyway and only here for rendering the string.
lazy_static! {
    /// A static Handlebars registry.
    pub static ref HANDLEBARS_REG: handlebars::Handlebars = handlebars::Handlebars::new();
}

/// A trait that specifies an object has a set of associated data.
pub trait Object {
    fn data(&self) -> toml::Value;
}

/// A basic macro for modifying a TOML table.
#[macro_export]
macro_rules! modify_toml_table {
    ($var:ident, $( ($field:expr, $value:expr) ),*) => {
        let temp_table = $var.as_table_mut().unwrap();

        $(
            temp_table.insert(String::from($field), toml::Value::try_from($value).unwrap());
        )*
    };
}

/// A basic macro for upserting a TOML table.
#[macro_export]
macro_rules! upsert_toml_table {
    ($var:ident, $( ($field:expr, $value:expr) ),*) => {
        let temp_table = $var.as_table_mut().unwrap();

        $(
            if temp_table.get($field).is_none() {
                temp_table.insert(String::from($field), toml::Value::try_from($value).unwrap());
            }
        )*
    };
}
