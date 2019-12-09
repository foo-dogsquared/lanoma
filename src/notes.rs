use std::collections::HashMap;
use std::fs::{ self, DirBuilder, OpenOptions };
use std::path::{ self, PathBuf };
use std::result::Result;
use std::io::{ self, Write };

use chrono::{ self };
use serde::{ Deserialize, Serialize };
use serde_json;

use crate::helpers;
use crate::error::Error;
use crate::shelf::Shelf;

const SUBJECT_METADATA_FILE: &str = "info.json";

/// A subject where it can contain notes or other subjects. 
/// 
/// In the filesystem, a subject is a folder with a specific metadata file (`info.json`). 
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Subject {
    name: String, 

    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>, 
}

impl Subject {
    /// Creates a new subject instance. 
    pub fn new (name: String) -> Self {
        Subject {
            name, 
            extra: HashMap::new()
        }
    }

    /// Create a subject instance from a given notes instance. 
    /// If the path is a valid subject folder, it will set the appropriate data from the metadata file and return with an `Option` field. 
    pub fn from (name: &str, notes: &Shelf) -> Result<Option<Self>, Error> {
        let subject = Subject::new(name.to_string());
        
        if !subject.is_valid(&notes) {
            return Ok(None)
        }

        let metadata_path = subject.metadata_path_in_shelf(&notes);
        let metadata = fs::read_to_string(metadata_path).map_err(Error::IoError)?;
        
        let subject: Subject = serde_json::from_str(&metadata).map_err(Error::SerdeValueError)?;
        
        Ok(Some(subject))
    }
    
    /// Searches for the subjects in the given shelf. 
    pub fn from_vec<P: AsRef<str>> (subjects: &Vec<P>, notes: &Shelf) -> Result<Vec<Option<Self>>, Error> {
        let mut subjects_result: Vec<Option<Self>> = vec![];

        for subject in subjects.iter() {
            let name = subject.as_ref();
            subjects_result.push(Subject::from(&name, &notes)?)
        }

        Ok(subjects_result)
    }

    /// Searches for the subjects in the given shelf filesystem. 
    /// 
    /// All nonexistent subjects are created as a new subject instance instead. 
    /// Though, this loses the indication whether the subject is on the shelf. 
    pub fn from_vec_loose<P: AsRef<str>> (subjects: &Vec<P>, notes: &Shelf) -> Result<Vec<Self>, Error> {
        let mut subjects_vector: Vec<Self> = vec![];

        for subject in subjects.iter() {
            let name = subject.as_ref();
            let subject_instance = match Subject::from(&name, &notes)? {
                Some(s) => s, 
                None => Subject::new(name.to_string())
            };

            subjects_vector.push(subject_instance);
        }

        Ok(subjects_vector)
    }
    
    /// Returns the name of the subject. 
    pub fn name (&self) -> String {
        self.name.clone()
    }

    /// Returns the modification datetime of the folder as a `chrono::DateTime` instance. 
    pub fn datetime_modified (&self, notes: &Shelf) -> Result<chrono::DateTime<chrono::Local>, Error> {
        match self.is_path_exists(&notes) {
            true => {
                let metadata = fs::metadata(self.path_in_shelf(&notes)).map_err(Error::IoError)?;
                let modification_systemtime = metadata.modified().map_err(Error::IoError)?;

                Ok(chrono::DateTime::<chrono::Local>::from(modification_systemtime))
            }, 
            false => Err(Error::IoError(io::Error::from(io::ErrorKind::Other))) 
        }
    }

    /// Returns the associated path with the given shelf.
    pub fn path_in_shelf (&self, notes: &Shelf) -> PathBuf {
        let mut path = notes.path();
        let subject_slug = helpers::string::kebab_case(&self.name);
        path.push(subject_slug);

        path
    }

    /// Returns the path starting with its own. 
    pub fn path (&self) -> PathBuf {
        PathBuf::from(helpers::string::kebab_case(&self.name))
    }

    /// Returns the associated metadata file path with the given shelf. 
    pub fn metadata_path (&self) -> PathBuf {
        let mut path = self.path();
        path.push(SUBJECT_METADATA_FILE);

        path
    }

    /// A quick method for returning the metadata path associated with a shelf. 
    pub fn metadata_path_in_shelf (&self, shelf: &Shelf) -> PathBuf {
        let mut path = self.path_in_shelf(&shelf);
        path.push(SUBJECT_METADATA_FILE);
        
        path
    }

    /// Exports the instance in the filesystem. 
    pub fn export(&self, notes: &Shelf) -> Result<(), Error> {
        if !notes.is_exported() {
            return Err(Error::UnexportedShelfError(notes.path()));
        }
        
        let path = self.path_in_shelf(&notes);
        let dir_builder = DirBuilder::new();
        
        helpers::filesystem::create_folder(&dir_builder, &path)?;
        
        let metadata_path = self.metadata_path_in_shelf(&notes);
        let mut metadata_file = OpenOptions::new().create_new(true).write(true).open(metadata_path).map_err(Error::IoError)?;
        metadata_file.write(serde_json::to_string_pretty(&self).map_err(Error::SerdeValueError)?.as_bytes()).map_err(Error::IoError)?;

        Ok(())
    }

    /// Deletes the associated folder in the shelf filesystem. 
    pub fn delete(&self, notes: &Shelf) -> Result<(), Error> {
        let path = self.path_in_shelf(&notes);
        
        fs::remove_dir_all(path).map_err(Error::IoError)
    }

    /// Checks if the associated path exists from the shelf. 
    pub fn is_path_exists (&self, notes: &Shelf) -> bool {
        self.path_in_shelf(&notes).exists()
    }

