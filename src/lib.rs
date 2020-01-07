use std::env;
use std::process;
use std::result;
use std::str::FromStr;
use std::sync;
use std::thread;

use toml::{self};

#[macro_use]
extern crate lazy_static;

mod consts;
pub mod error;
mod helpers;
pub mod items;
pub mod profile;
pub mod shelf;
pub mod subjects;
pub mod templates;
mod threadpool;

use crate::items::Note;
use crate::shelf::{Shelf, ShelfItem};
use crate::subjects::Subject;
use error::Error;

pub type Result<T> = result::Result<T, Error>;

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
    pub fn new() -> Self {
        Self {
            subject: Subject::new(),
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
    /// This does not consume the struct.
    pub fn compile(
        &self,
        shelf: &Shelf,
    ) -> Result<Vec<Note>> {
        let original_dir = env::current_dir().map_err(Error::IoError)?;
        let compilation_dst = self.subject.path_in_shelf(&shelf);
        env::set_current_dir(&compilation_dst).map_err(Error::IoError)?;

        // this will serve as a task queue for the threads to be spawned
        let compilation_environment = sync::Arc::new(sync::Mutex::new(self.clone()));
        let compiled_notes = sync::Arc::new(sync::Mutex::new(vec![]));
        let mut threads = vec![];

        for _i in 0..self.thread_count {
            let compilation_environment_mutex = sync::Arc::clone(&compilation_environment);
            let compiled_notes_mutex = sync::Arc::clone(&compiled_notes);
            let thread = thread::spawn(move || {
                let mut compilation_environment = compilation_environment_mutex.lock().unwrap();
                let mut compiled_notes = compiled_notes_mutex.lock().unwrap();
                let handlebars_reg = handlebars::Handlebars::new();

                while let Some(note) = compilation_environment.notes.pop() {
                    let resulting_toml = format!("note = '{}'", note.file_name());
                    let note_as_toml = toml::Value::from_str(&resulting_toml).unwrap();
                    let command_vector = handlebars_reg
                        .render_template(&compilation_environment.command, &note_as_toml)
                        .unwrap();
                    let mut command_iter = command_vector.split_whitespace();

                    let mut command_process = process::Command::new(command_iter.next().unwrap());
                    for arg in command_iter.into_iter() {
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
