use std::collections::HashMap;
use std::fs::{self, DirBuilder};
use std::io;
use std::path::{self, PathBuf};

use chrono::{self};
use serde::{Deserialize, Serialize};
use toml;

use crate::config;
use crate::error::Error;
use crate::helpers;
use crate::note::Note;
use crate::shelf::{Shelf, ShelfData, ShelfItem};
use crate::{Object, Result};

#[macro_use]
use crate::{modify_toml_table, upsert_toml_table};

const SUBJECT_METADATA_FILE: &str = "info.toml";

/// A subject where it can contain notes or other subjects.
///
/// In the filesystem, a subject is a folder with a specific metadata file (`info.json`).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Subject {
    name: String,
}

impl Object for Subject {
    fn data(&self) -> toml::Value {
        let mut subject_as_toml = toml::Value::from(HashMap::<String, toml::Value>::new());
        modify_toml_table! {subject_as_toml,
            ("name", self.name()),
            ("_slug", helpers::string::kebab_case(&self.name())),
            ("_path", self.path())
        };

        subject_as_toml
    }
}

impl AsRef<str> for Subject {
    fn as_ref(&self) -> &str {
        self.full_name().as_ref()
    }
}

impl ShelfData<&Shelf> for Subject {
    fn data(
        &self,
        shelf: &Shelf,
    ) -> toml::Value {
        let mut subject_as_toml = match self.get_config(&shelf) {
            Ok(v) => toml::Value::try_from(v).unwrap(),
            Err(_e) => toml::Value::from(HashMap::<String, toml::Value>::new()),
        };

        let subject_path = self.path_in_shelf(&shelf);
        upsert_toml_table! {subject_as_toml,
            ("name", self.name())
        };
        modify_toml_table! {subject_as_toml,
            ("_slug", helpers::string::kebab_case(&self.name())),
            ("_path", subject_path.clone()),
            ("_relpath_to_shelf", helpers::fs::relative_path_from(&shelf.path(), subject_path.clone()).unwrap().to_str().unwrap()),
            ("_relpath_from_shelf", helpers::fs::relative_path_from(subject_path, &shelf.path()).unwrap().to_str().unwrap())
        };

        subject_as_toml
    }
}

impl ShelfItem<&Shelf> for Subject {
    /// Returns the associated path with the given shelf.
    fn path_in_shelf(
        &self,
        shelf: &Shelf,
    ) -> PathBuf {
        let mut path = shelf.path();
        path.push(self.path());

        path
    }

    /// Checks if the associated path exists from the shelf.
    fn is_path_exists(
        &self,
        shelf: &Shelf,
    ) -> bool {
        self.path_in_shelf(&shelf).is_dir()
    }

    /// Exports the instance in the filesystem.
    fn export(
        &self,
        shelf: &Shelf,
    ) -> Result<()> {
        if !shelf.is_valid() {
            return Err(Error::UnexportedShelfError(shelf.path()));
        }

        let path = self.path_in_shelf(&shelf);
        let dir_builder = DirBuilder::new();

        if !self.is_path_exists(&shelf) {
            helpers::fs::create_folder(&dir_builder, &path)?;
        }

        Ok(())
    }

    /// Deletes the associated folder in the shelf filesystem.
    fn delete(
        &self,
        shelf: &Shelf,
    ) -> Result<()> {
        let path = self.path_in_shelf(&shelf);
        fs::remove_dir_all(path).map_err(Error::IoError)
    }
}

impl Subject {
    /// Create a subject instance with the given string.
    /// Take note the input will be normalized for paths.
    ///
    /// # Example
    ///
    /// ```
    /// use texture_notes_v2::subjects::Subject;
    ///
    /// assert_eq!(Subject::new("Mathematics").name(), Subject::new("Mathematics/Calculus/..").name())
    /// ```
    pub fn new<S>(name: S) -> Self
    where
        S: AsRef<str>,
    {
        let name = name.as_ref();
        let path: PathBuf = helpers::fs::naively_normalize_path(name.to_string())
            .unwrap()
            .components()
            .into_iter()
            .map(|component| component.as_os_str().to_str().unwrap().trim().to_string())
            .collect();
        Self {
            name: path.to_str().unwrap().to_string(),
        }
    }

    /// Create a subject instance from a given notes instance.
    /// If the path is a valid subject folder, it will set the appropriate data from the metadata file and return with an `Option` field.
    pub fn from_shelf(
        name: &str,
        shelf: &Shelf,
    ) -> Result<Self> {
        let subject = Subject::new(name);
        if !subject.is_path_exists(&shelf) {
            return Err(Error::InvalidSubjectError(subject.path_in_shelf(&shelf)));
        }

        Ok(subject)
    }

