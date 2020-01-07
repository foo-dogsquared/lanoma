use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::path::{self, PathBuf};
use std::str::FromStr;

use chrono::{self};
use serde::{Deserialize, Serialize};
use toml;

use crate::error::Error;
use crate::helpers;
use crate::items::Note;
use crate::shelf::{Shelf, ShelfItem};
use crate::Result;

const SUBJECT_METADATA_FILE: &str = "info.toml";
const MASTER_NOTE_FILE: &str = ".master.tex";

/// A subject where it can contain notes or other subjects.
///
/// In the filesystem, a subject is a folder with a specific metadata file (`info.json`).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Subject {
    name: String,
}

impl Subject {
    /// Creates a new subject instance.
    pub fn new() -> Self {
        Self {
            name: String::new(),
        }
    }

    /// Create a subject instance from the given string.
    /// Take note the input will be normalized for paths.
    ///
    /// # Example
    ///
    /// ```
    /// use texture_notes_v2::items::{Subject};
    ///
    /// assert_eq!(Subject::from("Mathematics").name(), Subject::from("Mathematics/Calculus/..").name())
    /// ```
    pub fn from<S>(name: S) -> Self
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
        let subject = Subject::from(name);
        if !subject.is_valid(&shelf) {
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
                    Err(_e) => Subject::from(subject.as_ref().to_string()),
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
        PathBuf::from(&self.name)
            .components()
            .into_iter()
            .map(|component| {
                let s = component.as_os_str().to_str().unwrap();

                match component {
                    path::Component::Normal(c) => helpers::string::kebab_case(s),
                    _ => s.to_string(),
                }
            })
            .collect()
    }

    /// Returns the associated path with the given shelf.
    pub fn path_in_shelf(
        &self,
        shelf: &Shelf,
    ) -> PathBuf {
        let mut path = shelf.path();
        path.push(self.path());

        path
    }

    /// Exports the instance in the filesystem.
    pub fn export(
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

    /// Returns the last subject component as a subject instance.
    pub fn stem(&self) -> Self {
        let mut subject = self.clone();
        subject.name = self.name();

        subject
    }

    /// Returns the modification datetime of the folder as a `chrono::DateTime` instance.
    pub fn datetime_modified(
        &self,
        shelf: &Shelf,
    ) -> Result<chrono::DateTime<chrono::Utc>> {
        match self.is_valid(&shelf) {
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
    pub fn get_metadata(
        &self,
        shelf: &Shelf,
    ) -> Result<toml::Value> {
        let metadata_path = self.metadata_path_in_shelf(&shelf);
        let metadata = fs::read_to_string(metadata_path).map_err(Error::IoError)?;

        toml::from_str(&metadata).map_err(Error::TomlValueError)
    }

    /// Deletes the associated folder in the shelf filesystem.
    pub fn delete(
        &self,
        notes: &Shelf,
    ) -> Result<()> {
        let path = self.path_in_shelf(&notes);
        fs::remove_dir_all(path).map_err(Error::IoError)
    }

    /// Checks if the associated path exists from the shelf.
    pub fn is_path_exists(
        &self,
        notes: &Shelf,
    ) -> bool {
        self.path_in_shelf(&notes).is_dir()
    }

    /// Checks if the subject has a valid folder structure from the shelf.
    pub fn is_valid(
        &self,
        shelf: &Shelf,
    ) -> bool {
        self.is_path_exists(&shelf)
    }

    /// Returns the `_file` metadata.
    /// If the key does not exist or invalid, it will return the default value.
    pub fn note_filter(
        &self,
        shelf: &Shelf,
    ) -> Vec<String> {
        self.get_metadata(&shelf)
            // If there is no metadata file, give the default files key.
            .unwrap_or(toml::Value::from_str("_files = ['*.tex']").unwrap())
            .get("_files")
            // If the metadata file is present but has no valid `_files` key.
            .unwrap_or(&toml::Value::try_from(["*.tex"]).unwrap())
            // At this point, it is guaranteed to be an array so it is safe to unwrap this.
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str())
            .filter(|value| value.is_some())
            .map(|value| value.unwrap().to_string())
            .collect()
    }

    /// Returns a vector of the parts of the subject.
    /// This does not check if each subject component is exported or valid.
    ///
    /// # Example
    ///
    /// ```
    /// use texture_notes_v2::items::{Subject};
    ///
    /// let subject = Subject::from("Bachelor I/Semester I/Calculus");
    ///
    /// let subjects = subject.split_subjects();
    /// let mut split_subjects = subjects.iter();
    ///
    /// assert_eq!(split_subjects.next().unwrap().name(), Subject::from("Bachelor I").name());
    /// assert_eq!(split_subjects.next().unwrap().name(), Subject::from("Bachelor I/Semester I").name());
    /// assert_eq!(split_subjects.next().unwrap().name(), Subject::from("Bachelor I/Semester I/Calculus").name());
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

                    Subject::from(item_path.to_str().unwrap())
                }
                None => Subject::from(component.as_os_str().to_str().unwrap()),
            };

