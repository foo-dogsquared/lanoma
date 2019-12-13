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
pub struct ProfileMetadata {
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

/// A struct for handling the parameters for the compilation environment. 
/// 
/// This data structure is made for abstracting the compilation process making it as a separate component. 
#[derive(Clone, Debug)]
struct CompilationEnvironment {
    subject: notes::Subject, 
    notes: Vec<notes::Note>, 
    command: Vec<String>, 
    thread_count: i16
}

impl CompilationEnvironment {
    fn new(
        subject: notes::Subject, 
        notes: Vec<notes::Note>, 
        command: Vec<String>, 
        thread_count: i16, 
    ) -> Self {
        Self {
            subject, 
            notes, 
            command, 
            thread_count, 
        }
    }

    fn compile(&self, shelf: &Shelf) -> Result<Vec<notes::Note>> {
        let original_dir = env::current_dir().map_err(Error::IoError)?;

        let compilation_dst = self.subject.path_in_shelf(&shelf);
        env::set_current_dir(&compilation_dst).map_err(Error::IoError)?;
        
        // this will serve as a task queue for the threads to be spawned
        let compilation_environment = sync::Arc::new(sync::Mutex::new(self.clone()));
        let compiled_notes = sync::Arc::new(sync::Mutex::new(vec![]));
        let mut threads = vec![];

        for i in 0..self.thread_count {
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

                    if command_output.status.success() {
                        compiled_notes.push(note);
                    }
                }
            });

