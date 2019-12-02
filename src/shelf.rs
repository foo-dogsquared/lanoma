use std::fs::{ self, DirBuilder };
use std::path::{ self, PathBuf, Path };

use rusqlite;
use rusqlite::vtab::array;
use rusqlite::{ Connection };
use chrono::{ self };
use serde_json;

use crate::consts;
use crate::helpers;
use crate::error::Error;
use crate::notes::{ Note, Subject };

// even though string literals are always static, 
// it is better to anotate them for explicit intentions
const DB_NAME: &str = "notes.db";

/// The shelf is where it contains the subjects and its notes. 
/// 
/// It is where the major operations on the database occur. 
#[derive(Debug)]
pub struct Shelf {
    path: PathBuf, 
    db: Option<Connection>, 
}

impl Shelf {
    /// Create a new shelf instance and immediately being created in the filesystem. 
    /// 
    /// If you want to import a shelf instance from the filesystem, use `Shelf::from`. 
    pub fn new (path: PathBuf, use_database: bool) -> Result<Self, Error> {
        let mut notes_object = Shelf { path: path.clone(), db: None };

        if !notes_object.is_exported() {
            let dir_builder = DirBuilder::new();
            helpers::filesystem::create_folder(&dir_builder, notes_object.path())?;
        }

        if use_database {
            notes_object.set_db()?;
        }

        Ok(notes_object)
    }