    /// Searches for the subjects in the given shelf.
    pub fn from_vec<P: AsRef<str>>(
        subjects: &Vec<P>,
        shelf: &Shelf,
    ) -> Vec<Self> {
        subjects
            .iter()
            .map(|subject| Subject::from_shelf(subject.as_ref(), &shelf))
            .filter(|subject_result| subject_result.is_ok())
            .map(|subject_result| subject_result.unwrap())
            .collect()
    }

    /// Searches for the subjects in the given shelf filesystem.
    ///
    /// All nonexistent subjects are created as a new subject instance instead.
    /// Though, this loses the indication whether the subject is on the shelf.
    pub fn from_vec_loose<P: AsRef<str>>(
        subjects: &Vec<P>,
        notes: &Shelf,
    ) -> Vec<Self> {
        subjects
            .iter()
            .map(
                |subject| match Subject::from_shelf(subject.as_ref(), &notes) {
                    Ok(v) => v,
                    Err(_e) => Subject::new(subject.as_ref().to_string()),
                },
            )
            .collect()
    }

    /// Returns the full name (with the parent folders) of the subject.
    pub fn full_name(&self) -> &String {
        &self.name
    }

    /// Returns the name of the subject.
    pub fn name(&self) -> String {
        PathBuf::from(&self.name)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    }

    /// Returns the subject path.
    pub fn path(&self) -> PathBuf {
        PathBuf::from(&self.full_name())
            .components()
            .into_iter()
            .map(|component| {
                let s = component.as_os_str().to_str().unwrap();

                match component {
                    path::Component::Normal(c) => helpers::string::kebab_case(c.to_str().unwrap()),
                    _ => s.to_string(),
                }
            })
            .collect()
    }

    /// Returns the last subject component as a subject instance.
    pub fn stem(&self) -> Self {
        Self::new(self.name())
    }

    /// Returns the modification datetime of the folder as a `chrono::DateTime` instance.
    pub fn datetime_modified(
        &self,
        shelf: &Shelf,
    ) -> Result<chrono::DateTime<chrono::Utc>> {
        match self.is_path_exists(&shelf) {
            true => {
                let metadata = fs::metadata(self.path_in_shelf(&shelf)).map_err(Error::IoError)?;
                let modification_systemtime = metadata.modified().map_err(Error::IoError)?;

                Ok(chrono::DateTime::<chrono::Utc>::from(
                    modification_systemtime,
                ))
            }
            false => Err(Error::IoError(io::Error::from(io::ErrorKind::Other))),
        }
    }

    /// Returns the associated metadata file path with the given shelf.
    pub fn metadata_path(&self) -> PathBuf {
        let mut path = self.path();
        path.push(SUBJECT_METADATA_FILE);

        path
    }

    /// A quick method for returning the metadata path associated with a shelf.
    pub fn metadata_path_in_shelf(
        &self,
        shelf: &Shelf,
    ) -> PathBuf {
        let mut path = self.path_in_shelf(&shelf);
        path.push(SUBJECT_METADATA_FILE);

        path
    }

    /// Checks if the metadata file exists in the shelf.
    pub fn has_metadata_file(
        &self,
        shelf: &Shelf,
    ) -> bool {
        self.metadata_path_in_shelf(&shelf).is_file()
    }

    /// Extract the metadata file as a subject instance.
    pub fn get_config(
        &self,
        shelf: &Shelf,
    ) -> Result<config::SubjectConfig> {
        let metadata_path = self.metadata_path_in_shelf(&shelf);
        let metadata = fs::read_to_string(metadata_path).map_err(Error::IoError)?;

        toml::from_str(&metadata).map_err(Error::TomlValueError)
    }

    /// Returns a vector of the parts of the subject.
    /// This does not check if each subject component is exported or valid.
    ///
    /// # Example
    ///
    /// ```
    /// use texture_notes_v2::subjects::Subject;
    ///
    /// let subject = Subject::new("Bachelor I/Semester I/Calculus");
    ///
    /// let subjects = subject.split_subjects();
    /// let mut split_subjects = subjects.iter();
    ///
    /// assert_eq!(split_subjects.next().unwrap().name(), Subject::new("Bachelor I").name());
    /// assert_eq!(split_subjects.next().unwrap().name(), Subject::new("Bachelor I/Semester I").name());
    /// assert_eq!(split_subjects.next().unwrap().name(), Subject::new("Bachelor I/Semester I/Calculus").name());
    /// assert!(split_subjects.next().is_none());
    /// ```
    pub fn split_subjects(&self) -> Vec<Self> {
        let mut subjects: Vec<Self> = vec![];

        let path = PathBuf::from(&self.name);
        for component in path.components() {
            let s = match subjects.iter().last() {
                Some(item) => {
                    let mut item_path = PathBuf::from(&item.name);
                    item_path.push(component.as_os_str());

                    Subject::new(item_path.to_str().unwrap())
                }
                None => Subject::new(component.as_os_str().to_str().unwrap()),
            };

            subjects.push(s);
        }

        subjects
    }