            threads.push(thread);
        }

        // waiting for all threads to be done 
        for thread in threads {
            thread.join().unwrap();
        }

        env::set_current_dir(original_dir).map_err(Error::IoError)?;

        match sync::Arc::try_unwrap(compiled_notes) {
            Ok(v) => Ok(v.into_inner().unwrap()), 
            Err(_e) => Err(Error::ValueError),  
        }
    }
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
   pub fn new<P: AsRef<Path>> (
       path: P, 
       name: Option<String>, 
       use_db: bool
    ) -> Result<Profile> {
        let mut path = path.as_ref().to_path_buf();

        if !path.ends_with(consts::TEXTURE_NOTES_DIR_NAME) {
            path.push(consts::TEXTURE_NOTES_DIR_NAME); 
        }

        // create the shelf instance
        let mut note_folder = path.clone();
        note_folder.push(PROFILE_SHELF_FOLDER);
        let notes: Shelf = Shelf::new(note_folder, use_db)?;

        // create the profile instance
        let metadata: ProfileMetadata = ProfileMetadata::new(name, None);
        let templates = handlebars::Handlebars::new();
        let profile = Profile { path, notes, metadata, templates };

        Ok(profile)
    }

    /// Opens an initiated profile. 
    /// 
    /// If the profile does not exist in the given path, it will cause an error. 
    pub fn from<P: AsRef<Path>> (path: P) -> Result<Profile> {
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

        if !profile.is_valid() {
            return Err(Error::InvalidProfileError(profile.path()));
        }

        profile.set_templates()?;

        Ok(profile)
    }

    /// Returns an immutable reference to the shelf. 
    pub fn shelf(&self) -> &Shelf {
        &self.notes
    }

    pub fn metadata(&self) -> &ProfileMetadata {
        &self.metadata
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
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
    /// 
    /// While creating the metadata from the profile, it will override certain fields 
    /// like `title` (the title of the note) and `subject` (the name of the subject). 
    pub fn return_string_from_note_template (
        &self, 
        subject: &notes::Subject, 
        note: &notes::Note, 
        template_name: Option<&str>, 
    ) -> Result<String> {
        let mut metadata = serde_json::to_value(self.metadata.clone()).map_err(Error::SerdeValueError)?;
        metadata["title"] = serde_json::json!(note.title());
        metadata["subject"] = serde_json::json!(subject.name());

        let template_name = match template_name {
            Some(v) => v, 
            None => PROFILE_NOTE_TEMPLATE_NAME, 
        };
        
        self.templates.render(template_name, &metadata).map_err(Error::HandlebarsRenderError)
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

    /// Get the metadata from the profile. 
    pub fn get_metadata(&self) -> Result<ProfileMetadata> {
        if !self.is_valid() {
            return Err(Error::InvalidProfileError(self.path.clone()));
        }

        let metadata_file_string = fs::read_to_string(self.metadata_path()).map_err(Error::IoError)?;
        let metadata: ProfileMetadata = serde_json::from_str(&metadata_file_string).map_err(Error::SerdeValueError)?;

        Ok(metadata)
    }

    /// Export the metadata in the filesystem. 
    pub fn export_metadata(&self) -> Result<()> {
        let mut file = fs::OpenOptions::new().write(true).create(true).open(self.metadata_path()).map_err(Error::IoError)?;
        let metadata_as_string = serde_json::to_string_pretty(&self.metadata).map_err(Error::SerdeValueError)?;
        file.write_all(metadata_as_string.as_bytes()).map_err(Error::IoError)?;
        
        Ok(())
    }

    /// Export the profile in the filesystem. 
    pub fn export(&mut self) -> Result<()> {
        let dir_builder = DirBuilder::new();

        if self.is_valid() {
            return Err(Error::ProfileAlreadyExists(self.path()));
        }

        if !self.is_exported() {
            helpers::filesystem::create_folder(&dir_builder, self.path())?;
        }

        if !self.has_common_files() {
            helpers::filesystem::create_folder(&dir_builder, self.common_files_path())?;
        }

        if !self.has_metadata() {
            self.export_metadata()?;
        }

        if !self.has_shelf() {
            self.notes.export()?;
        }

        if !self.has_templates() {
            helpers::filesystem::create_folder(&dir_builder, self.templates_path())?;
        }

        Ok(())
    }

    /// Create the subjects for the profile. 
    /// 
    /// Underneath, it just calls the same shelf function `shelf.create_subjects` method. 
    pub fn create_subjects(&self, subjects: &Vec<notes::Subject>, add_to_db: bool, strict: bool) -> Result<Vec<notes::Subject>> {
        self.notes.create_subjects(&subjects, add_to_db, strict)
    }

    /// Create the notes for the profile. 
    /// 
    /// Return the notes that have been successfully created by the shelf. 
    pub fn create_notes(
        &self, 
        subject: &notes::Subject, 
        notes: &Vec<notes::Note>, 
        add_to_db: bool, 
        strict: bool, 
        template_name: Option<&str>, 
    ) -> Result<Vec<notes::Note>> {
        let mut created_notes: Vec<notes::Note> = vec![];

        for note in notes.iter() {
            let template_string = self.return_string_from_note_template(&subject, &note, template_name)?;

            if self.notes.create_note(&subject, &note, &template_string, add_to_db, strict).is_ok() {
                created_notes.push(note.clone())
            }
        }

        Ok(created_notes)
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
        
        let compilation_environment = CompilationEnvironment::new(subject.clone(), note_list_in_reverse, command, thread_count);

        self.create_symlink_from_common_files_folder_with_subject(&subject)?;
        
        let compiled_notes_result = compilation_environment.compile(self.shelf());

        fs::remove_dir_all(self.symlink_to_common_path_from_subject(&subject)).map_err(Error::IoError)?;

        compiled_notes_result
    }

    /// Open an entry with the text editor. 
    pub fn open_entry(
        &mut self, 
        subject: &notes::Subject, 
        note: &notes::Note, 
        execute: String
    ) -> Result<()> {
        let mut editor_command_args: Vec<String> = match env::var("EDITOR") {
            Ok(v) => v.split_whitespace().map(| c | c.to_owned()).collect(), 
            Err(_e) => vec!["vi".to_string()], 
        };

        if editor_command_args.is_empty() {
            return Err(Error::ValueError);
        }

        editor_command_args.push(note.path_in_shelf(&subject, &self.notes).to_str().unwrap().to_string());
        
        let mut command = process::Command::new(editor_command_args.remove(0));

        for arg in editor_command_args {
            command.arg(arg);
        }

        let command_output = command.output().map_err(Error::IoError)?;
        match command_output.status.success() {
            true => Ok(()), 
            false => Err(Error::ProcessError(command_output.status))
        }
    }

    /// As self-explanatory as the function name, it creates a symlink pointing to the common files folder. 
    /// 
    /// This is mostly used for the compilation process or if you want to statically create a symlink for your notes. 
    pub fn create_symlink_from_common_files_folder<P: AsRef<Path>>(&self, dst: P) -> Result<()> {
        let path_relative_to_common_files = match helpers::filesystem::relative_path_from(self.common_files_path(), &dst) {
            Some(p) => p, 
            None => return Err(Error::ValueError), 
        };
        
        helpers::filesystem::create_symlink(path_relative_to_common_files, dst.as_ref())
    }

    pub fn create_symlink_from_common_files_folder_with_subject(&self, subject: &notes::Subject) -> Result<()> {
        let subject_path_with_common_files_folder = self.symlink_to_common_path_from_subject(&subject);
        let subject_path_relative_from_common_files_folder = helpers::filesystem::relative_path_from(
            self.common_files_path(),
            subject.path_in_shelf(self.shelf()), 
        ).unwrap();

        helpers::filesystem::create_symlink(subject_path_relative_from_common_files_folder, subject_path_with_common_files_folder)
    }

    /// Returns a relative path starting from the profile to the subject with the common files path. 
    /// 
    /// This is mainly used in the compilation process or dynamically generating symlinks. 
    fn symlink_to_common_path_from_subject(&self, subject: &notes::Subject) -> PathBuf {
        let mut subject_path = subject.path_in_shelf(&self.notes);
        subject_path.push(PROFILE_COMMON_FILES_DIR_NAME);

        subject_path
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
        
        assert_eq!(test_profile.export().is_ok(), true);
        
        let test_subjects = notes::Subject::from_vec_loose(&vec!["Calculus", "Algebra", "Physics"], &test_profile.notes)?;
        let test_notes = notes::Note::from_vec_loose(
            &vec!["Introduction to Precalculus", "Introduction to Integrations", "Taylor Series", "Introduction to Limits", "Matrices and Markov Chains"], 
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
        test_profile.compile_notes_in_parallel(&test_subjects[0], &test_notes, 2)?;
        assert_eq!(test_profile.symlink_to_common_path_from_subject(&test_subjects[0]), PathBuf::from("./tests/texture-notes-profile/notes/calculus/common"));

        assert!(test_profile.create_symlink_from_common_files_folder_with_subject(&test_subjects[0]).is_ok());

        let test_folder_name = PathBuf::from("./tests/");        
        assert_eq!(Profile::from(test_folder_name).is_ok(), true);
        
        Ok(())
    }

    #[test]
    fn invalid_profile_init_test() -> Result<()> {
        let test_path = PathBuf::from("./this/path/does/not/exists/");
        let mut test_profile_result = Profile::new(&test_path, None, true)?;

        assert!(test_profile_result.export().is_err());
        Ok(())
    }

    #[test]
    #[should_panic]
    fn invalid_profile_import() {
        let test_path = PathBuf::from("./this/path/also/does/not/exists/lol");
        assert!(Profile::from(test_path).is_ok(), "Profile is not valid.");
    }
}