use std::fs::{ self, DirBuilder };
use std::path::{ self, PathBuf, Path };

use rusqlite;
use rusqlite::{ Connection };
use r2d2;
use r2d2_sqlite;
use chrono::{ self };
use serde_json;
use globwalk;

use crate::consts;
use crate::helpers;
use crate::error::Error;
use crate::notes::{ Note, Subject };
use crate::Result;

// even though string literals are always static, 
// it is better to anotate them for explicit intentions
const DB_NAME: &str = "notes.db";

/// A struct holding the common export options. 
#[derive(Debug, Clone)]
pub struct ExportOptions {
    with_metadata: bool, 
    strict: bool, 
    include_in_db: bool, 
    with_db: bool, 
}

impl ExportOptions {
    /// Creates a new instance of the export options. 
    /// By default, all of the options are set to false. 
    pub fn new() -> Self {
        Self {
            /// This is used for exporting subjects. 
            with_metadata: false, 
            
            /// This is used for exporting items to the filesystem. 
            /// If the item already exists, it will cause an error. 
            strict: false, 

            /// This is used when creating/including the items in the database. 
            include_in_db: false, 

            /// If set to true, the shelf will have a database. 
            with_db: false, 
        }
    }

    /// Sets the export to include metadata. 
    /// This is used when exporting subjects. 
    pub fn with_metadata(&mut self, with_metadata: bool) -> &mut Self {
        self.with_metadata = with_metadata;
        self
    }

    /// Sets the strictness of the export. 
    /// This is used when including the items (e.g., subjects, notes) in the database during the creation process. 
    pub fn strict(&mut self, strict: bool) -> &mut Self {
        self.strict = strict;
        self
    }

    /// Sets when including the items in the database. 
    pub fn include_in_db(&mut self, include_in_db: bool) -> &mut Self {
        self.include_in_db = include_in_db;
        self
    }

    /// Sets `ExportOptions.with_db` with the given boolean value. 
    pub fn with_db(&mut self, with_db: bool) -> &mut Self {
        self.with_db = with_db;
        self
    }
}

/// The shelf is where it contains the subjects and its notes. 
/// 
/// It is where the major operations on the database occur. 
#[derive(Debug, Clone)]
pub struct Shelf {
    path: PathBuf, 
    db: Option<r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>>, 
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

    /// Sets the value for the `set_db` field.
    pub fn use_db(&mut self, use_db: bool) -> &mut Self {
        self.use_db = use_db;
        self
    }

    /// Sets the path of the shelf. 
    pub fn path<P>(&mut self, path: P) -> &mut Self 
        where 
            P: AsRef<Path>
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

        if self.use_db {
            shelf.set_db_pool_in_memory()?;
        }

        Ok(shelf)
    }
}

impl Shelf {
    /// Create a new shelf instance. 
    pub fn new() -> Self {
        Self {
            path: PathBuf::new(), 
            db: None
        }
    }

    /// Creates a shelf instance from the filesystem. 
    pub fn from (path: PathBuf) -> Result<Self> {
        let mut notes_object = Shelf { path, db: None };

        if !notes_object.is_valid() {
            return Err(Error::ValueError);
        }

        if notes_object.db_path().exists() {
            notes_object.set_db_pool()?;
        }

        Ok(notes_object)
    }

    /// Returns the current path of the shelf. 
    pub fn path (&self) -> PathBuf {
        self.path.clone()
    }

    /// Sets the path of the shelf. 
    /// Returns the old path. 
    /// 
    /// If the shelf is exported, it will also move the folder in the filesystem. 
    pub fn set_path<P: AsRef<Path>> (&mut self, to: P) -> Result<PathBuf> {
        let old_path = self.path();
        let new_path = to.as_ref().to_path_buf();

        if self.is_exported() {
            fs::rename(&old_path, &new_path).map_err(Error::IoError)?;
        }

        self.path = new_path;

        Ok(old_path)
    }

    /// Returns the associated path of the database. 
    pub fn db_path (&self) -> PathBuf {
        let mut db_path: PathBuf = self.path.clone();
        db_path.push(DB_NAME);

        db_path
    }

