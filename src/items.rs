use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{self, PathBuf};

use chrono::{self};
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::helpers;
use crate::shelf::Shelf;
use crate::subjects::Subject;
use crate::Result;

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
    ) -> Result<Option<Self>> {
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
    ) -> Result<Vec<Option<Self>>> {
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
    ) -> Result<Vec<Self>> {
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
    ) -> Result<chrono::DateTime<chrono::Utc>> {
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
    ) -> Result<()> {
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
    ) -> Result<()> {
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
}
