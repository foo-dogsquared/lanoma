use std::collections::HashMap;
use std::fs::{ self, DirBuilder, File };
use std::path::{ Path, PathBuf };
use std::io::{ BufReader, Read };
use std::io::Write;

use handlebars;
use serde::{ Serialize, Deserialize };
use serde_json::{ Value };

#[macro_use] extern crate lazy_static;
#[macro_use] extern crate serde_json;

mod consts;
pub mod error;
mod helpers;
pub mod shelf;
pub mod notes;

use shelf::Shelf;
use error::Error;

const PROFILE_METADATA_FILENAME: &str = "profile.json";
const PROFILE_COMMON_FILES_DIR_NAME: &str = "common";
const PROFILE_SHELF_FOLDER: &str = "notes";
const PROFILE_TEMPLATE_FILES_DIR_NAME: &str = "templates";

const PROFILE_NOTE_TEMPLATE_NAME: &str = "note";
const PROFILE_MASTER_NOTE_TEMPLATE_NAME: &str = "master";

/// The main use of a profile is provide an easy interface for the program. 
/// Each profile have its own set of notes, allowing you to have multiple profile representing 
/// different students. 
/// Also take note that a profile takes its own folder (currently named `texture-notes-profile`).   
pub struct Profile {
    path: PathBuf, 
    notes: Shelf, 
    metadata: ProfileMetadata, 
    templates: handlebars::Handlebars, 
}

#[derive(Serialize, Deserialize, Debug)]
struct ProfileMetadata {
    name: String, 
    version: String, 

    #[serde(flatten)]
    extra: HashMap<String, Value>, 
}

impl ProfileMetadata {
    /// Create a new profile metadata instance. 
    pub fn new (
        name: Option<String>, 
        extra: Option<HashMap<String, Value>>
    ) -> Self {
        ProfileMetadata {
            name: match name {
                Some(v) => v, 
                None => String::from("New Student"), 
            }, 
            version: String::from(consts::TEXTURE_NOTES_VERSION), 
            extra: match extra {
                Some(v) => v, 
                None => HashMap::<String, Value>::new(), 
            }
        }
    }
}

impl Profile {
    /// Initializes the data to set up a new profile. 
    /// 
    /// It also immediately creates the folder structure in the filesystem. 
   pub fn new<P: AsRef<Path>> (path: P, name: Option<String>, use_db: bool) -> Result<Profile, Error> {
        let mut path = path.as_ref().to_path_buf();

        if !path.ends_with(consts::TEXTURE_NOTES_DIR_NAME) {
            path.push(consts::TEXTURE_NOTES_DIR_NAME); 
        }

        // create the folder of the profile
        let path_builder = DirBuilder::new();
        helpers::filesystem::create_folder(&path_builder, &path)?;

        // create the shelf folder
        let mut note_folder = path.clone();
        note_folder.push(PROFILE_SHELF_FOLDER);
        let notes: Shelf = Shelf::new(note_folder, use_db)?;

        // create the profile instance
        let profile_metadata: ProfileMetadata = ProfileMetadata::new(name, None);
        let templates = handlebars::Handlebars::new();
        let mut profile = Profile { path, notes, metadata: profile_metadata, templates };

        // create the common files folder
        helpers::filesystem::create_folder(&path_builder, &profile.common_files_path())?;

        // create the templates folder 
        helpers::filesystem::create_folder(&path_builder, &profile.templates_path())?;

        // setup the default settings for the templates 
        profile.set_templates()?;

        // create the metadata file
        let mut profile_metadata_file_buffer = File::create(profile.metadata_path()).map_err(Error::IoError)?;
        let profile_metadata_string: String = serde_json::to_string_pretty(&profile.metadata).unwrap();
        profile_metadata_file_buffer.write_all(&profile_metadata_string.into_bytes()).map_err(Error::IoError)?;

        Ok(profile)
    }

