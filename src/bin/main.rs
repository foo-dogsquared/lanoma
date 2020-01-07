use std::env;
use std::fs;
use std::process;

use directories;
use structopt::StructOpt;

use std::path::PathBuf;

extern crate texture_notes_v2;
use texture_notes_v2::error::Error;
use texture_notes_v2::items::Note;
use texture_notes_v2::profile::{Profile, ProfileBuilder};
use texture_notes_v2::shelf::{ExportOptions, Shelf};
use texture_notes_v2::subjects::Subject;
use texture_notes_v2::CompilationEnvironment;

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
    },
}

fn main() {
    let args = TextureNotes::from_args();

    match cli(args) {
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

fn cli(args: TextureNotes) -> Result<(), Error> {
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
                    let notes = Note::from_vec_loose(&notes, &subject, &shelf)?;

                    let mut created_notes: Vec<Note> = vec![];
                    for note in notes {
                        let template_string = profile
                            .return_string_from_note_template(&shelf, &subject, &note, &template)?;

                        if shelf
                            .create_note(&subject, &note, &template_string, &export_options)
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
                    let subjects = Subject::from_vec_loose(&subjects, &shelf);
                    let created_subjects = shelf.create_subjects(&subjects);

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
                let subjects = Subject::from_vec_loose(&subjects, &shelf);
                let deleted_subjects = shelf.delete_subjects(&subjects);

                println!("{:?}", deleted_subjects);
            }
            Input::Notes { subject, notes } => {
                let subject = Subject::from_shelf(&subject, &shelf)?;
                let notes = Note::from_vec_loose(&notes, &subject, &shelf)?;

                let deleted_notes = shelf.delete_notes(&subject, &notes);

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
                    let notes = Note::from_vec_loose(&notes, &subject, &shelf)?;

                    let mut compiled_notes_env = CompilationEnvironment::new();
                    compiled_notes_env
                        .subject(subject)
                        .notes(notes)
                        .command(command)
                        .thread_count(thread_count as i16);
                    vec![compiled_notes_env]
                }
                Input::Subjects { subjects } => {
                    let mut envs: Vec<CompilationEnvironment> = vec![];

                    for subject in subjects.iter() {
                        let subject = Subject::from_shelf(&subject, &shelf)?;
                        let _file_filter = subject.note_filter(&shelf);
                        let file_filter = files.as_ref().unwrap_or(&_file_filter);

                        let notes = shelf.get_notes_in_fs(&file_filter, &subject)?;
                        let mut env = CompilationEnvironment::new();
                        env.command(command.clone())
                            .notes(notes)
                            .subject(subject)
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
        } => {
            let profile = Profile::from(&profile_path)?;

            for subject in subjects {
                let subject = Subject::from_shelf(&subject, &shelf)?;
                let _files = subject.note_filter(&shelf);
                let files = files.as_ref().unwrap_or(&_files);

                let notes = shelf.get_notes_in_fs(&files, &subject)?;
                let mut master_note = subject.create_master_note();
                for note in notes.iter() {
                    master_note.push(&note);
                }
                let master_note_template = profile.return_string_from_master_note_template(
                    &shelf,
                    &master_note,
                    &template,
                )?;
                master_note.export(&shelf, master_note_template)?;

                if !skip_compilation {}
            }
        }
        _ => (),
    }

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

        assert_eq!(cli(command_args).is_err(), true);
    }
}