    /// Check if the shelf has a database file. 
    pub fn has_db_file (&self) -> bool {
        self.db_path().exists()
    }

    /// Set up the associated database of the shelf from the filesystem. 
    /// It also means the database support for the shelf is enabled. 
    fn set_db_pool (&mut self) -> Result<()> {
        let db_path = self.db_path();

        if !self.has_db_file() {
            return Err(Error::NoShelfDatabase(db_path));
        }

        let db_manager = r2d2_sqlite::SqliteConnectionManager::file(db_path).with_init(| conn | conn.execute_batch(consts::SQLITE_SCHEMA) );
        let db_pool = r2d2::Pool::builder().max_size(20).build(db_manager).map_err(Error::R2D2Error)?;
                
        self.db = Some(db_pool);
        
        Ok(())
    }

    /// Sets the database pool in memory. 
    fn set_db_pool_in_memory(&mut self) -> Result<()> {
        let db_manager = r2d2_sqlite::SqliteConnectionManager::memory().with_init(| conn | conn.execute_batch(consts::SQLITE_SCHEMA));
        let db_pool = r2d2::Pool::builder().max_size(20).build(db_manager).map_err(Error::R2D2Error)?;

        self.db = Some(db_pool);

        Ok(())   
    }

    /// Creates a backup of the database. 
    /// Also useful for saving an in-memory database to a file. 
    fn db_backup(&self) -> Result<()> {
        let db_conn = self.open_db()?;

        db_conn.backup(rusqlite::DatabaseName::Main, self.db_path(), None).map_err(Error::DatabaseError)?;
        
        Ok(())
    }

    /// Removes the database from the shelf instance. 
    /// It will also delete the database file in the filesystem. 
    fn remove_db(&mut self) -> Result<()> {
        if self.has_db_file() {
            fs::remove_file(self.db_path()).map_err(Error::IoError)?;
        }
        
        self.db = None;

        Ok(())
    }

    /// Create a connection from the shelf database (if it has any). 
    /// This is mainly for using the database in multiple threads since Rusqlite (and the nature of Rust) does not allow it. 
    /// See [rusqlite issue #188](https://github.com/jgallagher/rusqlite/issues/188) for more information on the topic of SQLite thread safety. 
    fn open_db (&self) -> Result<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>> {
        let db_conn = match self.db.as_ref() {
            Some(db_pool) => db_pool.get().map_err(Error::R2D2Error)?, 
            None => return Err(Error::NoShelfDatabase(self.path())), 
        };

        Ok(db_conn)
    }

    /// Checks if the shelf database is enabled. 
    pub fn use_db (&self) -> bool {
        self.db.is_some()
    }
    
    /// Checks if the shelf is exported in the filesystem.
    pub fn is_exported (&self) -> bool {
        self.path.exists()
    }

    /// Checks if the shelf is valid. 
    pub fn is_valid(&self) -> bool {
        self.is_exported()
    }

    /// Exports the shelf in the filesystem. 
    /// If the shelf has a database, it will also export subjects at the filesystem. 
    /// However, notes are not exported due to needing a dynamic output. 
    pub fn export(&mut self, export_options: &ExportOptions) -> Result<()> {
        let dir_builder = DirBuilder::new();
        
        if self.is_valid() {
            return Err(Error::ShelfAlreadyExists(self.path()));
        }
        
        helpers::filesystem::create_folder(&dir_builder, self.path())?;
        if !self.has_db_file() && self.use_db() {
            self.db_backup()?;
            self.set_db_pool()?;
        }

        if self.use_db() {
            let subjects = self.get_all_subjects_from_db(None)?;
            for subject in subjects {
                if !subject.is_path_exists(&self) {
                    subject.export(&self, export_options.with_metadata)?;
                }
            }
        }

        Ok(())
    }