    /// Creates a shelf instance from the path. 
    pub fn from (path: PathBuf) -> Result<Self, Error> {
        let mut notes_object = Shelf { path: path.clone(), db: None };

        if !path.exists() {
            return Err(Error::ValueError);
        }

        if notes_object.db_path().exists() {
            notes_object.set_db()?;
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
    pub fn set_path<P: AsRef<Path>> (&mut self, to: P) -> Result<PathBuf, Error> {
        let old_path = self.path();
        let new_path = to.as_ref().to_path_buf();

        self.path = new_path;

        if self.is_exported() {
            fs::rename(&old_path, &self.path).map_err(Error::IoError)?;
        }

        Ok(old_path)
    }

    /// Returns the associated path of the database. 
    pub fn db_path (&self) -> PathBuf {
        let mut db_path: PathBuf = self.path.clone();
        db_path.push(DB_NAME);

        db_path
    }

    /// Set up the associated database of the shelf. 
    /// It also means the database support for the shelf is enabled. 
    fn set_db (&mut self) -> Result<(), Error> {
        let db_path = self.db_path();
 
        let db: Connection = Connection::open(&db_path.into_os_string()).map_err(Error::DatabaseError)?;
        array::load_module(&db).map_err(Error::DatabaseError)?;
        db.execute_batch(consts::SQLITE_SCHEMA).map_err(Error::DatabaseError)?;

        self.db = Some(db);

        Ok(())
    }

    /// Checks if the shelf database is enabled. 
    pub fn use_db (&self) -> bool {
        self.db.is_some()
    }

    fn db_prepare (&self, sql_string: &str) -> Result<rusqlite::Statement, Error> {
        let db = self.db.as_ref();
        match db {
            Some(db) => Ok(db.prepare(sql_string).map_err(Error::DatabaseError)?), 
            None => Err(Error::NoShelfDatabase(self.path())), 
        }
    }

    /// Checks if the shelf is exported in the filesystem.
    pub fn is_exported (&self) -> bool {
        self.path.exists()
    }

    /// Gets the associated subject ID in the database. 
    /// 
    /// This does not check if the subject is in the filesystem. 
    pub fn get_subject_id (&self, subject: &Subject) -> Result<Option<i64>, Error> {
        let subject_slug = helpers::string::kebab_case(&subject.name());
        let mut select_stmt = self.db_prepare("SELECT id FROM SUBJECTS WHERE slug == ?")?;
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
    pub fn get_subject_by_id (&self, id: i64) -> Result<Option<Subject>, Error> {
        let mut select_stmt = self.db_prepare("SELECT name FROM subjects WHERE id == ?")?;
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
    pub fn get_note_id(&self, subject: &Subject, note: &Note) -> Result<Option<i64>, Error> {
        let subject_id = match self.get_subject_id(&subject)? {
            Some(id) => id, 
            None => return Err(Error::ValueError), 
        };

        let note_title_slug = helpers::string::kebab_case(&note.title());
        let mut select_stmt = self.db_prepare("SELECT id FROM notes WHERE subject_id == ? AND slug == ?")?;
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
    pub fn get_note_by_id (&self, id: i64) -> Result<Option<Note>, Error> {
        let mut select_stmt = self.db_prepare("SELECT name, datetime_modified FROM subjects WHERE id == ?")?;
        let mut result_row = select_stmt.query(&[&id]).map_err(Error::DatabaseError)?;

        match result_row.next().map_err(Error::DatabaseError)? {
            Some(row) => {
                let name = row.get(0).map_err(Error::DatabaseError)?;
                let datetime_modified: chrono::DateTime<chrono::Local> = row.get(1).map_err(Error::DatabaseError)?;

                Ok(Some(Note::new(name)))
            }, 
            None => Ok(None)
        }
    }

    /// Gets the associated subject instance with the one of the note ID in the database. 
    /// 
    /// This does not check if the subject is in the shelf filesystem. 
    pub fn get_subject_by_note_id (&self, id: i64) -> Result<Option<Subject>, Error> {
        let mut select_stmt = self.db_prepare("SELECT subject_id FROM notes WHERE id == ?")?;
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
    pub fn get_subjects(&self, subjects: &Vec<Subject>, sync: bool) -> Result<Vec<Subject>, Error> {
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
    pub fn get_subjects_by_id (&self, subject_ids: &Vec<i64>, sort: Option<&str>) -> Result<Vec<Subject>, Error> {
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
    pub fn get_all_subjects_from_db (&self, sort: Option<&str>) -> Result<Vec<Subject>, Error> {
        let mut select_stmt = String::from("SELECT name, datetime_modified FROM subjects");
        match sort {
            Some(v) => {
                select_stmt.push_str(" ORDER BY ");
                select_stmt.push_str(v);
            }
            None => (), 
        };

        let mut select_stmt = self.db_prepare(&select_stmt)?;
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
        add_to_db: bool, 
        strict: bool, 
    ) -> Result<Vec<Subject>, Error> {
        let mut valid_subjects: Vec<Subject> = vec![];

        for subject in subjects.iter() {
            // a subject that already exists in the shelf is considered to be "created"
            // this is for considering when the already existing subject is considered to be added in the shelf database
            // though, an available strict mode is made for that case
            let ok_status = subject.export(&self).is_ok() || (subject.is_valid(&self) && !strict);
            
            if ok_status && add_to_db {
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

    fn create_subject_entry(&self, subject: &Subject) -> Result<i64, Error> {
        let mut insert_subject_stmt = self.db_prepare("INSERT INTO subjects (name, slug, datetime_modified) VALUES (?, ?, ?)")?;

        insert_subject_stmt.insert(&[&subject.name(), &helpers::string::kebab_case(&subject.name()), &subject.datetime_modified(&self)?.to_rfc3339()]).map_err(Error::DatabaseError)
    }

    /// Delete the subject in the shelf. 
    pub fn delete_subject (&self, subject: &Subject) -> Result<(), Error> {
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
    pub fn delete_subjects (&self, subjects: &Vec<Subject>) -> Result<Vec<Subject>, Error> {
        let mut valid_subjects: Vec<Subject> = vec![];

        for subject in subjects.iter() {            
            if self.delete_subject(&subject).is_ok() {
                valid_subjects.push(subject.clone());
            }
        }

        Ok(valid_subjects)
    }

    fn delete_subject_entry (&self, subject: &Subject) -> Result<usize, Error> {
        let mut delete_subject_stmt = self.db_prepare("DELETE FROM subjects WHERE name == ?")?;

        delete_subject_stmt.execute(&[&subject.name()]).map_err(Error::DatabaseError)
    }

    /// Get the valid notes in the shelf. 
    /// It can also check if the notes are in the shelf database. 
    pub fn get_notes (&self, subject: &Subject, notes: &Vec<Note>, sync: bool) -> Result<Vec<Note>, Error> {
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

    /// Returns the associated note instances from the database with its ID. 
    /// 
    /// This does not check if the note instances are present in the filesystem. 
    pub fn get_notes_by_id (&self, note_ids: &Vec<i64>) -> Result<Vec<Note>, Error> {
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
    pub fn get_all_notes_from_db (&self, sort: Option<&str>) -> Result<Vec<Note>, Error> {
        let mut sql_string = String::from("SELECT title, datetime_modified FROM notes");
        match sort {
            Some(order) => {
                sql_string.push_str(" ORDER BY ");
                sql_string.push_str(order);
            }, 
            None => (), 
        }

        let mut valid_notes: Vec<Note> = vec![];

        let mut select_stmt = self.db_prepare(&sql_string)?;
        let mut result_rows = select_stmt.query(rusqlite::NO_PARAMS).map_err(Error::DatabaseError)?;
        while let Some(note_row) = result_rows.next().map_err(Error::DatabaseError)? {
            let title: String = note_row.get(0).map_err(Error::DatabaseError)?;
            let datetime_modified: chrono::DateTime<chrono::Local> = note_row.get(1).map_err(Error::DatabaseError)?;

            valid_notes.push(Note::new(title));
        }

        Ok(valid_notes)
    }

    /// Get all of the associated note instances of a subject from the database. 
    /// 
    /// This does not verify if the note instances are present in the filesystem. 
    /// 
    /// If the shelf has no database, it will return an empty vector. 
    pub fn get_all_notes_by_subject_from_db (&self, subject: &Subject, sort: Option<&str>) -> Result<Vec<Note>, Error> {
        let mut select_string = String::from("SELECT title, datetime_modified FROM notes WHERE subject_id == ?");
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
        let mut select_stmt = self.db_prepare(&select_string)?;
        let mut result_rows = select_stmt.query(&[&subject_id.to_string()]).map_err(Error::DatabaseError)?;

        while let Some(note_row) = result_rows.next().map_err(Error::DatabaseError)? {
            let title: String = note_row.get(0).map_err(Error::DatabaseError)?;
            let datetime_modified: chrono::DateTime<chrono::Local> = note_row.get(1).map_err(Error::DatabaseError)?;

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
        add_to_db: bool, 
        strict: bool, 
    ) -> Result<(), Error> {
        let ok_status = note.export(&subject, &self, &value).is_ok() || (subject.is_valid(&self) && !strict);

        if ok_status && add_to_db {
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
        add_to_db: bool, 
        strict: bool, 
    ) -> Result<Vec<Note>, Error> {
        let mut valid_notes: Vec<Note> = vec![];

        for note in notes.iter() {
            if self.create_note(&subject, &note, &value, add_to_db, strict).is_ok() {
                valid_notes.push(note.clone());
            }
        }

        Ok(valid_notes)
    }

    fn create_note_entry (&self, subject: &Subject, note: &Note) -> Result<i64, Error> {
        let mut insert_note_stmt = self.db_prepare("INSERT INTO notes (subject_id, title, slug, datetime_modified) VALUES (?, ?, ?, ?)")?;

        let subject_id = self.get_subject_id(&subject)?;
        if subject_id.is_none() {
            return Err(Error::ValueError);
        }

        insert_note_stmt.insert(&[&subject_id.unwrap().to_string(), &note.title(), &helpers::string::kebab_case(&note.title()), &note.datetime_modified(&subject, &self)?.to_rfc3339()]).map_err(Error::DatabaseError)
    }

    /// Deletes the entry and filesystem of the note instances in the shelf. 
    pub fn delete_notes (&self, subject: &Subject, notes: &Vec<Note>) -> Result<Vec<Note>, Error> {
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

    fn delete_note_entry (&self, subject: &Subject, note: &Note) -> Result<usize, Error> {
        let note_id = match self.get_note_id(&subject, &note)? {
            Some(v) => v, 
            None => return Err(Error::ValueError), 
        };
        
        let mut delete_note_stmt = self.db_prepare("DELETE FROM notes WHERE id == ?")?;
        delete_note_stmt.execute(&[&note_id]).map_err(Error::DatabaseError)
    }

    // TODO: Update operation for the subjects and the notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_note_usage() -> Result<(), Error> {
        let note_path = PathBuf::from("./tests/notes");
        fs::remove_dir_all(&note_path);
        let test_case: Shelf = Shelf::new(note_path, true)?;

        let test_subject_input = Subject::from_vec_loose(&vec!["Calculus", "Algebra"], &test_case)?;
        let test_note_input = Note::from_vec_loose(&vec!["Precalculus Quick Review", "Introduction to Integrations"], &test_subject_input[0], &test_case)?;

        let created_subjects = test_case.create_subjects(&test_subject_input, true, false)?;
        assert_eq!(created_subjects.len(), 2);

        let created_notes = test_case.create_notes(&test_subject_input[0], &test_note_input, consts::NOTE_TEMPLATE, true, false)?;
        assert_eq!(created_notes.len(), 2);

        let available_subjects = test_case.get_subjects(&test_subject_input, true)?;
        assert_eq!(available_subjects.len(), 2);

        let available_notes = test_case.get_notes(&test_subject_input[0], &test_note_input, true)?;
        assert_eq!(available_notes.len(), 2);

        let all_available_subjects = test_case.get_all_subjects_from_db(None)?;
        assert_eq!(all_available_subjects.len(), 2);

        let all_available_notes = test_case.get_all_notes_by_subject_from_db(&test_subject_input[0], None)?;
        assert_eq!(all_available_notes.len(), 2);

        let deleted_notes = test_case.delete_notes(&test_subject_input[0], &test_note_input)?;
        assert_eq!(deleted_notes.len(), 2);

        let deleted_subjects = test_case.delete_subjects(&test_subject_input)?;
        assert_eq!(deleted_subjects.len(), 2);

        Ok(())
    }

    #[test]
    fn subject_instances_test() -> Result<(), Error> {
        let note_path = PathBuf::from("./tests/subjects");
        fs::remove_dir_all(&note_path);
        let test_case: Shelf = Shelf::new(note_path, true)?;

        let test_subject: Subject = Subject::new("Mathematics".to_string());

        assert_eq!(test_subject.is_valid(&test_case), false);
        assert_eq!(test_subject.is_entry_exists(&test_case)?, false);
        assert_eq!(test_subject.is_sync(&test_case), false);

        test_subject.export(&test_case)?;

        assert_eq!(test_subject.is_valid(&test_case), true);
        assert_eq!(test_subject.is_path_exists(&test_case), true);
        assert_eq!(test_subject.is_entry_exists(&test_case)?, false);

        test_case.create_subjects(&vec![test_subject.clone()], true, false)?;

        assert_eq!(test_subject.is_valid(&test_case), true);
        assert_eq!(test_subject.is_path_exists(&test_case), true);
        assert_eq!(test_subject.is_entry_exists(&test_case)?, true);

        Ok(())
    }

    #[test]
    #[should_panic]
    fn invalid_note_location() {
        let note_path = PathBuf::from("./test/invalid/location/is/invalid");
        let _test_case: Shelf = Shelf::new(note_path, false).unwrap();

        ()
    }
}
