use std::collections::HashMap;
use std::fs::{ self, DirBuilder, OpenOptions };
use std::path::{ self, PathBuf, Path };
use std::result::Result;
use std::io;
use std::io::Write;
use std::rc;

use handlebars;
use rusqlite;
use rusqlite::vtab::array;
use rusqlite::{ Connection };
use chrono::{ self };
use serde::{ Deserialize, Serialize };
use serde_json;

use crate::consts;
use crate::helpers;
use crate::error::Error;

// even though string literals are always static, 
// it is better to anotate them for explicit intentions
const NOTES_FOLDER: &str = "notes";
const SUBJECT_METADATA_FILE: &str = "info.json";
const DB_NAME: &str = "notes";
const DB_FILE_EXTENSION: &str = "db";

/// A subject where it can contain notes or other subjects. 
/// 
/// In the filesystem, a subject is a folder with a specific metadata file (`info.json`). 
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Subject {
    pub name: String, 
    datetime_modified: chrono::DateTime<chrono::Local>, 

    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>, 
}

impl Subject {
    /// Creates a new subject instance. 
    pub fn new (name: String) -> Self {
        Subject {
            name, 
            datetime_modified: chrono::Local::now(), 
            extra: HashMap::new()
        }
    }

    /// Create a subject instance from a given notes instance. 
    /// If the path is a valid subject folder, it will set the appropriate data from the metadata file and return with an `Option` field. 
    pub fn from (name: &str, notes: &Shelf) -> Result<Option<Self>, Error> {
        let subject = Subject::new(name.to_string());
        
        if !subject.is_path_exists(&notes) {
            return Ok(None)
        }

        let metadata_path = subject.metadata_path(&notes);
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

    /// Searches for the subjects in the given shelf. 
    /// 
    /// All nonexistent subjects are created as a new subject instance instead. 
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

    /// Returns the associated path with the given shelf.
    pub fn path (&self, notes: &Shelf) -> PathBuf {
        let mut path = notes.path.clone();
        let subject_slug = helpers::kebab_case(&self.name);
        path.push(subject_slug);

        path
    }

    /// Returns the associated metadata file path with the given shelf. 
    pub fn metadata_path (&self, notes: &Shelf) -> PathBuf {
        let mut path = self.path(&notes);
        path.push(SUBJECT_METADATA_FILE);

        path
    }

    /// Creates a string of the datetime as an ISO string (or equivalent to "%F" in `strftime`). 
    pub fn date_iso_string (&self) -> String {
        self.datetime_modified.format("%F").to_string()
    }

    /// Exports the instance in the filesystem. 
    pub fn export(&self, notes: &Shelf) -> Result<(), Error> {
        let path = self.path(&notes);
        let dir_builder = DirBuilder::new();
        
        helpers::create_folder(&dir_builder, &path)?;
        
        let metadata_path = self.metadata_path(&notes);
        let mut metadata_file = OpenOptions::new().create_new(true).write(true).open(metadata_path).map_err(Error::IoError)?;
        metadata_file.write(serde_json::to_string_pretty(&self).map_err(Error::SerdeValueError)?.as_bytes()).map_err(Error::IoError)?;

        Ok(())
    }

    /// Deletes the associated folder in the shelf filesystem. 
    pub fn delete(&self, notes: &Shelf) -> Result<(), Error> {
        let path = self.path(&notes);
        
        fs::remove_dir_all(path).map_err(Error::IoError)
    }

    /// Checks if the
    pub fn is_path_exists (&self, notes: &Shelf) -> bool {
        let path = self.metadata_path(&notes);

        path.is_file()
    }
    
    pub fn is_entry_exists (&self, notes: &Shelf) -> Result<bool, Error> {
        let id = notes.get_subject_id(&self)?;

        match id {
            Some(_id) => Ok(true), 
            None => Ok(false), 
        }
    }

    pub fn is_sync (&self, notes: &Shelf) -> bool {
        self.is_path_exists(&notes) && match self.is_entry_exists(notes) {
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
    pub title: String, 
    datetime_modified: chrono::DateTime<chrono::Local>, 
}

impl Note {
    /// Creates a new note instance. 
    pub fn new(title: String, datetime: Option<chrono::DateTime<chrono::Local>>) -> Self {
        Note {
            title, 
            datetime_modified: datetime.unwrap_or(chrono::Local::now()), 
        }
    }

    /// Searches for the note in the shelf. 
    /// 
    /// This only checks whether the associated path of the note exists. 
    /// To check if the note exists on the notes database, call the `Note::is_entry_exists` method. 
    pub fn from (title: &str, subject: &Subject, notes: &Shelf) -> Result<Option<Self>, Error> {
        let note = Note::new(title.to_string(), None);

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
                None => Note::new(title.to_string(), None), 
            };

            notes_vector.push(note_instance);
        }

        Ok(notes_vector)
    }

    /// Returns the file name of the note instance along with its associated subject. 
    /// 
    /// It does not necessarily mean that the note exists. 
    /// Be sure to check it first. 
    pub fn path(&self, subject: &Subject, notes: &Shelf) -> PathBuf {
        let mut path = subject.path(&notes);
        let slug = helpers::kebab_case(&self.title);

        path.push(slug);
        path.set_extension("tex");

        path
    }

    /// Writes the resulting LaTeX file in the filesystem. 
    /// 
    /// For templating, it uses [a Rust implementation of Handlebars](https://github.com/sunng87/handlebars-rust). 
    /// The configuration of Handlebars does not escape anything (uses [`handlebars::no_escape`](https://docs.rs/handlebars/3.0.0-beta.1/handlebars/fn.no_escape.html)). 
    pub fn export (&self, subject: &Subject, notes: &Shelf, value: &serde_json::Map<String, serde_json::Value>) -> Result<(), Error> {
        let path = self.path(&subject, &notes);

        let mut value = value.clone();
        value.insert("title".to_string(), json!(self.title));
        value.insert("date".to_string(), json!(self.datetime_modified.format("%F").to_string()));

        let mut note_file = OpenOptions::new().create_new(true).write(true).open(path).map_err(Error::IoError)?;

        let mut template_registry = handlebars::Handlebars::new();
        template_registry.register_escape_fn(handlebars::no_escape);
        template_registry.register_template_string("tex_note", consts::NOTE_TEMPLATE).map_err(Error::HandlebarsTemplateError)?;
        let rendered_string = template_registry.render("tex_note", &value).map_err(Error::HandlebarsRenderError)?;
        note_file.write(rendered_string.as_bytes()).map_err(Error::IoError)?;

        Ok(())
    }

    /// Simply deletes the file in the shelf filesystem. 
    /// 
    /// This does not delete the entry in the notes database. 
    pub fn delete (&self, subject: &Subject, notes: &Shelf) -> Result<(), Error> {
        let path = self.path(&subject, &notes);

        fs::remove_file(path).map_err(Error::IoError)
    }

    /// Checks for the file if it exists in the shelf. 
    pub fn is_path_exists (&self, subject: &Subject, notes: &Shelf) -> bool {
        let path = self.path(&subject, &notes);

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
        self.is_path_exists(&subject, &notes) && match self.is_entry_exists(&subject, notes) {
            Ok(v) => v, 
            Err(_e) => false, 
        }
    }
}

/// The shelf is where it contains the subjects and its notes. 
/// 
/// It is where the major operations on the database occur. 
pub struct Shelf {
    path: PathBuf, 
    db: Option<Connection>, 
}

impl Shelf {
    /// Create a new shelf instance. 
    /// Since it stores the database connection where operations can occur, it has to be mutable. 
    /// 
    /// The shelf is mainly where the database operations will occur (if it's enabled). 
    /// It also serves as a convenience object for the atomic operations in the filesystem. 
    pub fn new (path: PathBuf, use_database: bool) -> Result<Self, Error> {
        let mut notes_object = Shelf { path: path.clone(), db: None };

        if !path.exists() {
            let dir_builder = DirBuilder::new();
            helpers::create_folder(&dir_builder, &path)?;
        }

        match use_database {
            true => Some(notes_object.set_db()?), 
            false => None
        };

        Ok(notes_object)
    }