    /// Gets the associated subject ID in the database. 
    /// 
    /// This does not check if the subject is in the filesystem. 
    pub fn get_subject_id (&self, subject: &Subject) -> Result<Option<i64>> {
        let subject_slug = helpers::string::kebab_case(&subject.name());
        let db_conn = self.open_db()?;
        let mut select_stmt = db_conn.prepare("SELECT id FROM subjects WHERE slug == ?").map_err(Error::DatabaseError)?;
        let mut row_result = select_stmt.query(&[&subject_slug]).map_err(Error::DatabaseError)?;

        match row_result.next().map_err(Error::DatabaseError)? {
            Some(row) => {
                let id: i64 = row.get(0).map_err(Error::DatabaseError)?;
                Ok(Some(id))
            }, 
            None => Ok(None), 
        }
    }

    /// Gets the associated subject instance with its ID in the database. 
    /// 
    /// Take caution as this does not check if the subject is in the filesystem. 
    pub fn get_subject_by_id (&self, id: i64) -> Result<Option<Subject>> {
        let db_conn = self.open_db()?;
        let mut select_stmt = db_conn.prepare("SELECT name FROM subjects WHERE id == ?").map_err(Error::DatabaseError)?;
        let mut row_result = select_stmt.query(&[&id]).map_err(Error::DatabaseError)?;

        match row_result.next().map_err(Error::DatabaseError)? {
            Some(row) => {
                let name: String = row.get(0).map_err(Error::DatabaseError)?;

                match Subject::from(&name, &self)? {
                    Some(v) => Ok(Some(v)), 
                    None => Ok(None)
                }
            },
            None => Ok(None)
        }
    }

    /// Gets the associated ID of the note instance in the database. 
    pub fn get_note_id(&self, subject: &Subject, note: &Note) -> Result<Option<i64>> {
        let subject_id = match self.get_subject_id(&subject)? {
            Some(id) => id, 
            None => return Err(Error::ValueError), 
        };

        let note_title_slug = helpers::string::kebab_case(&note.title());

        let db_conn = self.open_db()?;
        let mut select_stmt = db_conn.prepare("SELECT id FROM notes WHERE subject_id == ? AND slug == ?").map_err(Error::DatabaseError)?;
        let mut row_result = select_stmt.query(&[&subject_id.to_string(), &note_title_slug]).map_err(Error::DatabaseError)?;

        match row_result.next().map_err(Error::DatabaseError)? {
            Some(row) => {
                let id: i64 = row.get(0).map_err(Error::DatabaseError)?;
                
                Ok(Some(id))
            }, 
            None => Ok(None)
        }
    }

    /// Gets the associated note instance with its ID in the database. 
    pub fn get_note_by_id (&self, id: i64) -> Result<Option<Note>> {
        let db_conn = self.open_db()?;
        let mut select_stmt = db_conn.prepare("SELECT name FROM subjects WHERE id == ?").map_err(Error::DatabaseError)?;
        let mut result_row = select_stmt.query(&[&id]).map_err(Error::DatabaseError)?;

        match result_row.next().map_err(Error::DatabaseError)? {
            Some(row) => {
                let name = row.get(0).map_err(Error::DatabaseError)?;

                Ok(Some(Note::new(name)))
            }, 
            None => Ok(None)
        }
    }

    /// Gets the associated subject instance with the one of the note ID in the database. 
    /// 
    /// This does not check if the subject is in the shelf filesystem. 
    pub fn get_subject_by_note_id (&self, id: i64) -> Result<Option<Subject>> {
        let db_conn = self.open_db()?;
        let mut select_stmt = db_conn.prepare("SELECT subject_id FROM notes WHERE id == ?").map_err(Error::DatabaseError)?;
        let mut result_row = select_stmt.query(&[&id]).map_err(Error::DatabaseError)?;

        match result_row.next().map_err(Error::DatabaseError)? {
            Some(row) => {
                let subject_id: i64 = row.get(0).map_err(Error::DatabaseError)?;
                let subject = self.get_subject_by_id(subject_id)?;
                
                match subject {
                    Some(v) => Ok(Some(v)), 
                    None => Ok(None), 
                }
            }, 
            None => Ok(None), 
        }
    }