    /// Opens an initiated profile. 
    /// 
    /// If the profile does not exist in the given path, it will cause an error. 
    pub fn open<P: AsRef<Path>> (path: P) -> Result<Profile, Error> {
        let mut path: PathBuf = path.as_ref().to_path_buf();

        // if the path is not found or does not have the profile metadata
        // then it will result in an error
        if !path.ends_with(consts::TEXTURE_NOTES_DIR_NAME) {
            path.push(consts::TEXTURE_NOTES_DIR_NAME);
        }

        let mut metadata_path: PathBuf = path.clone();
        metadata_path.push(PROFILE_METADATA_FILENAME);

        // getting the metadata from the metadata file
        let metadata_file: File = File::open(metadata_path).map_err(Error::IoError)?;
        let mut metadata_file_string: String = String::new();
        let mut metadata_file_buffer = BufReader::new(metadata_file);
        metadata_file_buffer.read_to_string(&mut metadata_file_string).map_err(Error::IoError)?;
        
        let metadata: ProfileMetadata = serde_json::from_str(&metadata_file_string).map_err(Error::SerdeValueError)?;
        let notes: Shelf = Shelf::new(path.clone(), true)?;
        let templates = handlebars::Handlebars::new();

        let mut profile = Profile { path, notes, metadata, templates };

        if profile.is_valid() {
            return Err(Error::InvalidProfileError(profile.path.clone()));
        }

        profile.set_templates()?;

        Ok(profile)
    }

    /// Insert the contents of the files inside of the templates directory of the profile in the Handlebars registry. 
    /// 
    /// Take note it will only get the contents of the top-level files in the templates folder. 
    pub fn set_templates (&mut self) -> Result<(), Error> {
        if !self.has_templates() {
            return Err(Error::InvalidProfileError(self.path.clone()));
        }

        // creating a new Handlebars registry
        let mut handlebars_registry = handlebars::Handlebars::new();
        handlebars_registry.register_escape_fn(handlebars::no_escape);

        // iterating through the entries in the templates folder
        for entry in fs::read_dir(self.templates_path()).map_err(Error::IoError)? {
            let entry = entry.map_err(Error::IoError)?;

            let path = entry.path();
            let metadata = entry.metadata().map_err(Error::IoError)?;

            if !metadata.is_file() {
                continue;
            }

            // checking if the file has an extension of "tex"
            match path.extension() {
                Some(v) => {
                    if v != "tex" {
                        continue;
                    }
                }, 
                None => continue, 
            }

            let file_name = match path.file_name() {
                Some(os_name) => match os_name.to_str() {
                    Some(v) => v, 
                    None => continue, 
                }, 
                None => continue, 
            };

            handlebars_registry.register_template_file(file_name, &path).map_err(Error::HandlebarsTemplateFileError)?;
        }

        // TODO: checks if there are custom template for the note and master note templates
        if !handlebars_registry.has_template(PROFILE_NOTE_TEMPLATE_NAME) {
            handlebars_registry.register_template_string(PROFILE_NOTE_TEMPLATE_NAME, consts::NOTE_TEMPLATE).map_err(Error::HandlebarsTemplateError)?;
        }

        if !handlebars_registry.has_template(PROFILE_MASTER_NOTE_TEMPLATE_NAME) {
            handlebars_registry.register_template_string(PROFILE_MASTER_NOTE_TEMPLATE_NAME, consts::MASTER_NOTE_TEMPLATE).map_err(Error::HandlebarsTemplateError)?;
        }

        self.templates = handlebars_registry;
        
        Ok(())
    }

    /// Returns the relative common files path of the profile. 
    pub fn common_files_path (&self) -> PathBuf {
        let mut path = self.path.clone();
        path.push(PROFILE_COMMON_FILES_DIR_NAME);

        path
    }

    /// Checks if the profile has the common files path in the filesystem. 
    pub fn has_common_files (&self) -> bool {
        self.common_files_path().exists()
    }

    /// Returns the metadata file path of the profile. 
    pub fn metadata_path (&self) -> PathBuf {
        let mut path = self.path.clone();
        path.push(PROFILE_METADATA_FILENAME);

        path
    }

    /// Checks if the metadata is in the filesystem. 
    pub fn has_metadata (&self) -> bool {
        self.metadata_path().exists()
    }

    /// Returns the shelf path of the profile. 
    pub fn shelf_path (&self) -> PathBuf {
        self.notes.path()
    }

    /// Checks if the shelf is in the filesystem. 
    pub fn has_shelf (&self) -> bool {
        self.shelf_path().exists()
    }

