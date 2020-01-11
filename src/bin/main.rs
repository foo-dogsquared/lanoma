use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process;

use directories;
use structopt::StructOpt;

use std::path::PathBuf;

extern crate texture_notes_v2;
use texture_notes_v2::error::Error;
use texture_notes_v2::masternote::MasterNote;
use texture_notes_v2::note::Note;
use texture_notes_v2::profile::{
    Profile, ProfileBuilder, PROFILE_MASTER_NOTE_TEMPLATE_NAME, PROFILE_NOTE_TEMPLATE_NAME,
};
use texture_notes_v2::shelf::{ExportOptions, Shelf, ShelfData, ShelfItem};
use texture_notes_v2::subjects::Subject;
use texture_notes_v2::threadpool::ThreadPool;
use texture_notes_v2::CompilationEnvironment;
use texture_notes_v2::Object;

#[macro_use]
use texture_notes_v2::{modify_toml_table, upsert_toml_table};

#[derive(Debug, StructOpt)]
#[structopt(name = "Texture Notes", about = "Manage your LaTeX study notes.")]
pub struct TextureNotes {
    #[structopt(
        short,
        long,
        parse(from_os_str),
        value_name = "path",
        help = "Sets the shelf directory."
    )]
    shelf: Option<PathBuf>,

    #[structopt(
        short,
        long,
        parse(from_os_str),
        value_name = "path",
        help = "Searches for the profile in the specified directory. By default, it searches in the default configuration folder of the filesystem (e.g., '%USERPROFILE%\\AppData\\Roaming' for Windows, '$HOME/.config/' for Linux)."
    )]
    profile: Option<PathBuf>,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
pub enum Input {
    Subjects {
        #[structopt(help = "Add a list of subjects.", min_values = 1)]
        subjects: Vec<String>,
    },

    Notes {
        #[structopt(help = "The subject of the notes.")]
        subject: String,

        #[structopt(
            min_values = 1,
            multiple = true,
            required = true,
            last = true,
            help = "A list of notes for a particular subject. Requires the subject as the first item in the list."
        )]
        notes: Vec<String>,
    },
}

#[derive(Debug, StructOpt)]
pub enum Command {
    #[structopt(about = "Initialize a profile.")]
    Init {
        #[structopt(short, long, help = "Set the name of the profile.")]
        name: Option<String>,
    },

    #[structopt(about = "Add multiple subjects and notes in the database.")]
    Add {
        #[structopt(short, long, parse(from_flag = std::ops::Not::not), help = "Force to replace the resulting files in the filesystem.")]
        not_strict: bool,

        #[structopt(subcommand)]
        kind: Input,

        #[structopt(
            short,
            long,
            help = "The name of the template to be used for creating the notes."
        )]
        template: Option<String>,
    },

    #[structopt(about = "Remove multiple subjects and notes in the database.")]
    Remove {
        #[structopt(subcommand)]
        kind: Input,
    },

    #[structopt(about = "Lists the subjects and its notes from the database.")]
    List {
        #[structopt(short, long, possible_values = &["date", "name"], help = "Sort the entries.")]
        sort: Option<String>,

        #[structopt(
            short,
            long,
            help = "Sort the entries by the modification datetime in the filesystem. If this is set to true, the rest of the sorting options are ignored."
        )]
        date: bool,

        #[structopt(short, long, help = "Reverse the list.")]
        reverse: bool,
    },

    #[structopt(about = "Compile the notes.")]
    Compile {
        #[structopt(subcommand)]
        kind: Input,

        #[structopt(
            short,
            long,
            default_value = "4",
            help = "Creates a specified number of threads compiling in parallel."
        )]
        thread_count: i64,

        #[structopt(short, long, help = "Specifies what files to be compiled.")]
        files: Option<Vec<String>>,

        #[structopt(short, long, help = "Overrides the default compilation command.")]
        command: Option<String>,
    },

    #[structopt(about = "A subcommand dedicated to interact with master notes.")]
    Master {
        #[structopt(help = "Add a list of subjects.", min_values = 1)]
        subjects: Vec<String>,

        #[structopt(short, long, help = "Skip the compilation step.")]
        skip_compilation: bool,

        #[structopt(
            short,
            long,
            help = "Specifies what files to be compiled to the master note."
        )]
        files: Option<Vec<String>>,

        #[structopt(
            short,
            long,
            help = "The name of the template to be used for creating the notes."
        )]
        template: Option<String>,

        #[structopt(
            short,
            long,
            help = "The command to be used to compile the master note."
        )]
        command: Option<String>,
    },
}