    /// Gets the subjects in the database. 
    /// 
    /// It can also check if the subject instance has an entry in the database, if specified. 
    pub fn get_subjects(&self, subjects: &Vec<Subject>, sync: bool) -> Result<Vec<Subject>> {
        let mut valid_subjects: Vec<Subject> = vec![];

        for subject in subjects.iter() {
            let ok_status = match sync {
                true => subject.is_sync(&self), 
                false => subject.is_valid(&self), 
            };

            if ok_status {
                valid_subjects.push(subject.clone());
            }
        }

        Ok(valid_subjects)
    }

    /// Returns a vector of valid subjects (by its ID) found in the database. 
    pub fn get_subjects_by_id (&self, subject_ids: &Vec<i64>, sort: Option<&str>) -> Result<Vec<Subject>> {
        let mut valid_subjects: Vec<Subject> = vec![];
        
        for &id in subject_ids.iter() {
            match self.get_subject_by_id(id)? {
                Some(v) => valid_subjects.push(v), 
                None => continue, 
            };
        }

        Ok(valid_subjects)
    }

    /// Returns all of the associated subject instances in the database. 
    /// 
    /// This does not check if the subject instance exists in the filesystem. 
    pub fn get_all_subjects_from_db (&self, sort: Option<&str>) -> Result<Vec<Subject>> {
        let mut select_stmt = String::from("SELECT name FROM subjects");
        match sort {
            Some(v) => {
                select_stmt.push_str(" ORDER BY ");
                select_stmt.push_str(v);
            }
            None => (), 
        };

        let db_conn = self.open_db()?;
        let mut select_stmt = db_conn.prepare(&select_stmt).map_err(Error::DatabaseError)?;
        let mut subjects_from_db_iter = select_stmt.query(rusqlite::NO_PARAMS).map_err(Error::DatabaseError)?;

        let mut valid_subjects: Vec<Subject> = vec![];

        while let Some(subject_row) = subjects_from_db_iter.next().map_err(Error::DatabaseError)? {
            let name: String = subject_row.get(0).map_err(Error::DatabaseError)?;
            valid_subjects.push(Subject::new(name)); 
        }

        Ok(valid_subjects)
    }

    /// Creates its folder structure on the filesystem. 
    /// It can also add the subject instance in the database, if specified. 
    /// 
    /// Returns the subject instance that succeeded in its creation process. 
    pub fn create_subjects(
        &self, 
        subjects: &Vec<Subject>, 
        export_options: &ExportOptions
    ) -> Result<Vec<Subject>> {
        let mut valid_subjects: Vec<Subject> = vec![];

        for subject in subjects.iter() {
            // a subject that already exists in the shelf is considered to be "created"
            // this is for considering when the already existing subject is considered to be added in the shelf database
            // though, an available strict mode is made for that case
            let ok_status = subject.export(&self, export_options.with_metadata).is_ok() || (subject.is_valid(&self) && !export_options.strict);
            
            if self.use_db() && ok_status && export_options.include_in_db {
                match self.create_subject_entry(&subject).err() {
                    Some(e) => match e {
                        Error::NoShelfDatabase (path) => return Err(Error::NoShelfDatabase(path)), 
                        _ => continue, 
                    },
                    None => (), 
                }
            }

            // creating the subject in the filesystem 
            if ok_status {
                valid_subjects.push(subject.clone());
            }
        }

        Ok(valid_subjects)
    }

    fn create_subject_entry(&self, subject: &Subject) -> Result<i64> {
        let db_conn = self.open_db()?;
        let mut insert_subject_stmt = db_conn.prepare("INSERT INTO subjects (name, slug) VALUES (?, ?)").map_err(Error::DatabaseError)?;

        insert_subject_stmt.insert(&[&subject.name(), &helpers::string::kebab_case(&subject.name())]).map_err(Error::DatabaseError)
    }

    /// Delete the subject in the shelf. 
    pub fn delete_subject (&self, subject: &Subject) -> Result<()> {
        match self.delete_subject_entry(&subject) {
            Ok(_v) => (), 
            Err(e) => match e {
                Error::NoShelfDatabase (_path) => (), 
                _ => return Err(e), 
            }, 
        }

        subject.delete(&self)
    }