            subjects.push(s);
        }

        subjects
    }

    pub fn create_master_note(&self) -> MasterNote {
        MasterNote {
            subject: self.clone(),
            notes: Vec::new(),
        }
    }
}

pub struct MasterNote<'a> {
    subject: Subject,
    notes: Vec<&'a Note>,
}

impl<'a> MasterNote<'a> {
    pub fn new() -> Self {
        Self {
            subject: Subject::new(),
            notes: Vec::new(),
        }
    }

    pub fn subject(&self) -> &Subject {
        &self.subject
    }

    pub fn notes(&self) -> &Vec<&'a Note> {
        &self.notes
    }

    pub fn push(
        &mut self,
        note: &'a Note,
    ) -> &mut Self {
        self.notes.push(&note);
        self
    }

    pub fn path(&self) -> PathBuf {
        let mut path = self.subject.path();
        path.push(MASTER_NOTE_FILE);

        path
    }

    pub fn path_in_shelf(
        &self,
        shelf: &Shelf,
    ) -> PathBuf {
        let mut path = self.subject.path_in_shelf(&shelf);
        path.push(MASTER_NOTE_FILE);

        path
    }

    pub fn export<S>(
        &self,
        shelf: &Shelf,
        template: S,
    ) -> Result<()>
    where
        S: AsRef<str>,
    {
        let template = template.as_ref();
        let master_note_path = self.path_in_shelf(&shelf);
        let mut master_note_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(master_note_path)
            .map_err(Error::IoError)?;

        master_note_file
            .write(template.as_bytes())
            .map_err(Error::IoError)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_subject() {
        let subject = Subject::from("Calculus");

        assert_eq!(subject.path(), PathBuf::from("calculus"));
        assert_eq!(subject.name(), String::from("Calculus"));

        let subject_fragments = subject.split_subjects();
        let mut subject_part = subject_fragments.iter();
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::from("Calculus").name
        );
    }

    #[test]
    fn subject_with_multiple_path() {
        let subject = Subject::from("Mathematics/Calculus/");

        assert_eq!(subject.path(), PathBuf::from("mathematics/calculus/"));
        assert_eq!(subject.name(), String::from("Calculus"));

        let subject_fragments = subject.split_subjects();
        let mut subject_part = subject_fragments.iter();
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::from("Mathematics").name
        );
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::from("Mathematics/Calculus").name
        );
    }

    #[test]
    fn subject_with_multiple_path_and_space() {
        let subject = Subject::from("Calculus/Calculus I");

        assert_eq!(subject.path(), PathBuf::from("calculus/calculus-i"));
        assert_eq!(subject.name(), String::from("Calculus I"));

        let subject_fragments = subject.split_subjects();
        let mut subject_part = subject_fragments.iter();
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::from("Calculus").name
        );
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::from("Calculus/Calculus I").name
        );
    }

    #[test]
    fn subject_with_multiple_path_and_improper_input() {
        let subject = Subject::from("Bachelor I/Semester I/Quantum Mechanics/../.");

        assert_eq!(subject.path(), PathBuf::from("bachelor-i/semester-i/"));
        assert_eq!(subject.name(), String::from("Semester I"));

        let subject_fragments = subject.split_subjects();
        let mut subject_part = subject_fragments.iter();
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::from("Bachelor I").name
        );
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::from("Bachelor I/Semester I").name
        );
    }

    #[test]
    fn subject_with_multiple_path_and_improper_input_and_leading_stars() {
        let subject = Subject::from("Bachelor I/Semester I/Quantum Mechanics/../.Logs");

        assert_eq!(subject.path(), PathBuf::from("bachelor-i/semester-i/logs"));
        assert_eq!(subject.name(), String::from(".Logs"));

        let subject_fragments = subject.split_subjects();
        let mut subject_part = subject_fragments.iter();
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::from("Bachelor I/").name
        );
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::from("Bachelor I/Semester I").name
        );
        assert_eq!(
            subject_part.next().unwrap().name,
            Subject::from("Bachelor I/Semester I/.Logs").name
        );
    }

    #[test]
    fn basic_note() {
        let subject = Subject::from("Calculus");
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
        let subject = Subject::from("First Year/Semester I/Calculus");
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
