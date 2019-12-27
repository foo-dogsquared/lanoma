use std::fs::{self, DirBuilder};
use std::path::{Path, PathBuf};

use globwalk;

use crate::error::Error;
use crate::helpers;
use crate::items::{Note, Subject};
use crate::Result;

/// A struct holding the common export options.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    strict: bool,
}

impl ExportOptions {
    /// Creates a new instance of the export options.
    /// By default, all of the options are set to false.
    pub fn new() -> Self {
        Self {
            /// This is used for exporting items to the filesystem.
            /// If the item already exists, it will cause an error.
            strict: false,
        }
    }

    /// Sets the strictness of the export.
    /// This is used when including the items (e.g., subjects, notes) in the database during the creation process.
    pub fn strict(
        &mut self,
        strict: bool,
    ) -> &mut Self {
        self.strict = strict;
        self
    }
}

/// The shelf is where it contains the subjects and its notes.
/// In other words, it is the base directory of the operations taken place in Texture Notes.
#[derive(Debug, Clone)]
pub struct Shelf {
    path: PathBuf,
}

/// A builder for the shelf instance.
/// Setting the data does not take the ownership (or consume) the builder to enable dynamic setting.
#[derive(Debug, Clone)]
pub struct ShelfBuilder {
    use_db: bool,
    path: Option<PathBuf>,
}

impl ShelfBuilder {
    /// Create a new shelf builder instance.
    pub fn new() -> Self {
        Self {
            use_db: false,
            path: None,
        }
    }

    /// Sets the path of the shelf.
    pub fn path<P>(
        &mut self,
        path: P,
    ) -> &mut Self
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref().to_path_buf();
        self.path = Some(path);

        self
    }

    /// Create the shelf instance from the builder.
    /// Also consumes the builder.
    pub fn build(self) -> Result<Shelf> {
        let mut shelf = Shelf::new();

        if self.path.is_some() {
            let path = self.path.unwrap();

            shelf.set_path(path)?;
        }

        Ok(shelf)
    }
}

impl Shelf {
    /// Create a new shelf instance.
    pub fn new() -> Self {
        Self {
            path: PathBuf::new(),
        }
    }

    /// Creates a shelf instance from the filesystem.
    pub fn from<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let notes_object = Shelf {
            path: path.to_path_buf(),
        };

        if !notes_object.is_valid() {
            return Err(Error::ValueError);
        }

