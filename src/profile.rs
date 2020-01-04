use std::collections::HashMap;
use std::fs::{self, DirBuilder};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml::{self, Value};

use crate::consts;
use crate::error::Error;
use crate::helpers;
use crate::items::{Note, Subject};
use crate::templates::{self, TemplateGetter, TemplateRegistry};
use crate::Result;

// profile constants
pub const PROFILE_METADATA_FILENAME: &str = ".profile.toml";
pub const PROFILE_TEMPLATE_FILES_DIR_NAME: &str = ".templates";

pub const PROFILE_NOTE_TEMPLATE_NAME: &str = "_default";
// TODO: Implement the master note templating system.
pub const PROFILE_MASTER_NOTE_TEMPLATE_NAME: &str = "_master";

/// A basic macro for modifying a TOML table.
macro_rules! modify_toml {
    ($var:ident, $( ($field:expr, $value:expr) ),*) => {
        let mut temp_table = $var.as_table_mut().unwrap();

        $(
            temp_table.insert(String::from($field), $value);
        )*
    };
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfileMetadata {
    name: String,
    version: String,

    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

impl ProfileMetadata {
    /// Create a new profile metadata instance.
    pub fn new() -> Self {
        Self {
            name: String::from("New Student"),
            version: String::from(consts::APP_VERSION),
            extra: HashMap::<String, Value>::new(),
        }
    }

    /// Create a profile metadata instance from a file.
    pub fn from_fs<P: AsRef<Path>>(path: P) -> Result<Self> {
        let metadata_file_string = fs::read_to_string(path).map_err(Error::IoError)?;
        let metadata_result: Self =
            toml::from_str(&metadata_file_string).map_err(Error::TomlValueError)?;

        Ok(metadata_result)
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn version(&self) -> &String {
        &self.version
    }

    pub fn extra(&self) -> &HashMap<String, Value> {
        &self.extra
    }

    /// Sets the name of the profile metadata.
    pub fn set_name(
        &mut self,
        name: String,
    ) -> &mut Self {
        self.name = name;
        self
    }

    /// Sets the version of the metadata.
    pub fn set_version(
        &mut self,
        version: String,
    ) -> &mut Self {
        self.version = version;
        self
    }

    /// Sets the extra metadata.
    pub fn set_extra(
        &mut self,
        extra: HashMap<String, Value>,
    ) -> &mut Self {
        self.extra = extra;
        self
    }

    /// Merge the extra metadata hashmap into the profile metadata instance.
    pub fn merge_extra(
        &mut self,
        new_extra: HashMap<String, Value>,
    ) -> &mut Self {
        for (key, value) in new_extra.into_iter() {
            self.extra.insert(key, value);
        }

        self
    }

    /// Export the metadata in the filesystem.
    pub fn export<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<()> {
        let path = path.as_ref();

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(path)
            .map_err(Error::IoError)?;
        let metadata_as_string =
            toml::to_string_pretty(&self).map_err(Error::TomlSerializeError)?;
        file.write_all(metadata_as_string.as_bytes())
            .map_err(Error::IoError)?;

        Ok(())
    }
}

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

        let mut metadata_instance = ProfileMetadata::new();

        if self.name.is_some() {
            let name = self.name.unwrap();

            metadata_instance.set_name(name);
        }

        if self.extra_metadata.is_some() {
            let extra = self.extra_metadata.unwrap();

            metadata_instance.set_extra(extra);
        }

        profile.metadata = metadata_instance;

        profile
    }
}

/// A profile holds certain metadata such as the templates.
pub struct Profile {
    path: PathBuf,
    metadata: ProfileMetadata,
    templates: templates::TemplateHandlebarsRegistry,
}

impl Profile {
    /// Creates a profile instance with empty data.
    pub fn new() -> Self {
        Self {
            path: PathBuf::new(),
            metadata: ProfileMetadata::new(),
            templates: templates::TemplateHandlebarsRegistry::new(),
        }
    }

    /// Opens an initiated profile.
    ///
    /// If the profile does not exist in the given path, it will cause an error.
    pub fn from<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path: PathBuf = path.as_ref().to_path_buf();

        let mut profile = Self::new();

        profile.path = fs::canonicalize(path).map_err(Error::IoError)?;
        profile.set_templates()?;
        profile.metadata = ProfileMetadata::from_fs(profile.metadata_path())?;

        Ok(profile)
    }

    pub fn metadata(&self) -> &ProfileMetadata {
        &self.metadata
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// Insert the contents of the files inside of the templates directory of the profile in the Handlebars registry.
    ///
    /// Take note it will only get the contents of the top-level files in the templates folder.
    pub fn set_templates(&mut self) -> Result<()> {
        if !self.has_templates() {
            return Err(Error::InvalidProfileError(self.path.clone()));
        }

        let mut registry = templates::TemplateHandlebarsRegistry::new();
        // registering with the default templates
        registry.register_template_string(PROFILE_NOTE_TEMPLATE_NAME, consts::NOTE_TEMPLATE)?;
        registry.register_template_string(
            PROFILE_MASTER_NOTE_TEMPLATE_NAME,
            consts::MASTER_NOTE_TEMPLATE,
        )?;

        let templates = TemplateGetter::get_templates(self.templates_path(), "*.tex")?;
        registry.register_vec(&templates)?;

        self.templates = registry;

        Ok(())
    }

    /// Returns a string from a Handlebars template.
    ///
    /// While creating the metadata from the profile, it will override certain fields
    /// like `title` (the title of the note) and `subject` (the name of the subject).
    pub fn return_string_from_note_template<S>(
        &self,
        subject: &Subject,
        note: &Note,
        template_name: &Option<S>,
    ) -> Result<String>
    where
        S: AsRef<str>,
    {
        let mut metadata: toml::Value = toml::Value::try_from(self.metadata.clone()).unwrap();
        let mut note_as_toml = toml::Value::try_from(note.clone()).unwrap();
        modify_toml! {note_as_toml,
            ("title", toml::Value::String(note.title())),
            ("file_name", toml::Value::String(note.file_name()))
        };

        modify_toml! {metadata,
            ("subject", toml::Value::try_from(subject.stem()).unwrap()),
            ("note", note_as_toml),
            ("date", toml::Value::String(chrono::Local::now().format("%F").to_string()))
        };

        let template_name = match template_name.as_ref() {
            Some(v) => v.as_ref(),
            None => PROFILE_NOTE_TEMPLATE_NAME,
        };

        self.templates.render(template_name, &metadata)
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
    pub fn get_metadata(&self) -> Result<ProfileMetadata> {
        if !self.is_valid() {
            return Err(Error::InvalidProfileError(self.path.clone()));
        }

        let metadata_file_string =
            fs::read_to_string(self.metadata_path()).map_err(Error::IoError)?;
        let metadata: ProfileMetadata =
            toml::from_str(&metadata_file_string).map_err(Error::TomlValueError)?;

        Ok(metadata)
    }

    /// Export the profile in the filesystem.
    pub fn export(&mut self) -> Result<()> {
        let dir_builder = DirBuilder::new();

        if self.is_valid() {
            return Err(Error::ProfileAlreadyExists(self.path()));
        }

        if !self.is_exported() {
            helpers::fs::create_folder(&dir_builder, self.path())?;
        }

        if !self.has_metadata() {
            self.metadata.export(self.metadata_path())?;
        }

        if !self.has_templates() {
            helpers::fs::create_folder(&dir_builder, self.templates_path())?;
        }

        Ok(())
    }

    /// Returns the command for compiling the notes.
    /// By default, the compilation command is `latexmk -pdf`.
    ///
    /// If there's no valid value found from the key (i.e., invalid type), it will return the default command.
    pub fn compile_note_command(&self) -> String {
        let PROFILE_DEFAULT_COMMAND = String::from("latexmk -pdf {{note}}");
        match self.metadata.extra.get("command").as_ref() {
            Some(value) => match value.is_str() {
                true => value.as_str().unwrap().to_string(),
                false => PROFILE_DEFAULT_COMMAND,
            },
            None => PROFILE_DEFAULT_COMMAND,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shelf::{ExportOptions, Shelf, ShelfBuilder};
    use crate::CompilationEnvironment;
    use tempfile;
    use toml;

    fn tmp_profile() -> Result<(tempfile::TempDir, Profile)> {
        let tmp_dir = tempfile::TempDir::new().map_err(Error::IoError)?;
        let mut profile_builder = ProfileBuilder::new();
        profile_builder.path(tmp_dir.path());

        Ok((tmp_dir, profile_builder.build()))
    }

    fn tmp_shelf() -> Result<(tempfile::TempDir, Shelf)> {
        let tmp_dir = tempfile::TempDir::new().map_err(Error::IoError)?;
        let mut shelf_builder = ShelfBuilder::new();
        shelf_builder.path(tmp_dir.path());

        Ok((tmp_dir, shelf_builder.build()?))
    }

    #[test]
    fn basic_profile_usage() -> Result<()> {
        let (profile_tmp_dir, mut profile) = tmp_profile()?;
        let (shelf_tmp_dir, mut shelf) = tmp_shelf()?;

        let export_options = ExportOptions::new();

        assert!(profile.export().is_ok());
        assert!(shelf.export().is_ok());

        let test_subjects =
            Subject::from_vec_loose(&vec!["Calculus", "Algebra", "Physics"], &shelf);
        let test_notes = Note::from_vec_loose(
            &vec![
                "Introduction to Precalculus",
                "Introduction to Integrations",
                "Taylor Series",
                "Introduction to Limits",
                "Matrices and Markov Chains",
            ],
            &test_subjects[0],
            &shelf,
        )?;
        let test_input = r"\documentclass[class=memoir, crop=false, oneside, 14pt]{standalone}

        % document metadata
        \author{ {{~author~}} }
        \title{ {{~title~}} }
        \date{ {{~date~}} }
        
        \begin{document}
        This is a content sample.
        \end{document}
        ";

        shelf.create_subjects(&test_subjects, &export_options);
        shelf.create_notes(&test_subjects[0], &test_notes, test_input, &export_options);

        let mut compilation_env = CompilationEnvironment::new();
        compilation_env
            .command(profile.compile_note_command())
            .notes(test_notes.clone())
            .subject(test_subjects[0].clone())
            .thread_count(4);
        assert_eq!(compilation_env.compile(&shelf)?.len(), 5);

        assert!(Profile::from(profile_tmp_dir).is_ok());

        Ok(())
    }

    #[ignore]
    #[test]
    fn basic_profile_usage_with_compilation_notes() -> Result<()> {
        let (profile_tmp_dir, mut profile) = tmp_profile()?;
        let (shelf_tmp_dir, mut shelf) = tmp_shelf()?;

        let export_options = ExportOptions::new();

        assert!(profile.export().is_ok());
        assert!(shelf.export().is_ok());

        let test_subjects =
            Subject::from_vec_loose(&vec!["Calculus", "Algebra", "Physics"], &shelf);
        let test_notes = Note::from_vec_loose(
            &vec![
                "Introduction to Precalculus",
                "Introduction to Integrations",
                "Taylor Series",
                "Introduction to Limits",
                "Matrices and Markov Chains",
            ],
            &test_subjects[0],
            &shelf,
        )?;
        let test_input = r"\documentclass[class=memoir, crop=false, oneside, 14pt]{standalone}

        % document metadata
        \author{ {{~author~}} }
        \title{ {{~title~}} }
        \date{ {{~date~}} }
        
        \begin{document}
        This is a content sample.
        \end{document}
        ";

        shelf.create_subjects(&test_subjects, &export_options);
        shelf.create_notes(&test_subjects[0], &test_notes, test_input, &export_options);

        let mut compilation_env = CompilationEnvironment::new();
        compilation_env
            .command(profile.compile_note_command())
            .notes(test_notes.clone())
            .subject(test_subjects[0].clone())
            .thread_count(4);
        assert_eq!(compilation_env.compile(&shelf)?.len(), 5);

        assert!(Profile::from(profile_tmp_dir).is_ok());

        Ok(())
    }

    #[test]
    fn basic_profile_template_usage() -> Result<()> {
        let (tmp_dir, mut profile) = tmp_profile()?;
        profile.export()?;

        let mut note_template_file =
            fs::File::create(profile.templates_path().join("_default.tex"))
                .map_err(Error::IoError)?;
        note_template_file
            .write("LOL".as_bytes())
            .map_err(Error::IoError)?;

        let mut profile = Profile::from(tmp_dir.path())?;
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