fn main() {
    let args = TextureNotes::from_args();

    match parse_from_args(args) {
        Ok(()) => (),
        Err(e) => {
            match e {
                Error::InvalidProfileError(path) => println!("Profile at {:?} is not valid or nonexistent.\nMake sure to export it successfully.", path),
                Error::InvalidSubjectError(path) => println!("Subject at {:?} is not valid or nonexistent.", path),
                Error::ProfileAlreadyExists(path) => println!("Profile at {:?} already exists.", path), 
                Error::ProcessError(exit) => println!("The child process has exit with status code {}", exit.code().unwrap()),
                Error::UnexportedShelfError(path) => println!("The shelf at {:?} is not exported.", path),
                Error::TomlValueError(e) => println!("A TOML parsing error occurred.\nERROR: {}", e), 
                Error::HandlebarsTemplateError(e) => println!("There's something wrong with the Handlebars template.\nERROR: {}", e), 
                Error::HandlebarsTemplateFileError(e) => println!("There's something wrong with the Handlebars template.\nERROR: {}", e), 
                Error::HandlebarsRenderError(e) => println!("An error has occurred while rendering the Handlebars template\nERROR: {}", e), 
                Error::IoError(e) => println!("An IO error has occurred while Texture Notes is running.\nERROR: {}", e),
                _ => println!("Unknown error."), 
            };

            process::exit(1)
        }
    };
}

