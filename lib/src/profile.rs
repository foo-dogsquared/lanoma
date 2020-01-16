//! A profile is an object that holds all of the required information for the operations in Lanoma.
//! For now, a profile contains the metadata and the templates.
//!
//! On the filesystem, it is represented as a folder with specific files and folders.
//! The required component in the filesystem is the profile metadata.

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use toml::{self, Value};

use crate::config::ProfileConfig;
use crate::consts;
use crate::error::Error;
use crate::helpers::{self, handlebars as handlebars_helpers};
use crate::templates::{self, TemplateGetter};
use crate::Object;

// profile constants
pub const PROFILE_METADATA_FILENAME: &str = ".profile.toml";
pub const PROFILE_TEMPLATE_FILES_DIR_NAME: &str = ".templates";

pub const TEMPLATE_FILE_EXTENSION: &str = "hbs";
pub const PROFILE_NOTE_TEMPLATE_NAME: &str = "_default";
pub const PROFILE_MASTER_NOTE_TEMPLATE_NAME: &str = "master/_default";

/// A builder for constructing the profile.
/// Setting the values does not consume the builder for dynamic setting.
pub struct ProfileBuilder {
    path: Option<PathBuf>,
    name: Option<String>,
    extra_metadata: Option<HashMap<String, Value>>,
}

impl ProfileBuilder {
    /// Creates a new profile builder instance.
    pub fn new() -> Self {
        Self {
            path: None,
            name: None,
            extra_metadata: None,
        }
    }

    /// Sets the path for the builder.
    pub fn path<P>(
        &mut self,
        p: P,
    ) -> &mut Self
    where
        P: AsRef<Path> + Clone,
    {
        let p = p.as_ref().to_path_buf();

        self.path = Some(p);
        self
    }

    /// Sets the name of the profile builder.
    pub fn name<S>(
        &mut self,
        name: S,
    ) -> &mut Self
    where
        S: AsRef<str> + Clone,
    {
        let name = name.clone().as_ref().to_string();

        self.name = Some(name);
        self
    }

    /// Sets the extra metadata of the profile builder.
    pub fn extra_metadata(
        &mut self,
        extra: HashMap<String, Value>,
    ) -> &mut Self {
        self.extra_metadata = Some(extra);
        self
    }

    /// Consumes the builder to create the profile instance.
    pub fn build(self) -> Profile {
        let mut profile = Profile::new();

        if self.path.is_some() {
            let path = self.path.unwrap();

            profile.path = path.clone();
        }

        let mut metadata_instance = ProfileConfig::new();

        if self.name.is_some() {
            let name = self.name.unwrap();
            metadata_instance.name = name.into();
        }

        if self.extra_metadata.is_some() {
            let extra = self.extra_metadata.unwrap();

            metadata_instance.extra = extra;
        }

        profile.config = metadata_instance;

        profile
    }
}

/// A profile holds certain metadata such as the templates.
pub struct Profile {
    path: PathBuf,
    config: ProfileConfig,
    templates: templates::TemplateHandlebarsRegistry,
}

impl Object for Profile {
    fn data(&self) -> toml::Value {
        toml::Value::try_from(self.config()).unwrap()
    }
}

impl Default for Profile {
    fn default() -> Self {
        let mut profile = Self::new();
        profile.init_templates().unwrap();

        profile
    }
}

impl Profile {
    /// Creates a profile instance with empty data.
    pub fn new() -> Self {
        Self {
            path: PathBuf::new(),
            config: ProfileConfig::new(),
            templates: templates::TemplateHandlebarsRegistry::new(),
        }
    }

    /// Opens an initiated profile.
    ///
    /// If the profile does not exist in the given path, it will cause an error.
    /// It will also detect the contents of the files inside of the templates directory to be registered to the Handlebars registry.
    pub fn from<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let path: PathBuf = path.as_ref().to_path_buf();

        let mut profile = Self::new();

        profile.path = fs::canonicalize(path).map_err(Error::IoError)?;
        if !profile.has_templates() {
            return Err(Error::InvalidProfileError(profile.path.clone()));
        }

        profile.init_templates()?;
        // Getting the templates with a specific file extension.
        // This also overrides the default templates if found any.
        let templates =
            TemplateGetter::get_templates(profile.templates_path(), TEMPLATE_FILE_EXTENSION)?;
        profile.templates.register_vec(&templates)?;

