use std::env;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;

use rayon::iter::{IntoParallelIterator, ParallelIterator};
use toml;
use texture_notes_lib::error::Error;
use texture_notes_lib::masternote::MasterNote;
use texture_notes_lib::note::Note;
use texture_notes_lib::HANDLEBARS_REG;

use crate::helpers;

/// A trait that converts an object into a command struct.
pub trait Compilable: Send + Sync {
    fn to_command(
        &self,
        command: &str,
    ) -> process::Command;

    fn name(&self) -> String;

    fn compile(
        &self,
        cmd: &str,
    ) -> Result<process::Output, Error> {
        self.to_command(&cmd).output().map_err(Error::IoError)
    }
}

impl Compilable for MasterNote {
    fn to_command(
        &self,
        cmd: &str,
    ) -> process::Command {
        let resulting_toml = format!("note = '{}'", self.file_name());
        let note_as_toml = toml::Value::from_str(&resulting_toml).unwrap();
        let command_string = HANDLEBARS_REG.render_template(&cmd, &note_as_toml).unwrap();

        helpers::str_as_cmd(command_string)
    }

    fn name(&self) -> String {
        self.subject().name()
    }
}

impl Compilable for Note {
    fn to_command(
        &self,
        cmd: &str,
    ) -> process::Command {
        let resulting_toml = format!("note = '{}'", self.file_name());
        let note_as_toml = toml::Value::from_str(&resulting_toml).unwrap();
        let command_string = HANDLEBARS_REG.render_template(&cmd, &note_as_toml).unwrap();

        helpers::str_as_cmd(command_string)
    }

    fn name(&self) -> String {
        self.title()
    }
}

/// A struct for handling the parameters for the compilation environment.
///
/// This data structure is made for abstracting the compilation process making it as a separate component.
/// Ideally, this is used for compiling a subject and its notes/master note.
pub struct CompilationEnvironment {
    path: PathBuf,
    compilables: Vec<Box<dyn Compilable>>,
    command: String,
    thread_count: i16,
}

impl Default for CompilationEnvironment {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            compilables: vec![],
            command: String::new(),
            thread_count: 1,
        }
    }
}

impl CompilationEnvironment {
    /// Create a new compilation environment instance.
    pub fn new<S>(path: S) -> Self
    where
        S: AsRef<Path>,
    {
        let mut compilation_env = Self::default();
        compilation_env.path = path.as_ref().to_path_buf();

        compilation_env
    }

    /// Set the subject of the notes to be compiled.
    pub fn path<S>(
        &mut self,
        path: S,
    ) -> &mut Self
    where
        S: AsRef<Path>,
    {
        self.path = path.as_ref().to_path_buf();
        self
    }

    /// Set the notes to be compiled.
    pub fn compilables(
        &mut self,
        notes: Vec<Box<dyn Compilable>>,
    ) -> &mut Self {
        // Reversing the note vector since the compilation process pops off the vector.
        self.compilables = notes;
        self.compilables.reverse();
        self
    }

    /// Set the command.
    pub fn command(
        &mut self,
        command: String,
    ) -> &mut Self {
        self.command = command;
        self
    }

    /// Set the thread count.
    pub fn thread_count(
        &mut self,
        thread_count: i16,
    ) -> &mut Self {
        self.thread_count = thread_count;
        self
    }

    /// Executes the compilation process.
    /// This also consume the struct.
    pub fn compile(mut self) -> Result<Vec<Box<dyn Compilable>>, Error> {
        let original_dir = env::current_dir().map_err(Error::IoError)?;

        env::set_current_dir(self.path.clone()).map_err(Error::IoError)?;
        let command = self.command.clone();
        let compiled_notes: Vec<Box<dyn Compilable>> = self
            .compilables
            .into_par_iter()
            .filter(|compilable| compilable.compile(&command).unwrap().status.success())
            .collect();
        env::set_current_dir(original_dir).map_err(Error::IoError)?;

        Ok(compiled_notes)
    }
}
