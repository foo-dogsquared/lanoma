//! This module simply defines all of the helpers to be used in Handlebars.
//! The functions defined here are eventually registered in the profile templates.

use std::path::PathBuf;

use chrono;
use handlebars;
use heck::{CamelCase, KebabCase, SnakeCase, TitleCase};

use crate::helpers;

// TODO: Convert this into a macro.
pub fn add_float(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _rc: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let final_value = h
        .params()
        .into_iter()
        .map(|param| param.value().as_f64().unwrap_or(0.0))
        .fold(0.0, |acc, value| value + acc);

    out.write(&final_value.to_string())?;
    Ok(())
}

pub fn add_int(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _rc: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let final_value = h
        .params()
        .into_iter()
        .map(|param| param.value().as_i64().unwrap_or(0))
        .fold(0, |acc, value| value + acc);

    out.write(&final_value.to_string())?;
    Ok(())
}

pub fn sub_float(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _rc: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let mut operands: Vec<f64> = h
        .params()
        .into_iter()
        .map(|param| param.value().as_f64().unwrap_or(0.0))
        .collect();
    let final_value = match operands.is_empty() {
        true => 0.0,
        false => {
            let initial_value = operands.remove(0);
            operands
                .into_iter()
                .fold(initial_value, |acc, value| acc - value)
        }
    };

    out.write(&final_value.to_string())?;
    Ok(())
}

pub fn sub_int(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _rc: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let mut operands: Vec<i64> = h
        .params()
        .into_iter()
        .map(|param| param.value().as_i64().unwrap_or(0))
        .collect();
    let final_value = match operands.is_empty() {
        true => 0,
        false => {
            let initial_value = operands.remove(0);
            operands
                .into_iter()
                .fold(initial_value, |acc, value| acc - value)
        }
    };

    out.write(&final_value.to_string())?;
    Ok(())
}

pub fn div_float(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _rc: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let mut operands: Vec<f64> = h
        .params()
        .into_iter()
        .map(|param| param.value().as_f64().unwrap_or(1.0))
        .collect();
    let final_value = match operands.is_empty() {
        true => 0.0,
        false => {
            let initial_value = operands.remove(0);
            operands
                .into_iter()
                .fold(initial_value, |acc, value| acc / value)
        }
    };

    out.write(&final_value.to_string())?;
    Ok(())
}

pub fn div_int(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _rc: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let mut operands: Vec<i64> = h
        .params()
        .into_iter()
        .map(|param| param.value().as_i64().unwrap_or(1))
        .collect();
    let final_value = match operands.is_empty() {
        true => 1,
        false => {
            let initial_value = operands.remove(0);
            operands
                .into_iter()
                .fold(initial_value, |acc, value| acc / value)
        }
    };

    out.write(&final_value.to_string())?;
    Ok(())
}

pub fn mult_float(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _rc: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let final_value = h
        .params()
        .into_iter()
        .map(|param| param.value().as_f64().unwrap_or(1.0))
        .fold(1.0, |acc, value| value * acc);

    out.write(&final_value.to_string())?;
    Ok(())
}

pub fn mult_int(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _rc: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let final_value = h
        .params()
        .into_iter()
        .map(|param| param.value().as_i64().unwrap_or(1))
        .fold(0, |acc, value| value * acc);

    out.write(&final_value.to_string())?;
    Ok(())
}

// Letter case functions.
handlebars::handlebars_helper!(kebab_case: |s: str| s.to_kebab_case());
handlebars::handlebars_helper!(snake_case: |s: str| s.to_snake_case());
handlebars::handlebars_helper!(title_case: |s: str| s.to_title_case());
handlebars::handlebars_helper!(camel_case: |s: str| s.to_camel_case());
handlebars::handlebars_helper!(upper_case: |s: str| s.to_uppercase());
handlebars::handlebars_helper!(lower_case: |s: str| s.to_lowercase());

// Miscellaneous functions.
handlebars::handlebars_helper!(is_file: |s: str| PathBuf::from(s).is_file());
handlebars::handlebars_helper!(is_dir: |s: str| PathBuf::from(s).is_dir());

pub fn relpath(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _rc: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let dst = PathBuf::from(h.param(0).and_then(|v| v.value().as_str()).unwrap_or(""));
    let base = PathBuf::from(h.param(1).and_then(|v| v.value().as_str()).unwrap_or(""));
    let result = helpers::fs::relative_path_from(dst, base).unwrap_or(PathBuf::new());

    out.write(result.to_str().unwrap_or(""))?;
    Ok(())
}

pub fn reldate(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    rc: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let format = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("%F");
    let relative_days = h.param(1).and_then(|v| v.value().as_i64()).unwrap_or(0);
    let now = chrono::Local::now();
    let days = chrono::Duration::days(relative_days);

    let datetime_delta = now + days;

    out.write(datetime_delta.format(format).to_string().as_ref())?;
    Ok(())
}