fn parse_from_args(args: TextureNotes) -> Result<(), Error> {
    let user_dirs = directories::BaseDirs::new().unwrap();
    let mut config_app_dir = user_dirs.config_dir().to_path_buf();
    config_app_dir.push(env!("CARGO_PKG_NAME"));

    let shelf = match args.shelf {
        Some(p) => Shelf::from(fs::canonicalize(p).map_err(Error::IoError)?)?,
        None => Shelf::from(env::current_dir().map_err(Error::IoError)?)?,
    };

    let profile_path = match args.profile {
        Some(p) => p,
        None => config_app_dir,
    };

    match args.cmd {
        Command::Init { name } => {
            let mut profile_builder = ProfileBuilder::new();
            profile_builder.path(profile_path);

            if name.is_some() {
                let name = name.unwrap();

                profile_builder.name(name);
            }

            let mut profile = profile_builder.build();

            profile.export()?;

            println!("Profile at {:?} successfully initialized.", profile.path());
        }
        Command::Add {
            kind,
            not_strict,
            template,
        } => {
            let profile = Profile::from(&profile_path)?;
            let mut export_options = ExportOptions::new();
            export_options.strict(not_strict);

            match kind {
                Input::Notes { subject, notes } => {
                    let subject = Subject::from_shelf(&subject, &shelf)?;
                    let notes: Vec<Note> = notes.iter().map(|note| Note::new(note)).collect();

                    let mut created_notes: Vec<Note> = vec![];
                    for note in notes {
                        let object = note_full_object(&profile, &shelf, &note, &subject);
                        let template_string = profile
                            .template_registry()
                            .render(
                                &template
                                    .as_ref()
                                    .unwrap_or(&String::from(PROFILE_NOTE_TEMPLATE_NAME)),
                                &object,
                            )
                            .map_err(Error::HandlebarsRenderError)?;
                        println!(
                            "Object:\n{:?}\n\nTemplate:\n{:?}",
                            &object, &template_string
                        );

                        if write_file(
                            note.path_in_shelf((&subject, &shelf)),
                            template_string,
                            not_strict,
                        )
                        .is_ok()
                        {
                            created_notes.push(note)
                        }
                    }

                    println!("Here are the notes under the subject {:?} that successfully created in the shelf.", subject.name());
                    for note in created_notes {
                        println!("  - {:?}", note.title());
                    }
                }
                Input::Subjects { subjects } => {
                    let created_subjects: Vec<Subject> = Subject::from_vec_loose(&subjects, &shelf)
                        .into_iter()
                        .filter(|subject| subject.export(&shelf).is_ok())
                        .collect();

                    if created_subjects.len() <= 0 {
                        println!("No subjects has been created.");
                    } else {
                        println!(
                        "Here are the subjects that have been successfully created in the shelf."
                        );
                        for subject in created_subjects {
                            println!("  - {:?}", subject.full_name());
                        }
                    }
                }
            }
        }
        Command::Remove { kind } => match kind {
            Input::Subjects { subjects } => {
                let deleted_subjects: Vec<Subject> = Subject::from_vec_loose(&subjects, &shelf)
                    .into_iter()
                    .filter(|subject| subject.delete(&shelf).is_ok())
                    .collect();

                println!("{:?}", deleted_subjects);
            }
            Input::Notes { subject, notes } => {
                let subject = Subject::from_shelf(&subject, &shelf)?;
                let deleted_notes: Vec<Note> = Note::from_vec_loose(&notes, &subject, &shelf)
                    .into_iter()
                    .filter(|note| note.delete((&subject, &shelf)).is_ok())
                    .collect();

                println!("The following notes has been deleted successfully:");
                for note in deleted_notes.iter() {
                    println!(" - {}", note.title());
                }
            }
        },
        Command::Compile {
            kind,
            thread_count,
            files,
            command,
        } => {
            let profile = Profile::from(&profile_path)?;
            let command = command.unwrap_or(profile.compile_note_command());

            let compiled_notes_envs = match kind {
                Input::Notes { subject, notes } => {
                    let subject = Subject::from_shelf(&subject, &shelf)?;
                    let notes = Note::from_vec_loose(&notes, &subject, &shelf);

                    let mut compiled_notes_env = CompilationEnvironment::new(subject);
                    compiled_notes_env
                        .notes(notes)
                        .command(command)
                        .thread_count(thread_count as i16);
                    vec![compiled_notes_env]
                }
                Input::Subjects { subjects } => {
                    let mut envs: Vec<CompilationEnvironment> = vec![];

                    for subject in subjects.iter() {
                        let subject = Subject::from_shelf(&subject, &shelf)?;
                        let subject_config = subject.get_config(&shelf)?;
                        let file_filter = files.as_ref().unwrap_or(&subject_config.files);

                        let notes = subject.get_notes_in_fs(&file_filter, &shelf)?;
                        let mut env = CompilationEnvironment::new(subject);
                        env.command(command.clone())
                            .notes(notes)
                            .thread_count(thread_count as i16);

                        envs.push(env);
                    }

                    envs
                }
            };

            for comp_env in compiled_notes_envs {
                let compiled_notes = match comp_env.compile(&shelf) {
                    Ok(v) => v,
                    Err(_e) => continue,
                };

                if compiled_notes.len() == 0 {
                    println!("No notes successfully ran the compile command under the subject. Please check for the command if it's valid or the note exists in the filesystem.")
                } else {
                    println!(
                        "Here are the compiled note that successfully run the compile command:"
                    );
                    for compiled_note in compiled_notes {
                        println!("  - {}", compiled_note.title());
                    }
                }
            }
        }
        Command::Master {
            subjects,
            skip_compilation,
            files,
            template,
            command,
        } => {
            let profile = Profile::from(&profile_path)?;
            let command = command.unwrap_or(profile.compile_note_command());

            let thread_pool = ThreadPool::new(4);
            for subject_name in subjects {
                let subject = match Subject::from_shelf(&subject_name, &shelf) {
                    Ok(v) => v,
                    Err(_e) => continue,
                };
                let subject_config = subject.get_config(&shelf)?;
                let files = files.as_ref().unwrap_or(&subject_config.files);

                let notes = subject.get_notes_in_fs(&files, &shelf)?;
                let mut master_note = MasterNote::new(subject.clone());
                for note in notes.iter() {
                    master_note.push(&note);
                }
                let master_note_object = master_note_full_object(&profile, &shelf, &master_note);
                let resulting_string = profile
                    .template_registry()
                    .render(
                        &template
                            .as_ref()
                            .unwrap_or(&PROFILE_MASTER_NOTE_TEMPLATE_NAME.into()),
                        &master_note_object,
                    )
                    .map_err(Error::HandlebarsRenderError)?;
                write_file(master_note.path_in_shelf(&shelf), resulting_string, false)?;

                if !skip_compilation {
                    let original_dir = env::current_dir().map_err(Error::IoError)?;
                    let compilation_dst = subject.path_in_shelf(&shelf);
                    let command = command.clone();
                    env::set_current_dir(&compilation_dst).map_err(Error::IoError)?;
                    thread_pool.execute(move || {
                        let mut master_note_compilation_cmd =
                            texture_notes_v2::master_note_to_cmd(&master_note, command);
                        let output = master_note_compilation_cmd.output();
                    });
                    env::set_current_dir(original_dir).map_err(Error::IoError)?;
                }
            }
        }
        _ => (),
    }

    Ok(())
}

