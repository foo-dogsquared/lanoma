use std::env;
use std::fs;
use std::process;

use directories;
use lanoma_lib::config::SubjectConfig;
use lanoma_lib::error::Error;
use lanoma_lib::masternote::MasterNote;
use lanoma_lib::note::Note;
use lanoma_lib::profile::{
    Profile, ProfileBuilder, PROFILE_MASTER_NOTE_TEMPLATE_NAME, PROFILE_NOTE_TEMPLATE_NAME,
};
use lanoma_lib::shelf::{ExportOptions, Shelf, ShelfItem};
use lanoma_lib::subjects::Subject;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use structopt::StructOpt;

// the modules from this crate
mod args;
mod compile;
mod helpers;

use crate::args::{Command, Input, Lanoma};
use crate::compile::{Compilable, CompilationEnvironment};

fn main() {
    let args = Lanoma::from_args();

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
                Error::IoError(e) => println!("An IO error has occurred while Lanoma is running.\nERROR: {}", e),
                _ => println!("Unknown error."), 
            };

            process::exit(1)
        }
    };
}

fn parse_from_args(args: Lanoma) -> Result<(), Error> {
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
                        let object = helpers::note_full_object(&profile, &shelf, &note, &subject);
                        let template_string = profile
                            .template_registry()
                            .render(
                                &template
                                    .as_ref()
                                    .unwrap_or(&String::from(PROFILE_NOTE_TEMPLATE_NAME)),
                                &object,
                            )
                            .map_err(Error::HandlebarsRenderError)?;

                        if helpers::write_file(
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

            let compiled_notes_envs = match kind {
                Input::Notes { subject, notes } => {
                    let subject = Subject::from_shelf(&subject, &shelf)?;
                    let subject_config = subject.get_config(&shelf).unwrap_or(SubjectConfig::new());
                    let notes = Note::from_vec_loose(&notes, &subject, &shelf);
                    let mut compilables: Vec<Box<dyn Compilable>> = vec![];
                    for note in notes {
                        compilables.push(Box::new(note));
                    }

                    let mut compiled_notes_env =
                        CompilationEnvironment::new(subject.path_in_shelf(&shelf));
                    compiled_notes_env
                        .compilables(compilables)
                        .command(command.as_ref().unwrap_or(&subject_config.command))
                        .thread_count(thread_count as i16);
                    vec![compiled_notes_env]
                }
                Input::Subjects { subjects } => {
                    let mut envs: Vec<CompilationEnvironment> = vec![];

                    for subject in subjects.iter() {
                        let subject = Subject::from_shelf(&subject, &shelf)?;
                        let subject_config =
                            subject.get_config(&shelf).unwrap_or(SubjectConfig::new());
                        let file_filter = files.as_ref().unwrap_or(&subject_config.files);

                        println!("{:?}", &subject_config);
                        let notes = subject.get_notes_in_fs(&file_filter, &shelf)?;
                        let mut compilables: Vec<Box<dyn Compilable>> = vec![];
                        for note in notes {
                            compilables.push(Box::new(note));
                        }

                        let mut env = CompilationEnvironment::new(subject.path_in_shelf(&shelf));
                        env.command(command.as_ref().unwrap_or(&subject_config.command))
                            .compilables(compilables)
                            .thread_count(thread_count as i16);

                        envs.push(env);
                    }

                    envs
                }
            };

            compiled_notes_envs.into_par_iter()
            .map(|comp_env| {
                let path = comp_env.path.clone();
                let compiled_notes = match comp_env.compile() {
                    Ok(v) => v,
                    Err(_e) => return,
                };

                if compiled_notes.len() == 0 {
                    println!("No notes successfully ran the compile command under the path {:?}.", path) ;
                    println!("Please check for the command if it's valid or the note exists in the filesystem.");
                } else {
                    println!(
                        "Here are the compiled note that successfully run the compile command in path {:?}:", path
                    );
                    for compiled_note in compiled_notes {
                        println!("  - {}", compiled_note.name());
                    }
                }
            })
            .collect::<()>();
        }
        Command::Master {
            subjects,
            skip_compilation,
            files,
            template,
            command,
        } => {
            let profile = Profile::from(&profile_path)?;

            let compiled_master_notes: Vec<MasterNote> = subjects
                .into_par_iter()
                .map(|subject| Subject::from_shelf(&subject, &shelf))
                .filter(|subject| subject.is_ok())
                .map(|subject| {
                    let subject = subject.unwrap();
                    let subject_config = subject.get_config(&shelf).unwrap_or(SubjectConfig::new());
                    let files = files.as_ref().unwrap_or(&subject_config.files);

                    let notes = subject.get_notes_in_fs(&files, &shelf).unwrap();
                    let mut master_note = MasterNote::new(subject.clone());
                    for note in notes.iter() {
                        master_note.push(&note);
                    }

                    (master_note, subject_config)
                })
                .filter(|(master_note, _)| {
                    let master_note_object =
                        helpers::master_note_full_object(&profile, &shelf, &master_note);
                    let resulting_string = profile
                        .template_registry()
                        .render(
                            &template
                                .as_ref()
                                .unwrap_or(&PROFILE_MASTER_NOTE_TEMPLATE_NAME.into()),
                            &master_note_object,
                        )
                        .map_err(Error::HandlebarsRenderError)
                        .unwrap();

                    helpers::write_file(master_note.path_in_shelf(&shelf), resulting_string, false)
                        .is_ok()
                })
                .filter(|(master_note, config)| {
                    if !skip_compilation {
                        let original_dir = env::current_dir().map_err(Error::IoError).unwrap();
                        let compilation_dst = master_note.subject().path_in_shelf(&shelf);

                        env::set_current_dir(&compilation_dst)
                            .map_err(Error::IoError)
                            .unwrap();
                        let mut master_note_compilation_cmd =
                            master_note.to_command(command.as_ref().unwrap_or(&config.command));
                        let output = master_note_compilation_cmd.output().unwrap();
                        env::set_current_dir(original_dir)
                            .map_err(Error::IoError)
                            .unwrap();

                        output.status.success()
                    } else {
                        false
                    }
                })
                .map(|(master_note, _)| {
                    println!(
                        "\n{:?} has successfully compiled a master note\nwith the following filtered notes.",
                        master_note.subject().full_name()
                    );

                    for note in master_note.notes() {
                        println!("  - {:?}", note.title());
                    }

                    master_note
                })
                .collect();
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
        let command_args_as_vec = vec!["lanoma", "--shelf", "this/path/does/not/exist", "init"];
        let command_args = Lanoma::from_iter(command_args_as_vec.iter());

        assert_eq!(parse_from_args(command_args).is_err(), true);
    }
}
