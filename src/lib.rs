use std::collections::HashMap;
use std::env;
use std::fs::{ self, DirBuilder, File };
use std::io::{ BufReader, Read };
use std::io::Write;
use std::path::{ Path, PathBuf };
use std::process;
use std::result;
use std::thread;
use std::sync;

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

pub type Result<T> = result::Result<T, Error>;

// profile constants 
const PROFILE_METADATA_FILENAME: &str = "profile.json";
const PROFILE_COMMON_FILES_DIR_NAME: &str = "common";
const PROFILE_SHELF_FOLDER: &str = "notes";
const PROFILE_TEMPLATE_FILES_DIR_NAME: &str = "templates";

const PROFILE_NOTE_TEMPLATE_NAME: &str = "note";
const PROFILE_MASTER_NOTE_TEMPLATE_NAME: &str = "master";

#[derive(Serialize, Deserialize, Debug, Clone)]
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

pub struct CompilationEnvironment {
    subject: notes::Subject, 
    notes: Vec<notes::Note>, 
    command: Vec<String>, 
}

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

impl Profile {
    /// Initializes the data to set up a new profile. 
    /// 
    /// It also immediately creates the folder structure in the filesystem. 
   pub fn new<P: AsRef<Path>> (
       path: P, 
       name: Option<String>, 
       use_db: bool
    ) -> Result<Profile> {
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
    pub fn open<P: AsRef<Path>> (path: P) -> Result<Profile> {
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
    pub fn set_templates (&mut self) -> Result<()> {
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

    /// Returns a string from a Handlebars template. 
    pub fn return_string_from_note_template (
        &self, 
        subject: &notes::Subject, 
        note: &notes::Note
    ) -> Result<String> {
        let mut metadata = serde_json::to_value(self.metadata.clone()).map_err(Error::SerdeValueError)?;

        Ok(String::from("d"))
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

    pub fn add_entries(&mut self, subjects: &Vec<&str>, notes: &Vec<Vec<&str>>, force: bool) -> Result<()> {
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

    pub fn remove_entries(&mut self, notes_id: &Vec<i64>, subjects_id: &Vec<i64>, subjects: &Vec<&str>, notes: &Vec<Vec<&str>>, delete: bool) -> Result<()> {
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

    pub fn list_entries(&mut self, sort: Option<&str>) -> Result<()> {
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

    /// Compiles notes. 
    /// This uses the concurrency features of Rust. 
    pub fn compile_notes_in_parallel (
        &self, 
        subject: &notes::Subject, 
        note_list: &Vec<notes::Note>, 
        thread_count: i16
    ) -> Result<Vec<notes::Note>> {        
        let command = self.compile_note_command();
        
        let mut note_list_in_reverse = note_list.clone();
        note_list_in_reverse.reverse();
        
        let compilation_environment = CompilationEnvironment {
            subject: subject.clone(), 
            notes: note_list_in_reverse, 
            command, 
        };

        let original_dir = env::current_dir().map_err(Error::IoError)?;
        env::set_current_dir(subject.path_in_shelf(&self.notes)).map_err(Error::IoError)?;
        
        // this will serve as a task queue for the threads to be spawned
        let compilation_environment = sync::Arc::new(sync::Mutex::new(compilation_environment));
        let compiled_notes = sync::Arc::new(sync::Mutex::new(vec![]));
        let mut threads = vec![];

        for i in 0..thread_count {
            let compilation_environment_mutex = sync::Arc::clone(&compilation_environment);
            let compiled_notes_mutex = sync::Arc::clone(&compiled_notes);
            let thread = thread::spawn(move || {
                let mut compilation_environment = compilation_environment_mutex.lock().unwrap();
                let mut compiled_notes = compiled_notes_mutex.lock().unwrap();

                while let Some(note) = compilation_environment.notes.pop() {
                    let mut command_vector = compilation_environment.command.clone(); 
                    command_vector.push(note.file_name());

                    let mut command_process = process::Command::new(command_vector.remove(0));
                    for arg in command_vector.into_iter() {
                        command_process.arg(arg);
                    }

                    let command_output = match command_process.output().map_err(Error::IoError) {
                        Ok(v) => v, 
                        Err(_e) => continue, 
                    };

                    if !command_output.status.success() {
                        continue;
                    }

                    compiled_notes.push(note);
                }
            });

            threads.push(thread);
        }

        // waiting for all threads to be done 
        for thread in threads {
            thread.join().unwrap();
        }

        env::set_current_dir(original_dir).map_err(Error::IoError)?;

        // at this point, the Arc should have no owners, allowing it to be moved out 
        match sync::Arc::try_unwrap(compiled_notes) {
            Ok(v) => Ok(v.into_inner().unwrap()), 
            Err(_e) => Err(Error::ValueError),  
        }
    }

    /// Compile a note of a subject. 
    /// 
    /// It uses the `command` member in the profile metadata. 
    /// This funciton merely depends on appending the note in the resulting command. 
    /// 
    /// If it's empty, the default command is `latexmk $NOTEFILE`. 
    pub fn compile_note (
        &self, 
        subject: &notes::Subject, 
        note: &notes::Note, 
    ) -> Result<process::Output> {
        let mut command_and_args: Vec<String> = self.compile_note_command();

        // change the current working directory to the note path        
        let original_dir = env::current_dir().map_err(Error::IoError)?;
        env::set_current_dir(subject.path_in_shelf(&self.notes)).map_err(Error::IoError)?;
        let mut relative_file_path_to_current_dir = env::current_dir().map_err(Error::IoError)?;
        relative_file_path_to_current_dir.push(note.file_name());
        command_and_args.push(relative_file_path_to_current_dir.to_str().unwrap().to_string());
        
        let mut command_process = process::Command::new(command_and_args.remove(0));
        for arg in command_and_args.into_iter() {
            command_process.arg(arg);
        }

        let command = command_process.output().map_err(Error::IoError); 

        env::set_current_dir(original_dir).map_err(Error::IoError)?;

        command
    }

    /// Compile the master note of the subject. 
    /// 
    /// The master note is basically the combination of the notes (the top=level LaTeX documents) of the subject. 
    pub fn compile_master_note (&self, subject: &notes::Subject) {}

    pub fn open_entry(&mut self, id: i64, execute: String) -> Result<()> {
        Ok(())
    }

    /// As self-explanatory as the function name, it creates a symlink pointing to the common files folder. 
    /// 
    /// This is mostly used for the compilation process or if you want to statically create a symlink for your notes. 
    pub fn create_symlink_from_common_files_folder<P: AsRef<Path>>(&self, dst: P) -> Result<()> {
        helpers::filesystem::create_symlink(self.common_files_path(), dst.as_ref())
    }

    /// Returns the command for compiling the notes. 
    /// By default, the compilation command is `latexmk -pdf`. 
    /// 
    /// If there's no valid value found from the key (i.e., invalid type), it will return the default command. 
    pub fn compile_note_command (&self) -> Vec<String> {
        let PROFILE_DEFAULT_COMMAND = vec![ String::from("latexmk"), String::from("-pdf") ];
        match self.metadata.extra.get("command").as_ref() {
            Some(value) => match value.is_array() { 
                true => {
                    let json_array = value.as_array().unwrap();
                    let mut vector: Vec<String> = vec![];
                    
                    for command_arg in json_array.iter() {
                        match command_arg {
                            serde_json::Value::String (string) => vector.push(string.clone()), 
                            _ => continue, 
                        }
                    }
    
                    vector
                }, 
                false => PROFILE_DEFAULT_COMMAND, 
            }, 
            None => PROFILE_DEFAULT_COMMAND, 
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_init_test() -> Result<()> {
        let test_folder_name = format!("./tests/{}", consts::TEXTURE_NOTES_DIR_NAME);
        let test_path = PathBuf::from(&test_folder_name);
        fs::remove_dir_all(&test_path);
        let mut test_profile: Profile = Profile::new(&test_path, None, true)?;

        let test_subjects = notes::Subject::from_vec_loose(&vec!["Calculus", "Algebra", "Physics"], &test_profile.notes)?;
        let test_notes = notes::Note::from_vec_loose(
            &vec!["Introduction to Precalculus", "Introduction to Integrations", "Taylor Series", "Introduction to Limits"], 
            &test_subjects[0], &test_profile.notes)?;
        let test_input = r"\documentclass[class=memoir, crop=false, oneside, 14pt]{standalone}

        % document metadata
        \author{ {{~author~}} }
        \title{ {{~title~}} }
        \date{ {{~date~}} }
        
        \begin{document}
        This is a content sample.
        \end{document}
        ";

        test_profile.notes.create_subjects(&test_subjects, true, false)?;
        test_profile.notes.create_notes(&test_subjects[0], &test_notes, test_input, true, false)?;
        test_profile.compile_notes_in_parallel(&test_subjects[0], &test_notes, 4)?;

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