    /// Deletes the entry and the filesystem of the subject instance in the database. 
    pub fn delete_subjects (&self, subjects: &Vec<Subject>) -> Result<Vec<Subject>> {
        let mut valid_subjects: Vec<Subject> = vec![];

        for subject in subjects.iter() {            
            if self.delete_subject(&subject).is_ok() {
                valid_subjects.push(subject.clone());
            }
        }

        Ok(valid_subjects)
    }

    pub fn delete_subject_entry (&self, subject: &Subject) -> Result<usize> {
        let db_conn = self.open_db()?;
        let mut delete_subject_stmt = db_conn.prepare("DELETE FROM subjects WHERE name == ?").map_err(Error::DatabaseError)?;

        delete_subject_stmt.execute(&[&subject.name()]).map_err(Error::DatabaseError)
    }

    /// Get the valid notes in the shelf. 
    /// It can also check if the notes are in the shelf database. 
    pub fn get_notes (&self, subject: &Subject, notes: &Vec<Note>, sync: bool) -> Result<Vec<Note>> {
        let mut valid_notes: Vec<Note> = vec![];

        for note in notes.iter() {
            let ok_status = match sync {
                true => note.is_sync(&subject, &self), 
                false => note.is_path_exists(&subject, &self), 
            }; 

            if ok_status {
                valid_notes.push(note.clone());
            }
        }

        Ok(valid_notes)
    }

