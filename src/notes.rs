use std::collections::HashMap;
use std::error as std_error;
use std::fs::{ self, DirBuilder };
use std::path::{ self, PathBuf };
use std::result::Result;
use std::rc::Rc;
use std::io::Write;

use handlebars;
use rusqlite;
use rusqlite::types::Value;
use rusqlite::vtab::array;
use rusqlite::{ Connection };
use chrono::{ self, Local };

use crate::constants;
use crate::helpers;
use crate::error::Error;

// even though string literals are always static, 
// it is better to anotate them for explicit intentions
const NOTES_FOLDER: &str = "notes";
const DB_NAME: &str = "notes";
const DB_FILE_EXTENSION: &str = "db";

// a Scannable trait means the object is located on the filesystem and database
pub trait Scannable {
    fn is_path_exists (&self) -> bool; 
    fn is_entry_exists (&self, db: Connection) -> bool; 
    fn is_sync (&self, db: Connection) -> bool;

    // filesystem operations
    // fn open (&self) -> bool;
}

#[derive(Clone, Debug)]
pub struct Subject {
    pub id: i64, 
    pub name: String, 
    modified_datetime: chrono::DateTime<Local>, 
}

impl Subject {
    fn path(&self, notes: &Notes) -> PathBuf {
        let mut path = notes.path.clone();
        let subject_slug = helpers::kebab_case(&self.name);

        path.push(subject_slug);

        path
    }
}

impl Scannable for Subject {
    fn is_path_exists (&self) -> bool {true}
    fn is_entry_exists (&self, db: Connection) -> bool {true}
    fn is_sync (&self, db: Connection) -> bool {true}
}

#[derive(Clone, Debug)]
pub struct Note {
    pub id: i64, 
    subject_id: i64, 
    pub title: String, 
    modified_datetime: chrono::DateTime<Local>, 
}

impl Scannable for Note {
    fn is_path_exists (&self) -> bool {true}
    fn is_entry_exists (&self, db: Connection) -> bool {true}
    fn is_sync (&self, db: Connection) -> bool {true}
}

impl Note {
    pub fn path(&self, notes: &Notes, subject: &Subject) -> PathBuf {
        let mut path = subject.path(notes);
        let slug = helpers::kebab_case(&self.title);

        path.push(slug);
        path.set_extension("tex");

        path
    }
}

pub struct Notes {
    path: PathBuf, 
    db: Connection, 
}

