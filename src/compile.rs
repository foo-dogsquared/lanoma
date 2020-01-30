use std::env;
use std::fmt::{self, Debug, Display, Formatter};
use std::iter::Sum;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;

use lanoma_lib::error::Error;
use lanoma_lib::masternote::MasterNote;
use lanoma_lib::note::Note;
use lanoma_lib::HANDLEBARS_REG;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use toml;

use crate::helpers;

pub type CompilableObject = Box<dyn Compilable>;

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

impl Display for dyn Compilable {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl Debug for dyn Compilable {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "{:?}", self.name())
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

/// The result from the compilation process of the compenv.
pub struct CompileResult {
    pub path: PathBuf,
    pub compiled: Vec<CompilableObject>,
    pub failed: Vec<CompilableObject>,
}

impl Sum for CompileResult {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.fold(Self::new(PathBuf::new()), |mut acc, mut object| {
            acc.path = object.path;
            acc.compiled.append(&mut object.compiled);
            acc.failed.append(&mut object.failed);

            acc
        })
    }
}

impl CompileResult {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            compiled: Vec::new(),
            failed: Vec::new(),
        }
    }
}

/// A struct for handling the parameters for the compilation environment.
///
/// This data structure is made for abstracting the compilation process making it as a separate component.
/// Ideally, this is used for compiling a subject and its notes/master note.
pub struct CompilationEnvironment {
    pub path: PathBuf,
    pub compilables: Vec<CompilableObject>,
    pub command: String,
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

    /// Set the notes to be compiled.
    pub fn compilables(
        &mut self,
        notes: Vec<CompilableObject>,
    ) -> &mut Self {
        // Reversing the note vector since the compilation process pops off the vector.
        self.compilables = notes;
        self.compilables.reverse();
        self
    }

    /// Set the command.
    pub fn command<S>(
        &mut self,
        command: S,
    ) -> &mut Self
    where
        S: AsRef<str>,
    {
        self.command = command.as_ref().to_string();
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
    pub fn compile(self) -> Result<CompileResult, Error> {
        let original_dir = env::current_dir().map_err(Error::IoError)?;

        env::set_current_dir(self.path.clone()).map_err(Error::IoError)?;
        let compilables = self.compilables;
        let path = self.path;
        let command = self.command;

        let compile_result = compilables
            .into_par_iter()
            .fold(
                || CompileResult::new(path.clone()),
                |mut result_struct, compilable| {
                    match compilable.compile(&command) {
                        Ok(output) => {
                            if output.status.success() {
                                result_struct.compiled.push(compilable);
                            } else {
                                result_struct.failed.push(compilable);
                            }
                        }
                        Err(_e) => result_struct.failed.push(compilable),
                    }

                    result_struct
                },
            )
            .sum();
        env::set_current_dir(original_dir).map_err(Error::IoError)?;

        Ok(compile_result)
    }
}