    /// Get the notes in the shelf filesystem. 
    pub fn get_notes_in_fs(&self, subject: &Subject) -> Result<Vec<Note>> {
        let mut notes: Vec<Note> = vec![];

        let subject_path = subject.path_in_shelf(&self);

        let tex_files = globwalk::GlobWalkerBuilder::new(subject_path, "*.tex").build().map_err(Error::GlobParsingError)?;

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

    /// Returns the associated note instances from the database with its ID. 
    /// 
    /// This does not check if the note instances are present in the filesystem. 
    pub fn get_notes_by_id (&self, note_ids: &Vec<i64>) -> Result<Vec<Note>> {
        let mut valid_notes: Vec<Note> = vec![];

        for &id in note_ids.iter() {
            match self.get_note_by_id(id)? {
                Some(v) => valid_notes.push(v), 
                None => continue, 
            }
        }

        Ok(valid_notes)
    }

    /// Returns all of the note instances in the database. 
    /// 
    /// This does not check if the note instances are present in the filesystem as well. 
    pub fn get_all_notes_from_db (&self, sort: Option<&str>) -> Result<Vec<Note>> {
        let mut sql_string = String::from("SELECT title FROM notes");
        match sort {
            Some(order) => {
                sql_string.push_str(" ORDER BY ");
                sql_string.push_str(order);
            }, 
            None => (), 
        }

        let mut valid_notes: Vec<Note> = vec![];

        let db_conn = self.open_db()?;
        let mut select_stmt = db_conn.prepare(&sql_string).map_err(Error::DatabaseError)?;
        let mut result_rows = select_stmt.query(rusqlite::NO_PARAMS).map_err(Error::DatabaseError)?;
        while let Some(note_row) = result_rows.next().map_err(Error::DatabaseError)? {
            let title: String = note_row.get(0).map_err(Error::DatabaseError)?;

            valid_notes.push(Note::new(title));
        }

        Ok(valid_notes)
    }

    /// Get all of the associated note instances of a subject from the database. 
    /// 
    /// This does not verify if the note instances are present in the filesystem. 
    /// 
    /// If the shelf has no database, it will return an empty vector. 
    pub fn get_all_notes_by_subject_from_db (&self, subject: &Subject, sort: Option<&str>) -> Result<Vec<Note>> {
        let mut select_string = String::from("SELECT title FROM notes WHERE subject_id == ?");
        match sort {
            Some(order) => {
                select_string.push_str(" ORDER BY ");
                select_string.push_str(order);
            },
            None => (), 
        };

        let subject_id = match self.get_subject_id(&subject)? {
            Some(v) => v, 
            None => return Err(Error::ValueError), 
        };

        let mut valid_notes: Vec<Note> = vec![];

        let db_conn = self.open_db()?;
        let mut select_stmt = db_conn.prepare(&select_string).map_err(Error::DatabaseError)?;
        let mut result_rows = select_stmt.query(&[&subject_id.to_string()]).map_err(Error::DatabaseError)?;

        while let Some(note_row) = result_rows.next().map_err(Error::DatabaseError)? {
            let title: String = note_row.get(0).map_err(Error::DatabaseError)?;

            let note = Note::new(title);

            valid_notes.push(note);
        }

        Ok(valid_notes)
    }

    /// Create the note in the shelf in the filesystem. 
    /// 
    /// If specified, it can also add the note in the shelf database. 
    /// 
    /// By default, the method will not return an error if it's already exported. 
    /// However, you can set the method to be strict on it. 
    pub fn create_note (
        &self, 
        subject: &Subject, 
        note: &Note, 
        value: &str, 
        export_options: &ExportOptions,
    ) -> Result<()> {
        let ok_status = note.export(&subject, &self, &value).is_ok() || (subject.is_valid(&self) && !export_options.strict);

        if ok_status && export_options.include_in_db {
            self.create_note_entry(&subject, &note)?;
        }

        Ok(())
    }

    /// Creates the files of the note instances in the shelf. 
    pub fn create_notes (
        &self, 
        subject: &Subject, 
        notes: &Vec<Note>, 
        value: &str, 
        export_options: &ExportOptions,
    ) -> Result<Vec<Note>> {
        let mut valid_notes: Vec<Note> = vec![];

        for note in notes.iter() {
            if self.create_note(&subject, &note, &value, &export_options).is_ok() {
                valid_notes.push(note.clone());
            }
        }

        Ok(valid_notes)
    }

    /// Create a note entry in the shelf database. 
    pub fn create_note_entry (&self, subject: &Subject, note: &Note) -> Result<i64> {
        let db_conn = self.open_db()?;
        let mut insert_note_stmt = db_conn.prepare("INSERT INTO notes (subject_id, title, slug) VALUES (?, ?, ?)").map_err(Error::DatabaseError)?;

        let subject_id = self.get_subject_id(&subject)?;
        if subject_id.is_none() {
            return Err(Error::ValueError);
        }

        insert_note_stmt.insert(&[&subject_id.unwrap().to_string(), &note.title(), &helpers::string::kebab_case(&note.title())]).map_err(Error::DatabaseError)
    }

    /// Deletes the entry and filesystem of the note instances in the shelf. 
    pub fn delete_notes (&self, subject: &Subject, notes: &Vec<Note>) -> Result<Vec<Note>> {
        let mut valid_notes: Vec<Note> = vec![];

        for note in notes.iter() {
            // checking for the error
            match self.delete_note_entry(&subject, &note).err() {
                Some(e) => match e {
                    Error::NoShelfDatabase (_path) => (), 
                    _ => continue, 
                }, 
                None => (), 
            }

            if note.delete(&subject, self).is_ok() {
                valid_notes.push(note.clone());
            }
        }

        Ok(valid_notes)
    }

    pub fn delete_note_entry (&self, subject: &Subject, note: &Note) -> Result<usize> {
        let note_id = match self.get_note_id(&subject, &note)? {
            Some(v) => v, 
            None => return Err(Error::ValueError), 
        };
        
        let db_conn = self.open_db()?;
        let mut delete_note_stmt = db_conn.prepare("DELETE FROM notes WHERE id == ?").map_err(Error::DatabaseError)?;
        delete_note_stmt.execute(&[&note_id]).map_err(Error::DatabaseError)
    }

    // TODO: Update operation for the subjects and the notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_note_usage() -> Result<()> {
        let note_path = PathBuf::from("tests/notes");
        fs::remove_dir_all(&note_path);
        let mut test_case_builder = ShelfBuilder::new();
        test_case_builder.path(note_path).use_db(true);
        let mut test_case = test_case_builder.build()?;

        let mut export_options = ExportOptions::new();
        export_options.include_in_db(true).with_metadata(true);

        assert!(test_case.export(&export_options).is_ok());

        let test_subject_input = Subject::from_vec_loose(&vec!["Calculus", "Algebra"], &test_case)?;
        let test_note_input = Note::from_vec_loose(&vec!["Precalculus Quick Review", "Introduction to Integrations", "Introduction to Limits"], &test_subject_input[0], &test_case)?;

        let created_subjects = test_case.create_subjects(&test_subject_input, &export_options)?;
        assert_eq!(created_subjects.len(), 2);

        let created_notes = test_case.create_notes(&test_subject_input[0], &test_note_input, consts::NOTE_TEMPLATE, &export_options)?;
        assert_eq!(created_notes.len(), 3);

        let available_subjects = test_case.get_subjects(&test_subject_input, true)?;
        assert_eq!(available_subjects.len(), 2);

        let available_notes = test_case.get_notes(&test_subject_input[0], &test_note_input, true)?;
        assert_eq!(available_notes.len(), 3);

        let all_available_subjects = test_case.get_all_subjects_from_db(None)?;
        assert_eq!(all_available_subjects.len(), 2);

        let all_available_notes = test_case.get_all_notes_by_subject_from_db(&test_subject_input[0], None)?;
        assert_eq!(all_available_notes.len(), 3);

        let all_available_notes_from_fs = test_case.get_notes_in_fs(&test_subject_input[0])?;
        assert_eq!(all_available_notes_from_fs.len(), 3);

        let deleted_notes = test_case.delete_notes(&test_subject_input[0], &test_note_input)?;
        assert_eq!(deleted_notes.len(), 3);

        let deleted_subjects = test_case.delete_subjects(&test_subject_input)?;
        assert_eq!(deleted_subjects.len(), 2);

        Ok(())
    }