impl Notes {
    /// Create a new notes instance of the profile. 
    /// Since it stores the database connection where operations can occur, it has to be mutable. 
    pub fn new<'a>(path: &'a PathBuf) -> Result<Notes, Box<dyn std_error::Error>> {
        let mut notes_path: PathBuf = path.clone();
        if !notes_path.ends_with(NOTES_FOLDER) {
            notes_path.push(NOTES_FOLDER);
        }

        if !notes_path.exists() {
            let mut dir_builder = DirBuilder::new();
            helpers::create_folder(&dir_builder, &notes_path)?;
        }

        let mut db_path: PathBuf = notes_path.clone();

        if db_path.is_dir() {
            db_path.push(NOTES_FOLDER);
            db_path.set_file_name(DB_NAME);
            db_path.set_extension(DB_FILE_EXTENSION);
        }
 
        let db: Connection = Connection::open(&db_path.into_os_string()).map_err(Error::DatabaseError)?;
        array::load_module(&db).map_err(Error::DatabaseError)?;        
        db.execute_batch(constants::SQLITE_SCHEMA).map_err(Error::DatabaseError)?;

        Ok(Notes { db, path: notes_path })
    }
    
    /// Create a subject in the database and the filesystem.
    pub fn create_subjects<'a>(&self, input: &Vec<&'a str>) -> Result<Vec<Subject>, Box<dyn std_error::Error>> {
        // Check for the subject 1
        let mut subject_name_stmt = self.db.prepare("INSERT INTO subjects (name, slug, datetime_modified) VALUES (?, ?, ?)").map_err(Error::DatabaseError)?;

        let mut now: chrono::DateTime<Local> = chrono::Local::now();
        let mut now_as_iso_string: String = now.to_rfc3339();
        let mut subjects: Vec<Subject> = vec![];
        let mut dir_builder = DirBuilder::new();
        dir_builder.recursive(true);
        
        // For each input, create a database entry, then a place in the filesystem 
        for subject in input.iter() {
            // Creating an entry in the database 
            let subject_slug = helpers::kebab_case(&subject);
            let row_id: i64 = subject_name_stmt.insert(&[subject, &subject_slug[..], &now_as_iso_string]).map_err(Error::DatabaseError)?;

            let subject_instance = Subject { id: row_id, name: subject.to_string(), modified_datetime: now };

            let mut subject_path: PathBuf = subject_instance.path(self);
            helpers::create_folder(&dir_builder, &subject_path)?;

            subjects.push( subject_instance );
        }

        Ok(subjects)
    }
    
    /// Retrieves a list of valid subjects in the database and the filesystem.
    pub fn read_subjects(&self, subjects: &Vec<&str>, sort: Option<&str>) -> Result<Vec<Subject>, Box<dyn std_error::Error>> {
        let mut subject_select_sql = match sort {
            Some(p) => format!("SELECT id, name, slug, datetime_modified FROM subjects WHERE name IN rarray(?) ORDER BY {}", p), 
            None => String::from("SELECT id, name, slug, datetime_modified FROM subjects WHERE name IN rarray(?)")
        };
        let mut subject_select_name_stmt = self.db.prepare_cached(&subject_select_sql).map_err(Error::DatabaseError)?;
        let mut input_names = subjects.clone().into_iter().map(| name | Value::from(name.to_string())).collect(); 
        let mut pointer = Rc::new(input_names);

        let mut subject_name_rows = subject_select_name_stmt.query(&[&pointer]).map_err(Error::DatabaseError)?;

        let mut subjects: Vec<Subject> = vec![];
        while let Some(subject_row) = subject_name_rows.next()? {
            let id: i64 = subject_row.get(0)?;
            let name: String = subject_row.get(1)?;
            let slug: String = subject_row.get(2)?;
            let datetime_modified: chrono::DateTime<Local> = subject_row.get(3)?;

            subjects.push(Subject { id, name, modified_datetime: datetime_modified });
        }

        Ok(subjects)
    }

    /// Returns a vector of valid subjects found in the database. 
    /// 
    /// ## Failure
    /// * When the database operation has gone something wrong. 
    /// * When retrieving the rows unexpectedly gave an error. 
    pub fn read_subjects_by_id(&self, subjects: &Vec<i64>, sort: Option<&str>) -> Result<Vec<Subject>, Box<dyn std_error::Error>> {
        let subjects_value_vector = subjects.clone().into_iter().map(Value::from).collect();
        let subjects_value_pointer = Rc::new(subjects_value_vector);

        let mut select_subject_stmt = self.db.prepare("SELECT id, name, datetime_modified FROM subjects WHERE id IN rarray(?)")?;
        let mut subject_rows = select_subject_stmt.query(&[&subjects_value_pointer])?;

        let mut subjects_vector: Vec<Subject> = vec![];
        while let Some(subject_row) = subject_rows.next()? {
            let id = subject_row.get(0)?;
            let name = subject_row.get(1)?;
            let modified_datetime = subject_row.get(2)?;

            subjects_vector.push(Subject { id, name, modified_datetime });
        }

        Ok(subjects_vector)
    }

    /// Retrieves all of the subjects (mainly from the database). 
    pub fn read_all_subjects (&self, sort: Option<&str>) -> Result<Vec<Subject>, Box<dyn std_error::Error>> {
        let mut select_str = String::from("SELECT id, name, datetime_modified FROM subjects");

        if let Some(sort_option) = sort {
            let sql_string = format!(" ORDER by {}", sort_option);
            select_str.push_str(&sql_string);
        }

        let mut select_all_subjects_stmt = self.db.prepare(&select_str)?;

        let mut subject_rows = select_all_subjects_stmt.query(rusqlite::NO_PARAMS)?;
        let mut subjects: Vec<Subject> = vec![];

        while let Some(subject_row) = subject_rows.next()? {
            let id: i64 = subject_row.get(0)?;
            let name: String = subject_row.get(1)?;
            let modified_datetime: chrono::DateTime<Local> = subject_row.get(2)?;

            subjects.push(Subject { id, name, modified_datetime });
        }

        Ok(subjects)
    }

    /// Deletes the subjects in the database and the filesystem. 
    /// Also returns the deleted subjects. 
    pub fn delete_subjects (&self, subjects: &Vec<&str>, delete: bool) -> Result<Vec<Subject>, Box<dyn std_error::Error>> {
        let subjects_vector: Vec<Subject> = self.read_subjects(&subjects, None)?;
        let input_names = subjects.clone().into_iter().map(| subject | Value::from(subject.to_string())).collect();
        let pointer = Rc::new(input_names);

        let mut subject_delete_stmt = self.db.prepare_cached("DELETE FROM subjects WHERE name IN rarray(?)")?;
        subject_delete_stmt.execute(&[&pointer])?;

        if delete {
            for subject in &subjects_vector {
                let path = subject.path(self);
                fs::remove_dir_all(&path);
            }
        }

        Ok(subjects_vector)
    }

    /// Deletes subjects (given with the ID) in the database and filesystem. 
    /// Also returns the data of the deleted subjects. 
    pub fn delete_subjects_by_id (&self, subjects: &Vec<i64>, delete: bool) -> Result<Vec<Subject>, Box<dyn std_error::Error>> {
        let subjects_vector: Vec<Subject> = self.read_subjects_by_id(&subjects, None)?;
        let input_names = subjects.clone().into_iter().map(| subject | Value::from(subject.to_string())).collect();
        let pointer = Rc::new(input_names);

        let mut subject_delete_stmt = self.db.prepare_cached("DELETE FROM subjects WHERE name IN rarray(?)")?;
        subject_delete_stmt.execute(&[&pointer])?;

        if delete {
            for subject in &subjects_vector {
                let path = subject.path(self);
                fs::remove_dir_all(&path);
            }
        }

        Ok(subjects_vector)
    }

    /// Deletes all of the subjects in the database and filesystem. 
    /// Returns the data of the deleted subjects.
    pub fn delete_all_subjects (&mut self, delete: bool) -> Result<Vec<Subject>, Box<dyn std_error::Error>> {
        let all_subjects = self.read_all_subjects(None)?;
        let mut delete_all_subjects_stmt = self.db.execute("DELETE FROM subjects", rusqlite::NO_PARAMS)?;
        
        if delete {
            for subject in all_subjects.iter() {
                let path = subject.path(self);

                fs::remove_dir_all(&path);
            }
        }

        Ok(all_subjects)
    }

    /// Updates the subjects in the database and the filesystem. 
    /// Returns the updated data of the subjects. 
    pub fn update_subjects (&self, subjects: &HashMap<&str, &str>) -> Result<Vec<Subject>, Box<dyn std_error::Error>> {
        let mut old_subjects: Vec<&str> = vec![];
        for key in subjects.keys() {
            old_subjects.push(&key);
        }

        let mut valid_subjects = self.read_subjects(&old_subjects, None)?;

        let mut update_subjects_stmt = self.db.prepare("UPDATE subjects SET name = :new_name, slug = :new_slug, datetime_modified = :new_date WHERE slug = :slug AND name = :name")?;
        
        let now: chrono::DateTime<Local> = chrono::Local::now();
        let now_as_iso_string = now.to_rfc3339();

        let mut updated_subjects: Vec<Subject> = vec![];
        
        for subject in valid_subjects.iter() {
            let new_subject = match subjects.get(subject.name.as_str()) {
                Some(s) => s.to_string(), 
                None => continue, 
            };
            let slug = helpers::kebab_case(&new_subject);
            let old_slug = helpers::kebab_case(&subject.name);
            
            let updated_row = match update_subjects_stmt.execute_named(&[(":new_name", &new_subject), (":new_slug", &slug), (":new_date", &now_as_iso_string), (":slug", &old_slug), (":name", &subject.name)]) {
                Ok(changed_row_size) => changed_row_size, 
                Err(_) => continue, 
            };

            if updated_row == 0 {
                continue;
            }

            let mut subject_instance = Subject { id: subject.id, name: subject.name.to_string(), modified_datetime: now };
            let mut subject_path: PathBuf = subject_instance.path(self);

            subject_instance.name = new_subject;
            let mut new_subject_path: PathBuf = subject_instance.path(self);
            match helpers::move_folder(&subject_path, &new_subject_path, Some(&now_as_iso_string)) {
                Ok(()) => (), 
                Err(_) => continue, 
            };

            updated_subjects.push(subject_instance);
        }

        Ok(updated_subjects)
    }

    pub fn create_notes<'a>(&self, subject: &'a str, notes: &Vec<&'a str>) -> Result<Vec<Note>, Box<dyn std_error::Error>> {
        let subjects: Vec<Subject> = self.read_subjects(&vec![subject], None)?;
        let subject: Subject = match subjects.len() {
            0 => return Err(Box::new(Error::ValueError)), 
            _ => subjects[0].clone(),  
        };

        let mut insert_stmt = self.db.prepare_cached("INSERT INTO notes (slug, title, subject_id, datetime_modified) VALUES (?, ?, ?, ?)")?;
        
        let mut now: chrono::DateTime<Local> = chrono::Local::now();
        let mut now_as_iso_string: String = now.to_rfc3339();

        let mut notes_object: Vec<Note> = vec![];
        let mut template_registry = handlebars::Handlebars::new();
        template_registry.register_template_string("tex_note", constants::NOTE_TEMPLATE)?;

        for note_title in notes.iter() {
            let subject_slug = helpers::kebab_case(note_title);

            let note_id = insert_stmt.insert(&[subject_slug.clone(), note_title.to_string(), subject.id.to_string(), now_as_iso_string.clone()])?;

            let note_instance = Note { 
                id: note_id, 
                subject_id: subject.id, 
                title: note_title.to_string(), 
                modified_datetime: now 
            };

            // creating the file
            let mut note_path = note_instance.path(self, &subject);
            let mut note_file = fs::OpenOptions::new().create_new(true).write(true).open(note_path)?;
            let rendered_string = template_registry.render("tex_note", &json!({ "author": "Me", "date": now_as_iso_string, "title": note_instance.title }))?;
            note_file.write(rendered_string.as_bytes())?;

            notes_object.push( note_instance )
        }

        Ok(notes_object)
    }

    pub fn read_notes<'a>(&mut self, subject: &'a str, notes: &Vec<&'a str>, sort: Option<&'a str>) -> Result<Vec<Note>, Box<dyn std_error::Error>> {
        let subjects: Vec<Subject> = self.read_subjects(&vec![subject], None)?;
        let subject: &Subject = match subjects.len() {
            0 => return Ok(vec![]), 
            _ => &subjects[0], 
        };
        
        let mut subject_select_sql = match sort {
            Some(p) => format!("SELECT id, subject_id, title, datetime_modified FROM notes WHERE title IN rarray(?) ORDER BY {}", p), 
            None => String::from("SELECT id, subject_id, title, datetime_modified FROM notes WHERE title IN rarray(?)")
        };
        let mut notes_select_stmt = self.db.prepare_cached(&subject_select_sql)?;
    
        let notes_value_list = notes.clone().into_iter().map(| title | Value::from(title.to_string()) ).collect();
        let notes_pointer = Rc::new(notes_value_list);

        let mut valid_notes = notes_select_stmt.query(&[&notes_pointer])?;
        let mut notes_vector: Vec<Note> = vec![];

        while let Some(note_row) = valid_notes.next()? {
            let id: i64 = note_row.get(0)?;
            let subject_id: i64 = note_row.get(1)?;
            let title: String = note_row.get(2)?;
            let modified_datetime: chrono::DateTime<Local> = note_row.get(3)?;

            notes_vector.push(Note { id, subject_id, title, modified_datetime });            
        }

        Ok(notes_vector)
    }

    pub fn read_notes_by_id<'a>(&mut self, note_id_list: &Vec<i64>, sort: Option<&'a str>) -> Result<Vec<Note>, Box<dyn std_error::Error>> {
        let mut subject_select_sql = match sort {
            Some(p) => format!("SELECT id, subject_id, title, datetime_modified FROM notes WHERE id IN rarray(?) ORDER BY {}", p), 
            None => String::from("SELECT id, subject_id, title, datetime_modified FROM notes WHERE id IN rarray(?)")
        };
        let mut notes_select_stmt = self.db.prepare_cached(&subject_select_sql)?;
        let notes_value_list = note_id_list.clone().into_iter().map(| title | Value::from(title.to_string()) ).collect();
        let notes_pointer = Rc::new(notes_value_list);

        let mut note_rows = notes_select_stmt.query(&[&notes_pointer])?;

        let mut notes_vector: Vec<Note> = vec![];

        while let Some(note_row) = note_rows.next()? {
            let id: i64 = note_row.get(0)?;
            let subject_id: i64 = note_row.get(1)?;
            let title: String = note_row.get(2)?;
            let modified_datetime: chrono::DateTime<Local> = note_row.get(3)?;

            notes_vector.push(Note { id, subject_id, title, modified_datetime });
        }

        Ok(notes_vector)
    }

    pub fn read_all_notes (&mut self, sort: Option<&str>, limit: Option<u64>, reverse: Option<bool>) -> Result<Vec<Note>, Box<dyn std_error::Error>> {
        let mut select_all_notes_stmt = String::from("SELECT id, subject_id, title, datetime_modified FROM notes");
        
        if let Some(order) = sort {
            select_all_notes_stmt.push_str(" ORDER BY ");
            select_all_notes_stmt.push_str(order);
        }

        if let Some(desc) = reverse {
            if desc {
                select_all_notes_stmt.push_str(" DESC");
            }
        }

        if let Some(limit_count) = limit {
            select_all_notes_stmt.push_str(" LIMIT ");
            select_all_notes_stmt.push_str(&limit_count.to_string());
        }

        let mut select_sql_stmt = self.db.prepare(&select_all_notes_stmt)?;
        let mut notes_row = select_sql_stmt.query(rusqlite::NO_PARAMS)?;

        let mut notes: Vec<Note> = vec![];

        while let Some(note_row) = notes_row.next()? {
            let id = note_row.get(0)?;
            let subject_id = note_row.get(1)?;
            let title = note_row.get(2)?;
            let modified_datetime = note_row.get(3)?;

            notes.push(Note { id, subject_id, title, modified_datetime });
        }

        Ok(notes)
    }

    pub fn read_all_notes_by_subject<'a> (&mut self, subject: &str, sort: Option<&'a str>) -> Result<Vec<Note>, Box<dyn std_error::Error>> {
        let subjects: Vec<Subject> = self.read_subjects(&vec![subject], sort)?;
        let subject: &Subject = match subjects.len() {
            0 => return Ok(vec![]), 
            _ => &subjects[0], 
        };

        let mut subject_select_sql = match sort {
            Some(p) => format!("SELECT id, subject_id, title, datetime_modified FROM notes WHERE subject_id == ? ORDER BY {}", p), 
            None => String::from("SELECT id, subject_id, title, datetime_modified FROM notes WHERE subject_id == ?")
        };
        
        let mut select_all_notes_stmt = self.db.prepare_cached(&subject_select_sql)?;

        let mut notes_rows = select_all_notes_stmt.query(&[subject.id])?;
        let mut notes_vector: Vec<Note> = vec![];

        while let Some(note_row) = notes_rows.next()? {
            let id: i64 = note_row.get(0)?;
            let subject_id: i64 = note_row.get(1)?;
            let title: String = note_row.get(2)?;
            let modified_datetime: chrono::DateTime<Local> = note_row.get(3)?;

            notes_vector.push(Note { id, subject_id, title, modified_datetime });
        }

        Ok(notes_vector)
    }

    pub fn delete_notes<'a>(&mut self, subject: &'a str, notes: &Vec<&'a str>, delete: bool) -> Result<Vec<Note>, Box<dyn std_error::Error>> {
        let notes_vector: Vec<Note> = self.read_notes(subject, notes, None)?;
        let subjects: Vec<Subject> = self.read_subjects(&vec![subject], None)?;
        let subject: &Subject = match subjects.len() {
            0 => return Ok(vec![]), 
            _ => &subjects[0], 
        };

        let input = notes_vector.iter().map(| note | Value::from(note.id)).collect();
        let input_pointer = Rc::new(input);

        let mut delete_note_stmt = self.db.prepare("DELETE FROM notes WHERE id IN rarray(?)")?;
        delete_note_stmt.execute(&[&input_pointer])?;

        if delete {
            for note in notes_vector.iter() {
                let path: PathBuf = note.path(self, &subject);

                fs::remove_file(path);
            }
        }

        Ok(notes_vector)
    }

    pub fn delete_notes_by_id (&mut self, notes: &Vec<i64>, delete: bool) -> Result<Vec<Note>, Box<dyn std_error::Error>> {
        let notes_vector: Vec<Note> = self.read_notes_by_id(notes, None)?;
        let subject_ids: Vec<i64> = notes_vector.iter().map(| note_instance | note_instance.subject_id).collect();
        let subject_vector: Vec<Subject> = self.read_subjects_by_id(&subject_ids, None)?;

        let input = notes_vector.iter().map(| note | Value::from(note.id)).collect();
        let input_pointer = Rc::new(input);

        let mut delete_note_stmt = self.db.prepare("DELETE FROM notes WHERE id IN rarray(?)")?;
        delete_note_stmt.execute(&[&input_pointer])?;

        if delete {
            let mut notes_and_subject = notes_vector.iter().zip(subject_vector.iter());
            while let Some(note_tuple) = notes_and_subject.next() {
                let note: &Note = note_tuple.0;
                let subject: &Subject = note_tuple.1;
                let path: PathBuf = note.path(self, &subject);

                fs::remove_file(path);
            }
        }

        Ok(notes_vector)
    }

    pub fn delete_all_notes_by_subject (&mut self, subject: &str, delete: bool) -> Result<Vec<Note>, Box<dyn std_error::Error>> {
        let subjects = self.read_subjects(&vec![subject], None)?;
        let subject: &Subject = match subjects.len() {
            0 => return Ok(vec![]), 
            _ => &subjects[0], 
        };

        let all_subject_notes = self.read_all_notes_by_subject(&subject.name, None)?;

        let delete_all_notes_stmt = self.db.execute("DELETE FROM notes WHERE subject_id == ?", &[subject.id])?;

        if delete {
            for note in all_subject_notes.iter() {
                let path = note.path(self, &subject);
    
                fs::remove_file(&path);
            }
        }

        Ok(all_subject_notes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Improve test case
    #[test]
    fn basic_note_initialization() -> Result<(), Box<dyn std_error::Error>> {
        let note_path = PathBuf::from("./tests/notes");
        fs::remove_dir_all(&note_path)?;
        let mut test_case: Notes = Notes::new(&note_path)?;

        let test_input = vec!["Calculus", "Algebra"];
        let test_note_input = vec!["Precalculus Quick Review", "Introduction to Integrations"];

        // creating a subject
        let created_subjects: Vec<Subject> = test_case.create_subjects(&test_input)?;
        assert_eq!(created_subjects.len(), 2);

        let created_notes: Vec<Note> = test_case.create_notes(test_input[0], &test_note_input)?;
        assert_eq!(created_notes.len(), 2);

        // reading the subjects 
        let available_subjects: Vec<Subject> = test_case.read_subjects(&test_input, None)?;
        assert_eq!(available_subjects.len(), 2);

        // reading all of the subjects of the note 
        let all_available_subjects: Vec<Subject> = test_case.read_all_subjects(Some("name"))?;
        assert_eq!(all_available_subjects.len(), 2);

        // reading the notes
        let available_notes: Vec<Note> = test_case.read_notes(test_input[0], &test_note_input, None)?;
        assert_eq!(available_notes.len(), 2);

        let available_notes_from_id: Vec<Note> = test_case.read_notes_by_id(&vec![1, 2], None)?;
        assert_eq!(available_notes_from_id.len(), 2);

        let all_available_notes: Vec<Note> = test_case.read_all_notes_by_subject(test_input[0], None)?;
        assert_eq!(all_available_notes.len(), 2);

        // deleting the notes
        // let deleted_notes: Vec<Note> = test_case.delete_notes(&test_input[0], &test_note_input, true)?;
        // assert_eq!(deleted_notes.len(), 2);

        // deleting the subjects 
        // let deleted_subjects: Vec<Subject> = test_case.delete_subjects(&test_input, true)?;
        // assert_eq!(deleted_subjects.len(), 2);

        Ok(())
    }

    #[test]
    #[should_panic]
    fn invalid_note_location() {
        let note_path = PathBuf::from("./test/invalid/location/is/invalid");
        let mut test_case: Notes = Notes::new(&note_path).unwrap();

        ()
    }
}