    /// Get the notes in the shelf filesystem.
    pub fn get_notes_in_fs(
        &self,
        file_globs: &Vec<String>,
        shelf: &Shelf,
    ) -> Result<Vec<Note>> {
        let mut notes: Vec<Note> = vec![];

        let subject_path = self.path_in_shelf(&shelf);

        let tex_files = globwalk::GlobWalkerBuilder::from_patterns(subject_path, &file_globs)
            .build()
            .map_err(Error::GlobParsingError)?;

        for file in tex_files {
            if let Ok(file) = file {
                let note_path = file.path();

                let file_stem = note_path.file_stem().unwrap().to_string_lossy();

                // All of the notes may not have a kebab-case as their file name so we have to check it if it's a valid note.
                match Note::from(file_stem, &self, &shelf) {
                    Some(v) => notes.push(v),
                    None => continue,
                }
            }
        }

        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_subject() {
        let subject = Subject::new("Calculus");

        assert_eq!(subject.path(), PathBuf::from("calculus"));
        assert_eq!(subject.name(), String::from("Calculus"));

        let subject_fragments = subject.split_subjects();
        let mut subject_part = subject_fragments.iter();
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("Calculus").name
        );
    }

    #[test]
    fn subject_with_multiple_path() {
        let subject = Subject::new("Mathematics/Calculus/");

        assert_eq!(subject.path(), PathBuf::from("mathematics/calculus/"));
        assert_eq!(subject.name(), String::from("Calculus"));

        let subject_fragments = subject.split_subjects();
        let mut subject_part = subject_fragments.iter();
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("Mathematics").name
        );
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("Mathematics/Calculus").name
        );
    }

    #[test]
    fn subject_with_multiple_path_and_space() {
        let subject = Subject::new("Calculus/Calculus I");

        assert_eq!(subject.path(), PathBuf::from("calculus/calculus-i"));
        assert_eq!(subject.name(), String::from("Calculus I"));

        let subject_fragments = subject.split_subjects();
        let mut subject_part = subject_fragments.iter();
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("Calculus").name
        );
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("Calculus/Calculus I").name
        );
    }

    #[test]
    fn subject_with_multiple_path_and_improper_input() {
        let subject = Subject::new("Bachelor I/Semester I/Quantum Mechanics/../.");

        assert_eq!(subject.path(), PathBuf::from("bachelor-i/semester-i/"));
        assert_eq!(subject.name(), String::from("Semester I"));

        let subject_fragments = subject.split_subjects();
        let mut subject_part = subject_fragments.iter();
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("Bachelor I").name
        );
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("Bachelor I/Semester I").name
        );
    }

    #[test]
    fn subject_with_multiple_path_and_improper_input_and_leading_stars() {
        let subject = Subject::new("Bachelor I/Semester I/Quantum Mechanics/../.Logs");

        assert_eq!(subject.path(), PathBuf::from("bachelor-i/semester-i/logs"));
        assert_eq!(subject.name(), String::from(".Logs"));

        let subject_fragments = subject.split_subjects();
        let mut subject_part = subject_fragments.iter();
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("Bachelor I/").name
        );
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("Bachelor I/Semester I").name
        );
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("Bachelor I/Semester I/.Logs").name
        );
    }

    #[test]
    fn subject_with_parent_dir() {
        let subject = Subject::new("../University/Year 1/Semester 1/Computer Engineering");

        assert_eq!(subject.name(), String::from("Computer Engineering"));
        assert_eq!(
            subject.path(),
            PathBuf::from("../university/year-1/semester-1/computer-engineering")
        );

        let subjects = subject.split_subjects();
        let mut subject_part = subjects.iter();

        assert_eq!(subject_part.next().unwrap().name, Subject::new("..").name);
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("../University").name
        );
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("../University/Year 1").name
        );
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("../University/Year 1/Semester 1").name
        );
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::new("../University/Year 1/Semester 1/Computer Engineering").name
        );
        assert!(subject_part.next().is_none());
    }

    #[test]
    fn basic_note() {
        let subject = Subject::new("Calculus");
        let note = Note::new("An introduction to calculus concepts");

        assert_eq!(
            note.file_name(),
            "an-introduction-to-calculus-concepts.tex".to_string()
        );

        assert_eq!(
            note.path(&subject),
            PathBuf::from("calculus/an-introduction-to-calculus-concepts.tex")
        );
    }

    #[test]
    fn note_and_subject_with_multiple_path() {
        let subject = Subject::new("First Year/Semester I/Calculus");
        let note = Note::new("An introduction to calculus concepts");

        assert_eq!(
            note.file_name(),
            "an-introduction-to-calculus-concepts.tex".to_string()
        );

        assert_eq!(
            note.path(&subject),
            PathBuf::from(
                "first-year/semester-i/calculus/an-introduction-to-calculus-concepts.tex"
            )
        );
    }
}