    /// Creates a shelf instance from the path if the associated database file exists. 
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

    /// Returns the associated path of the database. 
    pub fn db_path (&self) -> PathBuf {
        let mut db_path: PathBuf = self.path.clone();
        db_path.push(NOTES_FOLDER);
        db_path.set_file_name(DB_NAME);
        db_path.set_extension(DB_FILE_EXTENSION);

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
            None => Err(Error::ValueError) 
        }
    }

    /// Gets the associated subject ID in the database. 
    pub fn get_subject_id (&self, subject: &Subject) -> Result<Option<i64>, Error> {
        let subject_slug = helpers::kebab_case(&subject.name);
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

        let note_title_slug = helpers::kebab_case(&note.title);
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

                Ok(Some(Note::new(name, Some(datetime_modified))))
            }, 
            None => Ok(None)
        }
    }

    /// Gets the associated subject instance with the one of the note ID in the database. 
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
                false => subject.is_path_exists(&self), 
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
            match Subject::from(&name, &*self)? {
                Some(subject) => valid_subjects.push(subject), 
                None => continue, 
            }
        }

        Ok(valid_subjects)
    }

    /// Creates its folder structure on the filesystem. 
    /// It can also add the subject instance in the database, if specified. 
    /// 
    /// Returns the subject instance that succeeded in its creation process. 
    pub fn create_subjects(&self, subjects: &Vec<Subject>, add_to_db: bool) -> Result<Vec<Subject>, Error> {
        let mut valid_subjects: Vec<Subject> = vec![];

        for subject in subjects.iter() {
            if add_to_db && self.use_db() {
                self.create_subject_entry(&subject)?;
            }

            if subject.export(&self).is_ok() {
                valid_subjects.push(subject.clone());
            }
        }

        Ok(valid_subjects)
    }

    fn create_subject_entry(&self, subject: &Subject) -> Result<i64, Error> {
        let mut insert_subject_stmt = self.db_prepare("INSERT INTO subjects (name, slug, datetime_modified) VALUES (?, ?, ?)")?;

        insert_subject_stmt.insert(&[&subject.name, &helpers::kebab_case(&subject.name), &subject.datetime_modified.to_rfc3339()]).map_err(Error::DatabaseError)
    }

    /// Deletes the entry and the filesystem of the subject instance in the database. 
    pub fn delete_subjects (&self, subjects: &Vec<Subject>) -> Result<Vec<Subject>, Error> {
        let mut valid_subjects: Vec<Subject> = vec![];

        for subject in subjects.iter() {
            if self.use_db() {
                self.delete_subject_entry(&subject)?;
            }

            if subject.delete(self).is_ok() {
                valid_subjects.push(subject.clone());
            }
        }

        Ok(valid_subjects)
    }

    fn delete_subject_entry (&self, subject: &Subject) -> Result<usize, Error> {
        let mut delete_subject_stmt = self.db_prepare("DELETE FROM subjects WHERE name == ?")?;

        delete_subject_stmt.execute(&[&subject.name]).map_err(Error::DatabaseError)
    }

    /// Get the valid notes in the shelf. 
    /// It can also check if the notes are in the shelf database. 
    pub fn get_notes (&self, subject: &Subject, notes: &Vec<Note>, sync: bool) -> Result<Vec<Note>, Error> {
        let mut valid_notes: Vec<Note> = vec![];

        for note in notes.iter() {
            let ok_status = match sync {
                true => note.is_sync(&subject, self), 
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

            valid_notes.push(Note::new(title, Some(datetime_modified)));
        }

        Ok(valid_notes)
    }

    /// Get all of the associated note instances of a subject from the database. 
    /// 
    /// This does not verify if the note instances are present in the filesystem. 
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

            let note = Note::new(title, Some(datetime_modified));

            valid_notes.push(note);
        }

        Ok(valid_notes)
    }

    /// Creates the files of the note instances in the shelf. 
    pub fn create_notes (&self, subject: &Subject, notes: &Vec<Note>, value: &serde_json::Map<String, serde_json::Value>, add_to_db: bool) -> Result<Vec<Note>, Error> {
        let mut valid_notes: Vec<Note> = vec![];

        for note in notes.iter() {
            if add_to_db && self.use_db() {
                self.create_note_entry(&subject, &note)?;
            }

            match note.export(&subject, &self, &value) {
                Ok(v) => valid_notes.push(note.clone()), 
                Err(e) => return Err(e), 
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

        insert_note_stmt.insert(&[&subject_id.unwrap().to_string(), &note.title, &helpers::kebab_case(&note.title), &note.datetime_modified.to_rfc3339()]).map_err(Error::DatabaseError)
    }

    /// Deletes the entry and filesystem of the note instances in the shelf. 
    pub fn delete_notes (&self, subject: &Subject, notes: &Vec<Note>) -> Result<Vec<Note>, Error> {
        let mut valid_notes: Vec<Note> = vec![];

        for note in notes.iter() {
            if self.use_db() {
                self.delete_note_entry(&subject, &note)?;
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

    // TODO: Improve test case
    #[test]
    fn basic_note_usage() -> Result<(), Error> {
        let note_path = PathBuf::from("./tests/notes");
        fs::remove_dir_all(&note_path);
        let test_case: Shelf = Shelf::new(note_path, true)?;

        let test_subject_input = Subject::from_vec_loose(&vec!["Calculus", "Algebra"], &test_case)?;
        let test_note_input = Note::from_vec_loose(&vec!["Precalculus Quick Review", "Introduction to Integrations"], &test_subject_input[0], &test_case)?;

        let test_note_value = json!({ "author": "Gabriel Arazas" });

        let created_subjects = test_case.create_subjects(&test_subject_input, true)?;
        assert_eq!(created_subjects.len(), 2);

        let created_notes = test_case.create_notes(&test_subject_input[0], &test_note_input, &test_note_value.as_object().unwrap(), true)?;
        assert_eq!(created_notes.len(), 2);

        let available_subjects = test_case.get_subjects(&test_subject_input, true)?;
        assert_eq!(available_subjects.len(), 2);

        let available_notes = test_case.get_notes(&test_subject_input[0], &test_note_input, true)?;
        assert_eq!(available_notes.len(), 2);

        let deleted_notes = test_case.delete_notes(&test_subject_input[0], &test_note_input)?;
        assert_eq!(deleted_notes.len(), 2);

        let deleted_subjects = test_case.delete_subjects(&test_subject_input)?;
        assert_eq!(deleted_subjects.len(), 2);

        Ok(())
    }

    #[test]
    #[should_panic]
    fn invalid_note_location() {
        let note_path = PathBuf::from("./test/invalid/location/is/invalid");
        let mut test_case: Shelf = Shelf::new(note_path, false).unwrap();

        ()
    }
}