        Ok(notes_object)
    }

    /// Returns the current path of the shelf.
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// Sets the path of the shelf.
    /// Returns the old path.
    ///
    /// If the shelf is exported, it will also move the folder in the filesystem.
    pub fn set_path<P: AsRef<Path>>(
        &mut self,
        to: P,
    ) -> Result<PathBuf> {
        let old_path = self.path();
        let new_path = to.as_ref().to_path_buf();

        if self.is_valid() {
            fs::rename(&old_path, &new_path).map_err(Error::IoError)?;
        }

        self.path = new_path;

        Ok(old_path)
    }

    /// Checks if the shelf is valid.
    pub fn is_valid(&self) -> bool {
        self.path.is_dir()
    }

    /// Exports the shelf in the filesystem.
    /// If the shelf has a database, it will also export subjects at the filesystem.
    /// However, notes are not exported due to needing a dynamic output.
    pub fn export(&mut self) -> Result<()> {
        let dir_builder = DirBuilder::new();

        if !self.is_valid() {
            helpers::fs::create_folder(&dir_builder, self.path())?;
        }

        Ok(())
    }

    /// Gets the subjects in the shelf filesystem.
    pub fn get_subjects<'s>(
        &self,
        subjects: &'s Vec<Subject>,
    ) -> Vec<&'s Subject> {
        subjects
            .iter()
            .filter(|&subject| subject.is_path_exists(&self))
            .collect()
    }

    /// Creates its folder structure on the filesystem.
    /// It can also add the subject instance in the database, if specified.
    ///
    /// Returns the subject instance that succeeded in its creation process.
    pub fn create_subjects<'s>(
        &self,
        subjects: &'s Vec<Subject>,
        export_options: &ExportOptions,
    ) -> Vec<&'s Subject> {
        subjects
            .iter()
            .filter(|&subject| subject.export(&self, export_options.strict).is_ok())
            .collect()
    }

    /// Deletes the subject instance in the shelf.
    pub fn delete_subjects<'s>(
        &self,
        subjects: &'s Vec<Subject>,
    ) -> Vec<&'s Subject> {
        subjects
            .iter()
            .filter(|&subject| subject.delete(&self).is_ok())
            .collect()
    }

    /// Get the valid notes in the shelf.
    pub fn get_notes<'n>(
        &self,
        subject: &Subject,
        notes: &'n Vec<Note>,
    ) -> Vec<&'n Note> {
        notes
            .iter()
            .filter(|&note| note.is_path_exists(&subject, &self))
            .collect()
    }

    /// Get the notes in the shelf filesystem.
    pub fn get_notes_in_fs(
        &self,
        subject: &Subject,
    ) -> Result<Vec<Note>> {
        let mut notes: Vec<Note> = vec![];

        let subject_path = subject.path_in_shelf(&self);

        let tex_files = globwalk::GlobWalkerBuilder::new(subject_path, "*.tex")
            .build()
            .map_err(Error::GlobParsingError)?;

        for file in tex_files {
            if let Ok(file) = file {
                let note_path = file.path();

                let file_stem = note_path.file_stem().unwrap().to_string_lossy();

                let note_instance = Note::from(file_stem, &subject, &self)?.unwrap();

                notes.push(note_instance);
            }
        }

        Ok(notes)
    }

    /// Create the note in the shelf in the filesystem.
    ///
    /// If specified, it can also add the note in the shelf database.
    ///
    /// By default, the method will not return an error if it's already exported.
    /// However, you can set the method to be strict on it.
    pub fn create_note(
        &self,
        subject: &Subject,
        note: &Note,
        value: &str,
        export_options: &ExportOptions,
    ) -> Result<()> {
        note.export(&subject, &self, &value, export_options.strict)
    }

    /// Creates the files of the note instances in the shelf.
    pub fn create_notes<'n>(
        &self,
        subject: &Subject,
        notes: &'n Vec<Note>,
        value: &str,
        export_options: &ExportOptions,
    ) -> Vec<&'n Note> {
        notes
            .iter()
            .filter(|&note| {
                note.export(&subject, &self, &value, export_options.strict)
                    .is_ok()
            })
            .collect()
    }

    /// Deletes the entry and filesystem of the note instances in the shelf.
    pub fn delete_notes<'n>(
        &self,
        subject: &Subject,
        notes: &'n Vec<Note>,
    ) -> Vec<&'n Note> {
        notes
            .iter()
            .filter(|&note| note.delete(&subject, &self).is_ok())
            .collect()
    }

    // TODO: Update operation for the subjects and the notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts;
    use tempfile;

    fn tmp_shelf() -> Result<(tempfile::TempDir, Shelf)> {
        let tmp_dir = tempfile::TempDir::new().map_err(Error::IoError)?;
        let mut shelf_builder = ShelfBuilder::new();
        shelf_builder.path(tmp_dir.path());

        Ok((tmp_dir, shelf_builder.build()?))
    }

    #[test]
    fn basic_note_usage() -> Result<()> {
        let (shelf_tmp_dir, mut shelf) = tmp_shelf()?;
        let export_options = ExportOptions::new();

        assert!(shelf.export().is_ok());

        let test_subject_input =
            Subject::from_vec_loose(&vec!["Calculus", "Algebra", "Algebra/Precalculus"], &shelf);
        let test_note_input = Note::from_vec_loose(
            &vec![
                "Precalculus Quick Review",
                "Introduction to Integrations",
                "Introduction to Limits",
            ],
            &test_subject_input[0],
            &shelf,
        )?;

        let created_subjects = shelf.create_subjects(&test_subject_input, &export_options);
        assert_eq!(created_subjects.len(), 3);

        let created_notes = shelf.create_notes(
            &test_subject_input[0],
            &test_note_input,
            consts::NOTE_TEMPLATE,
            &export_options,
        );
        assert_eq!(created_notes.len(), 3);

        let available_subjects = shelf.get_subjects(&test_subject_input);
        assert_eq!(available_subjects.len(), 3);

        let available_notes = shelf.get_notes(&test_subject_input[0], &test_note_input);
        assert_eq!(available_notes.len(), 3);

        let all_available_notes_from_fs = shelf.get_notes_in_fs(&test_subject_input[0])?;
        assert_eq!(all_available_notes_from_fs.len(), 3);

        let deleted_notes = shelf.delete_notes(&test_subject_input[0], &test_note_input);
        assert_eq!(deleted_notes.len(), 3);

        // It became 2 because the algebra subject is deleted along with the precalculus subject.
        let deleted_subjects = shelf.delete_subjects(&test_subject_input);
        assert_eq!(deleted_subjects.len(), 2);

        Ok(())
    }

    #[test]
    fn subject_instances_test() -> Result<()> {
        let (shelf_tmp_dir, mut shelf) = tmp_shelf()?;

        let export_options: ExportOptions = ExportOptions::new();

        assert!(shelf.export().is_ok());

        let test_subject: Subject = Subject::from("Mathematics".to_string());
        assert_eq!(test_subject.is_valid(&shelf), false);

        test_subject.export(&shelf, export_options.strict)?;
        assert_eq!(test_subject.is_valid(&shelf), true);
        assert_eq!(test_subject.is_path_exists(&shelf), true);

        shelf.create_subjects(&vec![test_subject.clone()], &export_options);
        assert_eq!(test_subject.is_valid(&shelf), true);
        assert_eq!(test_subject.is_path_exists(&shelf), true);

        Ok(())
    }

    #[test]
    #[should_panic]
    fn invalid_note_export() {
        let note_path = PathBuf::from("./test/invalid/location/is/invalid");
        let mut test_case_builder = ShelfBuilder::new();
        test_case_builder.path(note_path);

        let mut test_case = test_case_builder.build().unwrap();

        assert!(test_case.export().is_ok());
    }

    #[test]
    #[should_panic]
    fn invalid_note_import() {
        let note_path = PathBuf::from("./this/is/invalid/note/location/it/does/not/exists/lol");

        assert!(Shelf::from(note_path).is_ok())
    }
}
