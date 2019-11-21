use std::collections::HashMap;
use std::error::Error;
use std::fs::{ self, DirBuilder, File };
use std::path::PathBuf;
use std::io::{ BufReader, Read };
use std::io::Write;

use serde::{ Serialize, Deserialize };
use serde_json::{ Value };

#[macro_use] extern crate lazy_static;

pub mod constants;
pub mod types;
pub mod error;
pub mod helpers;
pub mod notes;

use notes::Notes;

// TODO: Change the type of the notes to db::Notes later, this is for testing purposes
pub struct Profile {
    path: PathBuf, 
    notes: Notes, 
    metadata: ProfileMetadata, 
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProfileMetadata {
    name: String, 
    version: String, 

    #[serde(flatten)]
    extra: HashMap<String, Value>, 
}

impl Profile {
    /// Initializes the data to set up a new profile. 
    /// 
    /// The main use of a profile is provide an easy interface for the program. 
    /// Each profile have its own set of notes, allowing you to have multiple profile representing 
    /// different students. 
    /// Also take note that a profile takes its own folder (currently named `texture-notes-profile`). 
    /// 
    /// # Common failure cases
    /// 
    /// * When setting up the notes database unknowingly fails. 
    /// * When the directory doesn't exist. 
    /// * When there has been unexpected problems while filesystem operations are made. 
    pub fn new(path: &PathBuf) -> Result<Profile, Box<dyn Error>> {
        let mut path = path.clone();

        if !path.ends_with(constants::TEXTURE_NOTES_DIR_NAME) {
            path.push(constants::TEXTURE_NOTES_DIR_NAME); 
        }

        let path_builder = DirBuilder::new();
        helpers::create_folder(&path_builder, &path)?;

        let mut styles_path: PathBuf = path.clone();
        styles_path.push(constants::TEXTURE_NOTES_STYLES_DIR_NAME);

        let mut profile_metadata_path: PathBuf = path.clone();
        profile_metadata_path.push(constants::TEXTURE_NOTES_METADATA_FILENAME);
        
        let mut profile_metadata_file_buffer = File::create(profile_metadata_path.into_os_string()).map_err(error::Error::IoError)?;
        let extra: HashMap<String, Value> = HashMap::new();
        let profile_metadata: ProfileMetadata = ProfileMetadata { name: String::from("New Student"), version: String::from(constants::TEXTURE_NOTES_VERSION), extra };
        let profile_metadata_string: String = serde_json::to_string(&profile_metadata).unwrap();

        profile_metadata_file_buffer.write_all(&profile_metadata_string.into_bytes())?;

        helpers::create_folder(&path_builder, &styles_path)?;

        let mut notes: Notes = Notes::new(&path)?;

        Ok(Profile { path, notes, metadata: profile_metadata } )
    }

    /// Opens an initiated profile. 
    /// 
    /// If the profile does not exist in the given path, it will cause an error. 
    pub fn open(path: &PathBuf) -> Result<Profile, Box<dyn Error>> {
        // if the path is not found or does not have the profile metadata
        // then it will result in an error
        let mut path = path.clone();

        if !path.ends_with(constants::TEXTURE_NOTES_DIR_NAME) {
            path.push(constants::TEXTURE_NOTES_DIR_NAME);
        }

        let mut metadata_path: PathBuf = path.clone();
        metadata_path.push(constants::TEXTURE_NOTES_METADATA_FILENAME);

        let metadata_file: File = match File::open(metadata_path.into_os_string()) {
            Ok(file) => file, 
            Err(reason) => return Err(Box::new(error::Error::IoError(reason))), 
        };
        let mut metadata_file_string: String = String::new();
        let mut metadata_file_buffer = BufReader::new(metadata_file);

        match metadata_file_buffer.read_to_string(&mut metadata_file_string) {
            Ok(value) => value, 
            Err(reason) => return Err(Box::new(error::Error::IoError(reason))), 
        };
        
        let metadata: ProfileMetadata = match serde_json::from_str(&metadata_file_string) {
            Ok(data) => data, 
            Err(_error) => return Err(Box::new(error::Error::ProfileInvalidityError)), 
        };

         let notes: Notes = Notes::new(&path)?;

        Ok(Profile { path: path, notes: notes, metadata: metadata })
    }

    pub fn add_entries(&mut self, subjects: &Vec<&str>, notes: &Vec<Vec<&str>>, force: bool) -> Result<(), Box<dyn Error>> {
        // creating the subjects in the profile
        self.notes.create_subjects(&subjects)?;
        
        // creating the notes in the profile
        for note_tuple in notes.iter() {
            if let Some((subject, note_list)) = note_tuple.split_first() {
                self.notes.create_notes(subject, &note_list.to_vec())?;
            }
        }

        Ok(())
    }

    pub fn remove_entries(&mut self, notes_id: &Vec<i64>, subjects_id: &Vec<i64>, subjects: &Vec<&str>, notes: &Vec<Vec<&str>>, delete: bool) -> Result<(), Box<dyn Error>> {
        // removing the subjects
        self.notes.delete_subjects(&subjects, delete)?;
        self.notes.delete_subjects_by_id(&subjects_id, delete)?;
        self.notes.delete_notes_by_id(&notes_id, delete)?;

        for note_tuple in notes.iter() {
            if let Some((subject, note_list)) = note_tuple.split_first() {
                self.notes.delete_notes(subject, &note_list.to_vec(), delete)?;
            }
        }

        Ok(())
    }

    pub fn list_entries(&mut self, sort: String) -> Result<(), Box<dyn Error>> {

        Ok(())
    }

    pub fn compile_entries(&mut self, id_list: Vec<i64>, notes: Vec<String>, main: Vec<String>, cache: bool) {}

    pub fn open_entry(&mut self, id: i64, execute: String) {}

}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_init_test() -> Result<(), Box<dyn Error>> {
        let test_folder_name = format!("./tests/{}", constants::TEXTURE_NOTES_DIR_NAME);
        let test_path = PathBuf::from(&test_folder_name);
        fs::remove_dir_all(&test_path);
        let mut test_profile: Profile = Profile::new(&test_path)?;

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
        let test_profile_result: Result<Profile, Box<dyn Error>> = Profile::new(&test_path);

        match test_profile_result {
            Err(error) => panic!("WHAT"), 
            _ => (), 
        }
    }
}