fn master_note_full_object(
    profile: &Profile,
    shelf: &Shelf,
    master_note: &MasterNote,
) -> toml::Value {
    let subject_as_toml = ShelfData::data(master_note.subject(), &shelf);
    let master_note_as_toml = ShelfData::data(master_note, &shelf);
    let profile_config = Object::data(profile);

    let mut metadata = toml::Value::from(HashMap::<String, toml::Value>::new());
    modify_toml_table! {metadata,
        ("profile", profile_config),
        ("subject", subject_as_toml),
        ("master", master_note_as_toml),
        ("date", chrono::Local::now().format("%F").to_string())
    }

    metadata
}

fn note_full_object(
    profile: &Profile,
    shelf: &Shelf,
    note: &Note,
    subject: &Subject,
) -> toml::Value {
    let subject_toml = ShelfData::data(subject, &shelf);
    let note_toml = ShelfData::data(note, (&subject, &shelf));
    let profile_config = Object::data(profile);

    // The metadata is guaranteed to be valid since the codebase enforces it to be valid either at creation
    // or at retrieval from a folder.
    // It is safe to call `unwrap` from here.
    let mut metadata = toml::Value::from(HashMap::<String, toml::Value>::new());
    modify_toml_table! {metadata,
        ("profile", profile_config),
        ("subject", subject_toml),
        ("note", note_toml),
        ("date", toml::Value::String(chrono::Local::now().format("%F").to_string()))
    };

    metadata
}

/// A generic function for writing a shelf item (as a file).
fn write_file<P, S>(
    path: P,
    string: S,
    strict: bool,
) -> Result<(), Error>
where
    P: AsRef<Path>,
    S: AsRef<str>,
{
    let path = path.as_ref();
    let mut file_open_options = OpenOptions::new();
    file_open_options.write(true);

    if strict {
        file_open_options.create_new(true);
    } else {
        file_open_options.create(true).truncate(true);
    }

    let mut file = file_open_options.open(path).map_err(Error::IoError)?;
    file.write(string.as_ref().as_bytes())
        .map_err(Error::IoError)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn basic_usage_test() {
        let command_args_as_vec = vec![
            "texture-notes-v2",
            "--shelf",
            "this/path/does/not/exist",
            "init",
        ];
        let command_args = TextureNotes::from_iter(command_args_as_vec.iter());

        assert_eq!(parse_from_args(command_args).is_err(), true);
    }
}