    /// Returns the template path of the profile. 
    pub fn templates_path (&self) -> PathBuf {
        let mut path = self.path.clone();
        path.push(PROFILE_TEMPLATE_FILES_DIR_NAME);

        path
    }

    /// Checks if the templates is in the filesystem. 
    pub fn has_templates (&self) -> bool {
        self.templates_path().exists()
    }

    /// Checks if the profile has been exported in the filesystem. 
    pub fn is_exported (&self) -> bool {
        self.path.exists()
    }

    /// Checks if the profile has a valid folder structure. 
    pub fn is_valid (&self) -> bool {
        self.is_exported() && self.has_templates() && self.has_common_files() && self.has_metadata() && self.has_shelf()
    }

    pub fn add_entries(&mut self, subjects: &Vec<&str>, notes: &Vec<Vec<&str>>, force: bool) -> Result<(), Error> {
        // creating the subjects in the profile
        // self.notes.create_subjects(&subjects)?;
        
        // // creating the notes in the profile
        // for note_tuple in notes.iter() {
        //     if let Some((subject, note_list)) = note_tuple.split_first() {
        //         self.notes.create_notes(subject, &note_list.to_vec())?;
        //     }
        // }

        Ok(())
    }

    pub fn remove_entries(&mut self, notes_id: &Vec<i64>, subjects_id: &Vec<i64>, subjects: &Vec<&str>, notes: &Vec<Vec<&str>>, delete: bool) -> Result<(), Error> {
        // removing the subjects
        // self.notes.delete_subjects(&subjects, delete)?;
        // self.notes.delete_subjects_by_id(&subjects_id, delete)?;
        // self.notes.delete_notes_by_id(&notes_id, delete)?;

        // for note_tuple in notes.iter() {
        //     if let Some((subject, note_list)) = note_tuple.split_first() {
        //         self.notes.delete_notes(subject, &note_list.to_vec(), delete)?;
        //     }
        // }

        Ok(())
    }

    pub fn list_entries(&mut self, sort: Option<&str>) -> Result<(), Error> {
        let mut all_subjects: Vec<notes::Subject> = self.notes.get_all_subjects_from_db(sort)?;
        
        for subject in all_subjects.iter() {
            let mut available_notes: Vec<notes::Note> = self.notes.get_all_notes_by_subject_from_db(&subject, None)?;
            
            println!("Subject '{}' has {} notes.", subject.name(), available_notes.len());

            for note in available_notes.iter() {
                let note_id = self.notes.get_note_id(&subject, &note)?;
                println!("  - ({}) {}", note_id.unwrap(), note.title());
            }
        }

        Ok(())
    }

    pub fn compile_entries(&mut self, id_list: Vec<i64>, notes: Vec<String>, main: Vec<String>, cache: bool) {}

    pub fn open_entry(&mut self, id: i64, execute: String) -> Result<(), Error> {
        Ok(())
    }

    pub fn create_symlink<P: AsRef<Path>, Q: AsRef<Path>> (&self, from: P, to: Q) -> Result<(), Error> {
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_init_test() -> Result<(), Error> {
        let test_folder_name = format!("./tests/{}", consts::TEXTURE_NOTES_DIR_NAME);
        let test_path = PathBuf::from(&test_folder_name);
        fs::remove_dir_all(&test_path);
        let mut test_profile: Profile = Profile::new(&test_path, None, true)?;

        let test_subjects = vec!["Calculus", "Algebra", "Physics"];
        let test_notes = vec![
            vec!["Calculus", "Introduction to Precalculus", "Introduction to Integrations"], 
            vec!["Algebra", "Introduction to Functions"], 
        ];
        
        test_profile.add_entries(&test_subjects, &test_notes, false)?;
        test_profile.remove_entries(&vec![], &vec![], &test_subjects, &test_notes, true)?;

        Ok(())
    }

    #[test]
    #[should_panic]
    fn invalid_profile_init_test() {
        let test_path = PathBuf::from("./this/path/does/not/exists/");
        let test_profile_result = Profile::new(&test_path, None, true);

        match test_profile_result {
            Err(error) => panic!("WHAT"), 
            _ => (), 
        }
    }
}