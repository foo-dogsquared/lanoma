use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::PathBuf;

use chrono::{self};
use heck::KebabCase;
use serde::{Deserialize, Serialize};
use toml;

use crate::error::Error;
use crate::shelf::{Shelf, ShelfData, ShelfItem};
use crate::subjects::Subject;
use crate::{Object, Result};

use crate::modify_toml_table;

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

impl AsRef<str> for Note {
    fn as_ref(&self) -> &str {
        self.title.as_ref()
    }
}

impl ShelfItem<(&Subject, &Shelf)> for Note {
    /// Returns the path of the note instance along with its associated subject.
    ///
    /// It does not necessarily mean that the note exists.
    /// Be sure to check it first.
    fn path_in_shelf(
        &self,
        params: (&Subject, &Shelf),
    ) -> PathBuf {
        let (subject, shelf) = params;
        let mut path = subject.path_in_shelf(&shelf);
        path.push(self.file_name());

        path
    }

    fn is_path_exists(
        &self,
        params: (&Subject, &Shelf),
    ) -> bool {
        self.path_in_shelf(params).is_file()
    }

    /// Simply deletes the file in the shelf filesystem.
    fn delete(
        &self,
        params: (&Subject, &Shelf),
    ) -> Result<()> {
        let path = self.path_in_shelf(params);

        fs::remove_file(path).map_err(Error::IoError)
    }

    /// Simply create a new note in the filesystem.
    fn export(
        &self,
        params: (&Subject, &Shelf),
    ) -> Result<()> {
        let (_, shelf) = params;
        if !shelf.is_valid() {
            return Err(Error::UnexportedShelfError(shelf.path()));
        }

        let path = self.path_in_shelf(params);
        let mut note_file_open_options = OpenOptions::new();
        note_file_open_options.write(true).create_new(true);

        note_file_open_options.open(path).map_err(Error::IoError)?;
        Ok(())
    }
}

impl Object for Note {
    fn data(&self) -> toml::Value {
        let mut note_as_toml = toml::Value::from(HashMap::<String, toml::Value>::new());
        modify_toml_table! {note_as_toml,
            ("title", self.title()),
            ("file", self.file_name())
        };

        note_as_toml
    }
}

impl ShelfData<(&Subject, &Shelf)> for Note {
    fn data(
        &self,
        params: (&Subject, &Shelf),
    ) -> toml::Value {
        let mut note_as_toml = Object::data(self);

        modify_toml_table! {note_as_toml,
            ("path_in_shelf", self.path_in_shelf(params))
        };

        note_as_toml
    }
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

    /// Searches for the note in the shelf filesystem.
    pub fn from<S: AsRef<str>>(
        title: S,
        subject: &Subject,
        shelf: &Shelf,
    ) -> Option<Self> {
        let title = title.as_ref();
        let note = Note::new(title.to_string());

        match note.is_path_exists((&subject, &shelf)) {
            true => Some(note),
            false => None,
        }
    }

    /// Similar to the `from` method, only on a bigger scale.
    pub fn from_vec<S: AsRef<str>>(
        notes: &Vec<S>,
        subject: &Subject,
        shelf: &Shelf,
    ) -> Vec<Option<Self>> {
        notes
            .iter()
            .map(|note| Self::from(note, &subject, &shelf))
            .collect()
    }

    /// Searches for the specified notes in the shelf.
    /// If there is no associated note found in the shelf, it will instead create one.
    /// Making the return data creates a guaranteed vector of note instances.
    pub fn from_vec_loose<S: AsRef<str>>(
        notes: &Vec<S>,
        subject: &Subject,
        shelf: &Shelf,
    ) -> Vec<Self> {
        notes
            .iter()
            .map(|note| Self::from(note, &subject, &shelf).unwrap_or(Self::new(note)))
            .collect()
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
        match self.is_path_exists((&subject, &shelf)) {
            true => {
                let metadata =
                    fs::metadata(self.path_in_shelf((&subject, &shelf))).map_err(Error::IoError)?;
                let modification_time = metadata.modified().map_err(Error::IoError)?;

                Ok(chrono::DateTime::<chrono::Utc>::from(modification_time))
            }
            false => Err(Error::IoError(io::Error::from(io::ErrorKind::NotFound))),
        }
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
        let mut slug = self.title.to_kebab_case();
        slug.push_str(".tex");

        slug
    }
}