    /// Checks if the subject has a valid folder structure from the shelf. 
    pub fn is_valid (&self, notes: &Shelf) -> bool {
        self.metadata_path_in_shelf(&notes).is_file()
    }
    
    /// Checks if the subject instance has an entry in the shelf database. 
    pub fn is_entry_exists (&self, notes: &Shelf) -> Result<bool, Error> {
        let id = notes.get_subject_id(&self)?;

        match id {
            Some(_id) => Ok(true), 
            None => Ok(false), 
        }
    }

    /// Checks if the subject instance is present in the filesystem and database in the shelf. 
    pub fn is_sync (&self, notes: &Shelf) -> bool {
        self.is_valid(&notes) && match self.is_entry_exists(&notes) {
            Ok(v) => v, 
            Err(_e) => false
        }
    }
}

/// The individual LaTeX documents in a notes instance. 
/// 
/// Unlike subjects, there are no prerequisites for a note. 
/// Though certain processes (i.e., compilation) will require the note to be exported in the filesystem. 
/// 
/// Because of the nature of the program (and filesystems, in general), all note instances does not have the parent object. 
/// Thus, its methods constantly require the parent object as one of the parameters. 
#[derive(Clone, Debug)]
pub struct Note {
    title: String, 
}

impl Note {
    /// Creates a new note instance. 
    pub fn new(title: String) -> Self {
        Note {
            title, 
        }
    }

    /// Searches for the note in the shelf. 
    /// 
    /// This only checks whether the associated path of the note exists. 
    /// To check if the note exists on the notes database, call the `Note::is_entry_exists` method. 
    pub fn from (title: &str, subject: &Subject, notes: &Shelf) -> Result<Option<Self>, Error> {
        let note = Note::new(title.to_string());

        match note.is_path_exists(&subject, &notes) {
            true => Ok(Some(note)), 
            false => Ok(None), 
        }
    }

    /// Similar to the `from` method, only on a bigger scale. 
    pub fn from_vec<S: AsRef<str>> (note_titles: &Vec<S>, subject: &Subject, notes: &Shelf) -> Result<Vec<Option<Self>>, Error> {
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
    pub fn from_vec_loose<S: AsRef<str>> (note_titles: &Vec<S>, subject: &Subject, notes: &Shelf) -> Result<Vec<Self>, Error> {
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
    pub fn datetime_modified (&self, subject: &Subject, shelf: &Shelf) -> Result<chrono::DateTime<chrono::Local>, Error> {
        match self.is_path_exists(&subject, &shelf) {
            true => {
                let metadata = fs::metadata(self.path_in_shelf(&subject, &shelf)).map_err(Error::IoError)?;
                let modification_time = metadata.modified().map_err(Error::IoError)?;

                Ok(chrono::DateTime::<chrono::Local>::from(modification_time))
            }, 
            false => Err(Error::IoError(io::Error::from(io::ErrorKind::NotFound)))
        }
    } 

    /// Returns the path of the note instance along with its associated subject. 
    /// 
    /// It does not necessarily mean that the note exists. 
    /// Be sure to check it first. 
    pub fn path_in_shelf(&self, subject: &Subject, notes: &Shelf) -> PathBuf {
        let mut path = subject.path_in_shelf(&notes);
        path.push(self.file_name());

        path
    }

    /// Returns the path of the note relative to the subject. 
    pub fn path(&self, subject: &Subject) -> PathBuf {
        let mut path = subject.path();
        path.push(self.file_name());

        path
    }

    /// Returns the file name of the note. 
    pub fn file_name (&self) -> String {
        let mut slug = helpers::string::kebab_case(&self.title);
        slug.push_str(".tex");
        
        slug
    }

    /// Writes the resulting LaTeX file in the filesystem. 
    /// 
    /// For templating, it uses [a Rust implementation of Handlebars](https://github.com/sunng87/handlebars-rust). 
    /// The configuration of Handlebars does not escape anything (uses [`handlebars::no_escape`](https://docs.rs/handlebars/3.0.0-beta.1/handlebars/fn.no_escape.html)). 
    pub fn export (
        &self, 
        subject: &Subject, 
        notes: &Shelf, 
        template: &str, 
    ) -> Result<(), Error> {
        if !notes.is_exported() {
            return Err(Error::UnexportedShelfError(notes.path()));
        }
        
        let path = self.path_in_shelf(&subject, &notes);
        let mut note_file = OpenOptions::new().create_new(true).write(true).open(path).map_err(Error::IoError)?;
        note_file.write(template.as_bytes()).map_err(Error::IoError)?;

        Ok(())
    }

    /// Simply deletes the file in the shelf filesystem. 
    /// 
    /// This does not delete the entry in the notes database. 
    pub fn delete (&self, subject: &Subject, notes: &Shelf) -> Result<(), Error> {
        let path = self.path_in_shelf(&subject, &notes);

        fs::remove_file(path).map_err(Error::IoError)
    }

    /// Checks for the file if it exists in the shelf. 
    pub fn is_path_exists (&self, subject: &Subject, notes: &Shelf) -> bool {
        let path = self.path_in_shelf(&subject, &notes);

        path.exists()
    }

    /// Checks if the note instance is present in the shelf database. 
    pub fn is_entry_exists (&self, subject: &Subject, notes: &Shelf) -> Result<bool, Error> {
        let id = notes.get_note_id(subject, &self)?;
        
        match id {
            Some(_id) => Ok(true), 
            None => Ok(false), 
        }
    }

    /// Checks if the associated file in the filesystem and the note entry in the database both exists. 
    pub fn is_sync (&self, subject: &Subject, notes: &Shelf) -> bool {
        self.is_path_exists(&subject, &notes) && match self.is_entry_exists(&subject, &notes) {
            Ok(v) => v, 
            Err(_e) => false, 
        }
    }
}