        profile.config = ProfileConfig::try_from(profile.metadata_path())?;

        Ok(profile)
    }

    pub fn config(&self) -> &ProfileConfig {
        &self.config
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn template_registry(&self) -> &templates::TemplateHandlebarsRegistry {
        &self.templates
    }

    /// Initialize the template registry.
    fn init_templates(&mut self) -> Result<(), Error> {
        let mut registry = templates::TemplateHandlebarsRegistry::new();

        // registering with the default templates
        registry.register_template_string(PROFILE_NOTE_TEMPLATE_NAME, consts::NOTE_TEMPLATE)?;
        registry.register_template_string(
            PROFILE_MASTER_NOTE_TEMPLATE_NAME,
            consts::MASTER_NOTE_TEMPLATE,
        )?;

        // Registering some helper functions in the Handlebars registry.
        let registry_as_mut = registry.as_mut();
        // Mathematical functions.
        registry_as_mut.register_helper("add-float", Box::new(handlebars_helpers::add_float));
        registry_as_mut.register_helper("add-int", Box::new(handlebars_helpers::add_int));
        registry_as_mut.register_helper("sub-float", Box::new(handlebars_helpers::sub_float));
        registry_as_mut.register_helper("sub-int", Box::new(handlebars_helpers::sub_int));
        registry_as_mut.register_helper("div-int", Box::new(handlebars_helpers::div_int));
        registry_as_mut.register_helper("div-float", Box::new(handlebars_helpers::div_float));
        registry_as_mut.register_helper("div-int", Box::new(handlebars_helpers::div_int));
        registry_as_mut.register_helper("mul-int", Box::new(handlebars_helpers::mult_int));
        registry_as_mut.register_helper("mul-float", Box::new(handlebars_helpers::mult_float));

        // Letter case functions.
        registry_as_mut.register_helper("upper-case", Box::new(handlebars_helpers::upper_case));
        registry_as_mut.register_helper("lower-case", Box::new(handlebars_helpers::lower_case));
        registry_as_mut.register_helper("kebab-case", Box::new(handlebars_helpers::kebab_case));
        registry_as_mut.register_helper("snake-case", Box::new(handlebars_helpers::snake_case));
        registry_as_mut.register_helper("camel-case", Box::new(handlebars_helpers::camel_case));
        registry_as_mut.register_helper("title-case", Box::new(handlebars_helpers::title_case));

        // Miscellaneous helpers.
        registry_as_mut.register_helper("is-file", Box::new(handlebars_helpers::is_file));
        registry_as_mut.register_helper("is-dir", Box::new(handlebars_helpers::is_dir));
        registry_as_mut.register_helper("reldate", Box::new(handlebars_helpers::reldate));
        registry_as_mut.register_helper("relpath", Box::new(handlebars_helpers::relpath));

        self.templates = registry;

        Ok(())
    }

    /// Returns the metadata file path of the profile.
    pub fn metadata_path(&self) -> PathBuf {
        let mut path = self.path.clone();
        path.push(PROFILE_METADATA_FILENAME);

        path
    }

    /// Checks if the metadata is in the filesystem.
    pub fn has_metadata(&self) -> bool {
        self.metadata_path().exists()
    }

    /// Returns the template path of the profile.
    pub fn templates_path(&self) -> PathBuf {
        let mut path = self.path.clone();
        path.push(PROFILE_TEMPLATE_FILES_DIR_NAME);

        path
    }

    /// Checks if the templates is in the filesystem.
    pub fn has_templates(&self) -> bool {
        self.templates_path().exists()
    }

    /// Checks if the profile has been exported in the filesystem.
    pub fn is_exported(&self) -> bool {
        self.path.exists()
    }

    /// Checks if the profile has a valid folder structure.
    pub fn is_valid(&self) -> bool {
        self.is_exported() && self.has_templates() && self.has_metadata()
    }

    /// Get the metadata from the profile.
    pub fn get_metadata(&self) -> Result<ProfileConfig, Error> {
        if !self.is_valid() {
            return Err(Error::InvalidProfileError(self.path.clone()));
        }

        let metadata_file_string =
            fs::read_to_string(self.metadata_path()).map_err(Error::IoError)?;
        let metadata: ProfileConfig =
            toml::from_str(&metadata_file_string).map_err(Error::TomlValueError)?;

        Ok(metadata)
    }

    /// Export the profile in the filesystem.
    pub fn export(&mut self) -> Result<(), Error> {
        let dir_builder = DirBuilder::new();

        if self.is_valid() {
            return Err(Error::ProfileAlreadyExists(self.path()));
        }

        if !self.is_exported() {
            helpers::fs::create_folder(&dir_builder, self.path())?;
        }

        if !self.has_metadata() {
            let mut metadata_file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(self.metadata_path())
                .map_err(Error::IoError)?;

            metadata_file
                .write_all(toml::to_string_pretty(self.config()).unwrap().as_bytes())
                .map_err(Error::IoError)?;
        }

        if !self.has_templates() {
            helpers::fs::create_folder(&dir_builder, self.templates_path())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::Note;
    use crate::shelf::{Shelf, ShelfItem};
    use crate::subjects::Subject;
    use crate::templates::TemplateRegistry;
    use tempfile;
    use toml;

    fn tmp_profile() -> Result<(tempfile::TempDir, Profile), Error> {
        let tmp_dir = tempfile::TempDir::new().map_err(Error::IoError)?;
        let mut profile_builder = ProfileBuilder::new();
        profile_builder.path(tmp_dir.path());

        Ok((tmp_dir, profile_builder.build()))
    }

    fn tmp_shelf() -> Result<(tempfile::TempDir, Shelf), Error> {
        let tmp_dir = tempfile::TempDir::new().map_err(Error::IoError)?;
        let shelf = Shelf::from(tmp_dir.path())?;

        Ok((tmp_dir, shelf))
    }

    #[test]
    fn basic_profile_usage() -> Result<(), Error> {
        let (profile_tmp_dir, mut profile) = tmp_profile()?;
        let (_, mut shelf) = tmp_shelf()?;

        assert!(profile.export().is_ok());
        assert!(shelf.export().is_ok());

        let test_subjects =
            Subject::from_vec_loose(&vec!["Calculus", "Algebra", "Physics"], &shelf);
        let subject = test_subjects[0].clone();
        let test_notes = Note::from_vec_loose(
            &vec![
                "Introduction to Precalculus",
                "Introduction to Integrations",
                "Taylor Series",
                "Introduction to Limits",
                "Matrices and Markov Chains",
            ],
            &subject,
            &shelf,
        );

        let exported_subjects: Vec<Subject> = test_subjects
            .into_iter()
            .filter(|subject| subject.export(&shelf).is_ok())
            .collect();
        assert_eq!(exported_subjects.len(), 3);

        let exported_notes: Vec<Note> = test_notes
            .clone()
            .into_iter()
            .filter(|note| note.export((&subject.clone(), &shelf)).is_ok())
            .collect();
        assert_eq!(exported_notes.len(), 5);

        assert!(Profile::from(profile_tmp_dir).is_ok());

        Ok(())
    }

    #[test]
    fn basic_profile_template_usage() -> Result<(), Error> {
        let (tmp_dir, mut profile) = tmp_profile()?;
        profile.export()?;

        let mut note_template_file = fs::File::create(
            profile
                .templates_path()
                .join(format!("_default.{}", TEMPLATE_FILE_EXTENSION)),
        )
        .map_err(Error::IoError)?;
        note_template_file
            .write("LOL".as_bytes())
            .map_err(Error::IoError)?;

        let profile = Profile::from(tmp_dir.path())?;
        assert_eq!(
            profile
                .templates
                .render::<&str, toml::Value>("_default", toml::from_str("name = 'ME'").unwrap())?,
            "LOL".to_string()
        );

        Ok(())
    }

    #[test]
    #[should_panic]
    fn invalid_profile_export() {
        let test_path = PathBuf::from("./this/path/does/not/exists/");
        let mut test_profile_builder = ProfileBuilder::new();
        test_profile_builder.path(&test_path);

        let mut test_profile = test_profile_builder.build();

        assert!(test_profile.export().is_ok());
    }

    #[test]
    #[should_panic]
    fn invalid_profile_import() {
        let test_path = PathBuf::from("./this/path/also/does/not/exists/lol");
        assert!(Profile::from(test_path).is_ok(), "Profile is not valid.");
    }
}
