use std::collections::HashMap;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::result::Result;

use chrono::{self};
use serde::{Deserialize, Serialize};
use toml;

use crate::error::Error;
use crate::helpers;
use crate::shelf::Shelf;

const SUBJECT_METADATA_FILE: &str = "info.toml";

/// A subject where it can contain notes or other subjects.
///
/// In the filesystem, a subject is a folder with a specific metadata file (`info.json`).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Subject {
    name: String,

    #[serde(flatten)]
    extra: HashMap<String, toml::Value>,
}

impl Subject {
    /// Creates a new subject instance.
    pub fn new() -> Self {
        Self {
            name: String::new(),
            extra: HashMap::new(),
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
            extra: HashMap::new(),
        }
    }

    /// Create a subject instance from a given notes instance.
    /// If the path is a valid subject folder, it will set the appropriate data from the metadata file and return with an `Option` field.
    pub fn from_shelf(
        name: &str,
        shelf: &Shelf,
    ) -> Result<Self, Error> {
        let mut subject = Subject::from(name);
        println!("{:?}", subject.clone());
        if !subject.is_valid(&shelf) {
            return Err(Error::InvalidSubjectError(subject.path_in_shelf(&shelf)));
        }

        if subject.has_metadata_file(&shelf) {
            let metadata_path = subject.metadata_path_in_shelf(&shelf);
            let metadata = fs::read_to_string(metadata_path).map_err(Error::IoError)?;

            // changing the name of the TOML value to the referred name since it contains the TOML value as a stem.
            subject = toml::from_str(&metadata).map_err(Error::TomlValueError)?;
            subject.name = name.to_string();
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
    ) -> Result<chrono::DateTime<chrono::Utc>, Error> {
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

    /// Returns the subject path.
    pub fn path(&self) -> PathBuf {
        PathBuf::from(&self.name)
            .components()
            .into_iter()
            .map(|component| helpers::string::kebab_case(component.as_os_str().to_str().unwrap()))
            .collect()
    }

    /// Returns the associated path with the given shelf.
    pub fn path_in_shelf(
        &self,
        notes: &Shelf,
    ) -> PathBuf {
        let mut path = notes.path();
        path.push(self.path());

        path
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

    /// Exports the instance in the filesystem.
    pub fn export(
        &self,
        shelf: &Shelf,
        strict: bool,
    ) -> Result<(), Error> {
        if !shelf.is_valid() {
            return Err(Error::UnexportedShelfError(shelf.path()));
        }

        let path = self.path_in_shelf(&shelf);
        let dir_builder = DirBuilder::new();

        if !self.is_path_exists(&shelf) {
            helpers::fs::create_folder(&dir_builder, &path)?;
        }

        let metadata_path = self.metadata_path_in_shelf(&shelf);
        let mut metadata_file_options = OpenOptions::new();
        metadata_file_options.write(true);

        if !self.has_metadata_file(&shelf) || strict {
            metadata_file_options.create_new(true);
        } else {
            metadata_file_options.truncate(true);
        }

        let mut metadata_file = metadata_file_options
            .open(metadata_path)
            .map_err(Error::IoError)?;
        metadata_file
            .write(
                toml::to_string_pretty(&self.stem())
                    .map_err(Error::TomlSerializeError)?
                    .as_bytes(),
            )
            .map_err(Error::IoError)?;

        Ok(())
    }

    /// Deletes the associated folder in the shelf filesystem.
    pub fn delete(
        &self,
        notes: &Shelf,
    ) -> Result<(), Error> {
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

    /// Returns the valid subjects in the shelf filesystem sorted by the modification datetime.
    pub fn sort_by_date(
        shelf: &Shelf,
        subjects: &Vec<Subject>,
    ) -> Vec<Subject> {
        if !shelf.is_valid() {
            return vec![];
        }

        let mut valid_subjects: Vec<Subject> = subjects
            .iter()
            .filter(|subject| subject.is_valid(&shelf))
            .cloned()
            .collect();

        valid_subjects.sort_unstable_by(|a, b| {
            let a = match a.datetime_modified(&shelf) {
                Ok(v) => v,
                Err(_e) => chrono::MIN_DATE.and_hms(0, 0, 0),
            };
            let b = match b.datetime_modified(&shelf) {
                Ok(v) => v,
                Err(_e) => chrono::MIN_DATE.and_hms(0, 0, 0),
            };

            a.partial_cmp(&b).unwrap()
        });

        valid_subjects
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
}

/// The individual LaTeX documents in a notes instance.
///
/// Unlike subjects, there are no prerequisites for a note.
/// Though certain processes (i.e., compilation) will require the note to be exported in the filesystem.
///
/// Because of the nature of the program (and filesystems, in general), all note instances does not have the parent object.
/// Thus, its methods constantly require the parent object as one of the parameters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    title: String,
}

impl Note {
    /// Creates a new note instance.
    pub fn new<S>(title: S) -> Self
    where
        S: AsRef<str>,
    {
        Self {
            title: title.as_ref().to_string(),
        }
    }

    /// Searches for the note in the shelf.
    ///
    /// This only checks whether the associated path of the note exists.
    /// To check if the note exists on the notes database, call the `Note::is_entry_exists` method.
    pub fn from<S: AsRef<str>>(
        title: S,
        subject: &Subject,
        notes: &Shelf,
    ) -> Result<Option<Self>, Error> {
        let title = title.as_ref();
        let note = Note::new(title.to_string());

        match note.is_path_exists(&subject, &notes) {
            true => Ok(Some(note)),
            false => Ok(None),
        }
    }

    /// Similar to the `from` method, only on a bigger scale.
    pub fn from_vec<S: AsRef<str>>(
        note_titles: &Vec<S>,
        subject: &Subject,
        notes: &Shelf,
    ) -> Result<Vec<Option<Self>>, Error> {
        let mut notes_vector: Vec<Option<Self>> = vec![];

        for title in note_titles.iter() {
            let note_title = title.as_ref();

            notes_vector.push(Note::from(note_title, &subject, &notes)?);
        }

        Ok(notes_vector)
    }

    /// Searches for the specified notes in the shelf.
    /// If there is no associated note found in the shelf, it will instead create one.
    /// Making the return data creates a guaranteed vector of note instances.
    pub fn from_vec_loose<S: AsRef<str>>(
        note_titles: &Vec<S>,
        subject: &Subject,
        notes: &Shelf,
    ) -> Result<Vec<Self>, Error> {
        let mut notes_vector: Vec<Self> = vec![];

        for title in note_titles.iter() {
            let title = title.as_ref();
            let note_instance = match Note::from(title, &subject, &notes)? {
                Some(v) => v,
                None => Note::new(title.to_string()),
            };

            notes_vector.push(note_instance);
        }

        Ok(notes_vector)
    }

    /// Returns the title of the note instance.
    pub fn title(&self) -> String {
        self.title.clone()
    }

    /// Returns the modification datetime of the note file in the shelf filesystem as a `chrono::DateTime` instance.
    pub fn datetime_modified(
        &self,
        subject: &Subject,
        shelf: &Shelf,
    ) -> Result<chrono::DateTime<chrono::Utc>, Error> {
        match self.is_path_exists(&subject, &shelf) {
            true => {
                let metadata =
                    fs::metadata(self.path_in_shelf(&subject, &shelf)).map_err(Error::IoError)?;
                let modification_time = metadata.modified().map_err(Error::IoError)?;

                Ok(chrono::DateTime::<chrono::Utc>::from(modification_time))
            }
            false => Err(Error::IoError(io::Error::from(io::ErrorKind::NotFound))),
        }
    }

    /// Returns the path of the note instance along with its associated subject.
    ///
    /// It does not necessarily mean that the note exists.
    /// Be sure to check it first.
    pub fn path_in_shelf(
        &self,
        subject: &Subject,
        notes: &Shelf,
    ) -> PathBuf {
        let mut path = subject.path_in_shelf(&notes);
        path.push(self.file_name());

        path
    }

    /// Returns the path of the note relative to the subject.
    pub fn path(
        &self,
        subject: &Subject,
    ) -> PathBuf {
        let mut path = subject.path();
        path.push(self.file_name());

        path
    }

    /// Returns the file name of the note.
    pub fn file_name(&self) -> String {
        let mut slug = helpers::string::kebab_case(&self.title);
        slug.push_str(".tex");

        slug
    }

    /// Writes the resulting LaTeX file in the filesystem.
    ///
    /// For templating, it uses [a Rust implementation of Handlebars](https://github.com/sunng87/handlebars-rust).
    /// The configuration of Handlebars does not escape anything (uses [`handlebars::no_escape`](https://docs.rs/handlebars/3.0.0-beta.1/handlebars/fn.no_escape.html)).
    pub fn export(
        &self,
        subject: &Subject,
        notes: &Shelf,
        template: &str,
        strict: bool,
    ) -> Result<(), Error> {
        if !notes.is_valid() {
            return Err(Error::UnexportedShelfError(notes.path()));
        }

        let path = self.path_in_shelf(&subject, &notes);
        let mut note_file_open_options = OpenOptions::new();
        note_file_open_options.write(true);

        // the `create_new` option will give an error if the file already exists
        // so this option is suitable for file checking in strict mode
        if !self.is_path_exists(&subject, &notes) || strict {
            note_file_open_options.create_new(true);
        } else {
            note_file_open_options.truncate(true);
        }

        let mut note_file = note_file_open_options.open(path).map_err(Error::IoError)?;

        note_file
            .write(template.as_bytes())
            .map_err(Error::IoError)?;

        Ok(())
    }

    /// Simply deletes the file in the shelf filesystem.
    ///
    /// This does not delete the entry in the notes database.
    pub fn delete(
        &self,
        subject: &Subject,
        notes: &Shelf,
    ) -> Result<(), Error> {
        let path = self.path_in_shelf(&subject, &notes);

        fs::remove_file(path).map_err(Error::IoError)
    }

    /// Checks for the file if it exists in the shelf.
    pub fn is_path_exists(
        &self,
        subject: &Subject,
        notes: &Shelf,
    ) -> bool {
        self.path_in_shelf(&subject, &notes).is_file()
    }

    /// Returns all of the notes that are in the shelf filesystem sorted by its modification datetime.
    pub fn sort_by_date(
        shelf: &Shelf,
        subject: &Subject,
        notes: &Vec<Note>,
    ) -> Vec<Note> {
        if !subject.is_path_exists(&shelf) {
            return vec![];
        }

        let mut valid_notes: Vec<Note> = notes
            .iter()
            .filter(|note| note.is_path_exists(&subject, &shelf))
            .cloned()
            .collect();

        valid_notes.sort_unstable_by(|a, b| {
            let a: chrono::DateTime<chrono::Utc> = match a.datetime_modified(&subject, &shelf) {
                Ok(v) => v,
                Err(_e) => chrono::MIN_DATE.and_hms(0, 0, 0),
            };
            let b = match b.datetime_modified(&subject, &shelf) {
                Ok(v) => v,
                Err(_e) => chrono::MIN_DATE.and_hms(0, 0, 0),
            };

            a.partial_cmp(&b).unwrap()
        });

        valid_notes
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
