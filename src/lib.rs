use std::env;
use std::process;
use std::result;
use std::str::FromStr;
use std::sync;
use std::thread;

use handlebars;
use toml::{self};

#[macro_use]
extern crate lazy_static;

pub mod config;
mod consts;
pub mod error;
mod helpers;
pub mod masternote;
pub mod note;
pub mod profile;
pub mod shelf;
pub mod subjects;
pub mod templates;
pub mod threadpool;

use crate::masternote::MasterNote;
use crate::note::Note;
use crate::shelf::{Shelf, ShelfItem};
use crate::subjects::Subject;
use error::Error;

pub type Result<T> = result::Result<T, Error>;

// Making it static since it does not handle any templates anyway and only here for rendering the string.
lazy_static! {
    static ref HANDLEBARS_REG: handlebars::Handlebars = handlebars::Handlebars::new();
}

pub trait Object {
    fn data(&self) -> toml::Value;
}

/// A basic macro for modifying a TOML table.
#[macro_export]
macro_rules! modify_toml_table {
    ($var:ident, $( ($field:expr, $value:expr) ),*) => {
        let temp_table = $var.as_table_mut().unwrap();

        $(
            temp_table.insert(String::from($field), toml::Value::try_from($value).unwrap());
        )*
    };
}

/// A basic macro for upserting a TOML table.
#[macro_export]
macro_rules! upsert_toml_table {
    ($var:ident, $( ($field:expr, $value:expr) ),*) => {
        let temp_table = $var.as_table_mut().unwrap();

        $(
            if temp_table.get($field).is_none() {
                temp_table.insert(String::from($field), toml::Value::try_from($value).unwrap());
            }
        )*
    };
}

/// A struct for handling the parameters for the compilation environment.
///
/// This data structure is made for abstracting the compilation process making it as a separate component.
#[derive(Clone, Debug)]
pub struct CompilationEnvironment {
    subject: Subject,
    notes: Vec<Note>,
    command: String,
    thread_count: i16,
}

impl CompilationEnvironment {
    /// Create a new compilation environment instance.
    pub fn new<S>(subject: S) -> Self
    where
        S: AsRef<str>,
    {
        Self {
            subject: Subject::new(subject),
            notes: vec![],
            command: String::new(),
            thread_count: 1,
        }
    }

    /// Set the subject of the notes to be compiled.
    pub fn subject(
        &mut self,
        subject: Subject,
    ) -> &mut Self {
        self.subject = subject;
        self
    }

    /// Set the notes to be compiled.
    pub fn notes(
        &mut self,
        notes: Vec<Note>,
    ) -> &mut Self {
        // Reversing the note vector since the compilation process pops off the vector.
        self.notes = notes;
        self.notes.reverse();
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
    pub fn compile(
        self,
        shelf: &Shelf,
    ) -> Result<Vec<Note>> {
        let original_dir = env::current_dir().map_err(Error::IoError)?;
        let compilation_dst = self.subject.path_in_shelf(&shelf);
        env::set_current_dir(&compilation_dst).map_err(Error::IoError)?;

        // this will serve as a task queue for the threads to be spawned
        let thread_count = self.thread_count;
        let compilation_environment = sync::Arc::new(sync::Mutex::new(self));
        let compiled_notes = sync::Arc::new(sync::Mutex::new(vec![]));
        let mut threads = vec![];
        let thread_pool = threadpool::ThreadPool::new(thread_count as usize);

        for _i in 0..thread_count {
            let compilation_environment_mutex = sync::Arc::clone(&compilation_environment);
            let compiled_notes_mutex = sync::Arc::clone(&compiled_notes);
            let thread = thread::spawn(move || {
                let mut compilation_environment = compilation_environment_mutex.lock().unwrap();
                let mut compiled_notes = compiled_notes_mutex.lock().unwrap();

                while let Some(note) = compilation_environment.notes.pop() {
                    let mut command_process = note_to_cmd(&note, &compilation_environment.command);

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

pub fn str_as_cmd<S>(string: S) -> process::Command
where
    S: AsRef<str>,
{
    let string = string.as_ref();
    let mut command_iter = string.split_whitespace();

    let mut command_process = process::Command::new(command_iter.next().unwrap());
    for arg in command_iter.into_iter() {
        command_process.arg(arg);
    }

    command_process
}

pub fn note_to_cmd<S>(
    note: &Note,
    cmd: S,
) -> process::Command
where
    S: AsRef<str>,
{
    let cmd = cmd.as_ref();
    let resulting_toml = format!("note = '{}'", note.file_name());
    let note_as_toml = toml::Value::from_str(&resulting_toml).unwrap();
    let command_string = HANDLEBARS_REG.render_template(&cmd, &note_as_toml).unwrap();

    str_as_cmd(command_string)
}

pub fn master_note_to_cmd<S>(
    master_note: &MasterNote,
    cmd: S,
) -> process::Command
where
    S: AsRef<str>,
{
    let cmd = cmd.as_ref();
    let resulting_toml = format!("note = '{}'", master_note.file_name());
    let note_as_toml = toml::Value::from_str(&resulting_toml).unwrap();
    let command_string = HANDLEBARS_REG.render_template(&cmd, &note_as_toml).unwrap();

    str_as_cmd(command_string)
}
