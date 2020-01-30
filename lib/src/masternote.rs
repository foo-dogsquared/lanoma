use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;

use toml;

use crate::error::Error;
use crate::note::Note;
use crate::shelf::{Shelf, ShelfData, ShelfItem};
use crate::subjects::Subject;
use crate::Object;
use crate::Result;

use crate::modify_toml_table;

const MASTER_NOTE_FILE: &str = "_master.tex";

/// The master note is a note that uses the filtered notes.
pub struct MasterNote {
    subject: Subject,
    notes: Vec<Note>,
}

impl Object for MasterNote {
    fn data(&self) -> toml::Value {
        let mut master_note_as_toml = toml::Value::from(HashMap::<String, toml::Value>::new());
        let notes_toml: Vec<toml::Value> =
            self.notes.iter().map(|note| Object::data(note)).collect();

        modify_toml_table! {master_note_as_toml,
            ("notes", notes_toml),
            ("subject", Object::data(&self.subject)),
            ("_file", self.file_name())
        };

        master_note_as_toml
    }
}

impl ShelfData<&Shelf> for MasterNote {
    fn data(
        &self,
        shelf: &Shelf,
    ) -> toml::Value {
        let mut master_note_as_toml = Object::data(self);
        let notes_toml: Vec<toml::Value> = self
            .notes
            .iter()
            .map(|note| ShelfData::data(note, (self.subject(), &shelf)))
            .collect();

        modify_toml_table! {master_note_as_toml,
            ("notes", notes_toml),
            ("path", self.path().to_string_lossy())
        };

        master_note_as_toml
    }
}

impl ShelfItem<&Shelf> for MasterNote {
    /// Return the path of the master note in the shelf.
    fn path_in_shelf(
        &self,
        shelf: &Shelf,
    ) -> PathBuf {
        let mut path = self.subject.path_in_shelf(&shelf);
        path.push(MASTER_NOTE_FILE);

        path
    }

    fn is_path_exists(
        &self,
        shelf: &Shelf,
    ) -> bool {
        self.path_in_shelf(&shelf).is_file()
    }

    fn export(
        &self,
        shelf: &Shelf,
    ) -> Result<()> {
        let master_note_path = self.path_in_shelf(&shelf);
        OpenOptions::new()
            .create_new(true)
            .open(&master_note_path)
            .map_err(Error::IoError)?;

        Ok(())
    }

    fn delete(
        &self,
        shelf: &Shelf,
    ) -> Result<()> {
        let master_note_path = self.path_in_shelf(&shelf);
        fs::remove_file(master_note_path).map_err(Error::IoError)
    }
}

impl MasterNote {
    /// Create a new master note.
    pub fn new(subject: Subject) -> Self {
        Self {
            subject,
            notes: Vec::new(),
        }
    }

    /// Return a reference to the subject.
    pub fn subject(&self) -> &Subject {
        &self.subject
    }

    /// Return a reference to the notes.
    pub fn notes(&self) -> &Vec<Note> {
        &self.notes
    }

    /// Add a note to be included.
    pub fn push(
        &mut self,
        note: &Note,
    ) -> &mut Self {
        self.notes.push(note.clone());
        self
    }

    /// Return the path of the master note.
    pub fn path(&self) -> PathBuf {
        let mut path = self.subject.path();
        path.push(MASTER_NOTE_FILE);

        path
    }

    /// Return the file name of the master note.
    pub fn file_name(&self) -> String {
        MASTER_NOTE_FILE.to_string()
    }
}
