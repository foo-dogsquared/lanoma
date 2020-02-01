//! An adapter for a template engine.
//! This is implemented in case Lanoma decides to support multiple template engine.
//!
//! (On the other hand, this may be just a case of overengineering.)

use std::fs;
use std::ops::Deref;
use std::path::Path;

use globwalk;
use handlebars;
use serde;

use crate::error::Error;
use crate::helpers;

/// A trait for the template registry.
/// It handles all of the template operations such as checking if the there is already a template
/// with the specified name, rendering them, and including templates in the template list.
pub trait TemplateRegistry {
    /// Register the template.
    fn register(
        &mut self,
        template: &Template,
    ) -> Result<(), Error>;

    fn unregister<S>(
        &mut self,
        template_name: S,
    ) -> Result<(), Error>
    where
        S: AsRef<str>;

    /// Checks if the template is already in the registry.
    fn has_template<S>(
        &self,
        name: S,
    ) -> bool
    where
        S: AsRef<str>;

    /// Render the template given with the specified name.
    /// It should also render with the given value.
    fn render<S, V>(
        &self,
        name: S,
        value: V,
    ) -> Result<String, Error>
    where
        S: AsRef<str>,
        V: serde::Serialize;
}

/// The template registry implemented with the `rust-handlebars` crate.
#[derive(Debug)]
pub struct TemplateHandlebarsRegistry<'a>(handlebars::Handlebars<'a>);

impl<'a> TemplateRegistry for TemplateHandlebarsRegistry<'a> {
    /// Registers a template in the registry.
    /// If there is a template with the same name, it will be overwritten.
    fn register(
        &mut self,
        template: &Template,
    ) -> Result<(), Error> {
        self.0
            .register_template_string(&template.name, &template.s)
            .map_err(Error::HandlebarsTemplateError)
    }

    fn unregister<S>(
        &mut self,
        template_name: S,
    ) -> Result<(), Error>
    where
        S: AsRef<str>,
    {
        self.0.unregister_template(template_name.as_ref());

        Ok(())
    }

    fn has_template<S>(
        &self,
        name: S,
    ) -> bool
    where
        S: AsRef<str>,
    {
        self.0.has_template(name.as_ref())
    }

    fn render<S, V>(
        &self,
        template_name: S,
        value: V,
    ) -> Result<String, Error>
    where
        S: AsRef<str>,
        V: serde::Serialize,
    {
        self.0
            .render(template_name.as_ref(), &value)
            .map_err(Error::HandlebarsRenderError)
    }
}

impl<'a> Deref for TemplateHandlebarsRegistry<'a> {
    type Target = handlebars::Handlebars<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> AsMut<handlebars::Handlebars<'a>> for TemplateHandlebarsRegistry<'a> {
    fn as_mut(&mut self) -> &mut handlebars::Handlebars<'a> {
        &mut self.0
    }
}

impl<'a> TemplateHandlebarsRegistry<'a> {
    /// Creates a new instance of the registry.
    pub fn new() -> Self {
        let mut renderer = handlebars::Handlebars::new();
        renderer.register_escape_fn(handlebars::no_escape);

        Self(renderer)
    }

    /// Returns the wrapped template engine as a reference.
    pub fn registry(&self) -> &handlebars::Handlebars {
        &self.0
    }

    /// Register a vector of template.
    /// This does not check if the template registration is successful.
    pub fn register_vec<'b>(
        &mut self,
        templates: &'b Vec<Template>,
    ) -> Result<Vec<&'b Template>, Error> {
        let mut registered_templates = vec![];
        let mut template_errors = vec![];
        for template in templates.iter() {
            match self
                .0
                .register_template_string(&template.name, &template.s)
                .map_err(Error::HandlebarsTemplateError)
            {
                Ok(_v) => registered_templates.push(template),
                Err(e) => template_errors.push(e),
            }
        }

        match template_errors.is_empty() {
            true => Ok(registered_templates),
            false => Err(template_errors).map_err(Error::Errors),
        }
    }

    /// Register the template with the specified name.
    /// This is just a thin wrapper behind the `rust-handlebars::Handlebars` struct.
    pub fn register_template_string<N, S>(
        &mut self,
        name: N,
        s: S,
    ) -> Result<(), Error>
    where
        N: AsRef<str>,
        S: AsRef<str>,
    {
        self.0
            .register_template_string(name.as_ref(), s.as_ref())
            .map_err(Error::HandlebarsTemplateError)
    }
}

/// A generic struct for templates to be used in a template engine.
#[derive(Debug)]
pub struct Template {
    name: String,
    s: String,
}

impl Template {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            s: String::new(),
        }
    }

    pub fn from_path<P, S>(
        path: P,
        name: S,
    ) -> Result<Self, Error>
    where
        P: AsRef<Path>,
        S: AsRef<str>,
    {
        let path = path.as_ref();
        let name = name.as_ref();
        let s = fs::read_to_string(&path).map_err(Error::IoError)?;

        Ok(Self {
            name: name.to_string(),
            s,
        })
    }
}

/// A template builder.
/// It specifically looks for a file glob to get the templates.
pub struct TemplateGetter;

impl TemplateGetter {
    /// Get a bunch of templates.
    pub fn get_templates<P, S>(
        path: P,
        file_ext: S,
    ) -> Result<Vec<Template>, Error>
    where
        P: AsRef<Path>,
        S: AsRef<str>,
    {
        let path = path.as_ref();
        let file_ext = file_ext.as_ref();
        let mut templates: Vec<Template> = vec![];

        let files = globwalk::GlobWalkerBuilder::new(&path, format!("**/*.{}", file_ext))
            .min_depth(1)
            .build()
            .map_err(Error::GlobParsingError)?;
        for file in files {
            if let Ok(file) = file {
                let relpath_from_path = helpers::fs::relative_path_from(file.path(), path).unwrap();
                let path_as_str = relpath_from_path.to_string_lossy();
                let relpath_from_path_without_file_ext =
                    &path_as_str[..path_as_str.len() - file_ext.len() - 1];
                match Template::from_path(file.path(), relpath_from_path_without_file_ext) {
                    Ok(v) => templates.push(v),
                    Err(_e) => continue,
                }
            }
        }

        Ok(templates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts;
    use std::fs;
    use std::io::Write;
    use tempfile;

    #[test]
    pub fn search_for_tex_files() -> Result<(), Error> {
        let tmp_dir = tempfile::TempDir::new().map_err(Error::IoError)?;
        for file in &["a.tex", "b.txt", "c.tex", "d.tex"] {
            let mut file_handle =
                fs::File::create(tmp_dir.path().join(file)).map_err(Error::IoError)?;
            file_handle
                .write(consts::NOTE_TEMPLATE.as_bytes())
                .map_err(Error::IoError)?;
        }

        let template_files = TemplateGetter::get_templates(tmp_dir.path(), "tex")?;

        assert_eq!(template_files.len(), 3);

        Ok(())
    }
}