    #[test]
    fn subject_instances_test() -> Result<()> {
        let note_path = PathBuf::from("tests/subjects");
        fs::remove_dir_all(&note_path);
        let mut test_case_builder = ShelfBuilder::new();
        test_case_builder.path(note_path).use_db(true);

        let mut test_case = test_case_builder.build()?;

        let mut export_options: ExportOptions = ExportOptions::new();
        export_options.include_in_db(true).with_metadata(true);
        
        assert!(test_case.export(&export_options).is_ok());

        let test_subject: Subject = Subject::new("Mathematics".to_string());

        assert_eq!(test_subject.is_valid(&test_case), false);
        assert_eq!(test_subject.is_entry_exists(&test_case)?, false);
        assert_eq!(test_subject.is_sync(&test_case), false);

        test_subject.export(&test_case, export_options.with_metadata)?;

        assert_eq!(test_subject.is_valid(&test_case), true);
        assert_eq!(test_subject.is_path_exists(&test_case), true);
        assert_eq!(test_subject.is_entry_exists(&test_case)?, false);

        test_case.create_subjects(&vec![test_subject.clone()], &export_options)?;

        assert_eq!(test_subject.is_valid(&test_case), true);
        assert_eq!(test_subject.is_path_exists(&test_case), true);
        assert_eq!(test_subject.is_entry_exists(&test_case)?, true);

        Ok(())
    }

    #[test]
    #[should_panic]
    fn invalid_note_export() {
        let note_path = PathBuf::from("./test/invalid/location/is/invalid");
        let mut test_case_builder = ShelfBuilder::new();
        test_case_builder.path(note_path).use_db(false);
        
        let mut test_case = test_case_builder.build().unwrap();

        assert!(test_case.export(&ExportOptions::new()).is_ok());
    }

    #[test]
    #[should_panic]
    fn invalid_note_import() {
        let note_path = PathBuf::from("./this/is/invalid/note/location/it/does/not/exists/lol");
        
        assert!(Shelf::from(note_path).is_ok())
    }
}
