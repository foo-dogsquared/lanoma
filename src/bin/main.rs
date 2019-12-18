use std::env;
use std::process;

// this is where the setup for the program will happen 
// and where the setup for the hook scripts
use structopt::StructOpt; 

use std::path::{ PathBuf };

extern crate texture_notes_v2;
use texture_notes_v2::error::Error;

pub const TEXTURE_NOTES_DIR_NAME: &str = "texture-notes-profile";

// Search for a pattern in a file and display the lines that contain it.
#[derive(Debug, StructOpt)]
#[structopt(name = "Texture Notes", about = "Manage your LaTeX study notes.")]
pub struct TextureNotes {
    #[structopt(short, long, parse(from_os_str), value_name = "path", help = "Sets the target directory to be used.")]
    target: Option<PathBuf>,

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

        #[structopt(min_values = 1, multiple = true, required = true, last = true, 
        help = "A list of notes for a particular subject. Requires the subject as the first item in the list.")]
        notes: Vec<String>,  
    }
}

#[derive(Debug, StructOpt)]
pub enum FullInput {
    Subjects {
        #[structopt(help = "Add a list of subjects.", min_values = 1)]
        subjects: Vec<String>, 
    }, 

    Notes {
        #[structopt(help = "The subject of the notes.")]
        subject: String, 

        #[structopt(min_values = 1, multiple = true, last = true, required = true, 
        help = "A list of notes for a particular subject. Requires the subject as the first item in the list.")]
        notes: Vec<String>,  
    },

    SubjectIds {
        #[structopt(help = "A list of IDs", min_values = 1)]
        subject_ids: Vec<i64>, 
    }, 

    NoteIds {
        #[structopt(help = "A list of notes IDs", min_values = 1)]
        note_ids: Vec<i64>, 
    }
}

#[derive(Debug, StructOpt)]
pub enum Command {
    #[structopt(about = "Initialize a profile.")]
    Init {
        #[structopt(short, long, help = "Initialize a profile without a shelf database.", parse(from_flag = std::ops::Not::not))]
        without_db: bool, 

        #[structopt(short, long, help = "Set the name of the profile.")]
        name: Option<String>,
    }, 

    #[structopt(about = "Add multiple subjects and notes in the database.")]
    Add {
        #[structopt(short, long, help = "Force to replace the resulting files in the filesystem.")]
        force: bool, 

        #[structopt(subcommand)]
        kind: Input, 

        #[structopt(short, long, parse(from_flag = std::ops::Not::not), help = "Exclude the item in the shelf database.")]
        exclude_to_db: bool, 

        #[structopt(short, long, help = "The program will panic if the files already exist.")]
        strict: bool, 

        #[structopt(short, long, help = "The name of the template to be used for creating the notes.")] 
        template: Option<String>, 

        #[structopt(short, long, help = "Includes a metadata file when creating the subjects in the filesystem.")]
        with_metadata: bool, 
    },

    #[structopt(about = "Remove multiple subjects and notes in the database.")]
    Remove {
        #[structopt(subcommand)]
        kind: FullInput, 
    },

    #[structopt(about = "Lists the subjects and its notes from the database.")]
    List {
        #[structopt(short, long, possible_values = &["id", "name"], help = "Sort the entries.")]
        sort: Option<String>, 

        #[structopt(short, long, help = "Sort the entries by the modification datetime in the filesystem. If this is set to true, the rest of the sorting options are ignored.")]
        date: bool, 

        #[structopt(short, long, help = "Reverse the list.")]
        reverse: bool,         
    },

    #[structopt(about = "Compile the notes.")]
    Compile {
        #[structopt(subcommand)]
        kind: FullInput,

        #[structopt(short, long, default_value = "4", help = "Creates a specified number of threads compiling in parallel.")]
        thread_count: i64, 
    },

    #[structopt(about = "Open a note.")]
    Open {
        #[structopt(help = "The ID of the note to be opened.")]
        id: i64, 

        #[structopt(short, long, value_name = "command", help = "Replaces the command with the given command string. Be sure to indicate the note with '{note}'.")]
        execute: Option<String>, 
    },
}

fn main() {
    let args = TextureNotes::from_args();

    match cli(args) {
        Ok(()) => (), 
        Err(e) => {
            let exit_code = <i32>::from(&e);

            match e {
                Error::DatabaseError(_) => println!("The shelf database has encountered an error."), 
                Error::InvalidProfileError(path) => println!("Profile at {:?} is not valid or nonexistent.\nMake sure to export it successfully.", path),
                Error::ProfileAlreadyExists(path) => println!("Profile at {:?} already exists.", path), 
                Error::ProcessError(exit) => println!("The child process has exit with status code {}", exit.code().unwrap()),
                Error::UnexportedShelfError(path) => println!("The shelf at {:?} is not exported.", path),
                Error::NoShelfDatabase(path) => println!("The shelf (at {:?}) of the profile does not have a database.", path), 
                Error::SerdeValueError(e) => println!("A Serde error occurred.\n{}", e), 
                Error::HandlebarsTemplateError(e) => println!("There's something wrong with the Handlebars template.\n{}", e), 
                Error::HandlebarsTemplateFileError(e) => println!("There's something wrong with the Handlebars template.\n{}", e), 
                Error::HandlebarsRenderError(e) => println!("An error has occurred while rendering the Handlebars template\n{}", e), 
                Error::IoError(e) => println!("An IO error has occurred while Texture Notes is running.\n{}", e),
                _ => println!("Unknown error."), 
            };
            
            process::exit(exit_code)
        }
    };
}

fn cli(args: TextureNotes) -> Result<(), texture_notes_v2::error::Error> {
    let mut path: PathBuf = match args.target {
        Some(p) => p, 
        None => PathBuf::new()
    };

    if !path.ends_with(TEXTURE_NOTES_DIR_NAME) {
        path.push(TEXTURE_NOTES_DIR_NAME);
    }

    match args.cmd {
        Command::Init { without_db, name } => {
            let mut profile_builder = texture_notes_v2::ProfileBuilder::new();
            profile_builder.path(path).use_db(without_db);

            if name.is_some() {
                let name = name.unwrap();

                profile_builder.name(name);
            }

            let mut profile = profile_builder.build()?;

            profile.export(&texture_notes_v2::shelf::ExportOptions::new())?;

            println!("Profile at {:?} successfully initialized.", profile.path());
        },
        Command::Add { kind, force, exclude_to_db, template, strict, with_metadata } => {
            let profile = texture_notes_v2::Profile::from(&path)?;
            let mut export_options = texture_notes_v2::shelf::ExportOptions::new();
            export_options.include_in_db(exclude_to_db).strict(strict).with_metadata(with_metadata);
            
            match kind {
                Input::Notes { subject, notes } => {
                    let subject = texture_notes_v2::notes::Subject::new(subject);
                    let notes = texture_notes_v2::notes::Note::from_vec_loose(&notes, &subject, profile.shelf())?;

                    let created_notes = profile.create_notes(&subject, &notes, &export_options, None)?;

                    println!("Here are the notes under the subject {:?} that successfully created in the shelf.", subject.name());
                    for note in created_notes {
                        println!("  - {:?}", note.title());
                    }
                },
                Input::Subjects { subjects } => {
                    let subjects = texture_notes_v2::notes::Subject::from_vec_loose(&subjects, profile.shelf())?;
                    let created_subjects = profile.create_subjects(&subjects, &export_options)?;

                    println!("Here are the subjects that have been successfully created in the shelf.");
                    for subject in created_subjects {
                        println!("  - {:?}", subject.name());
                    }
                }
            }
        },
        Command::Remove { kind } => {
            let profile = texture_notes_v2::Profile::from(&path)?;

            match kind {
                FullInput::NoteIds { note_ids } => {
                    let notes = profile.shelf().get_notes_by_id(&note_ids)?;
                }, 
                FullInput::Notes { subject, notes } => {
                    let subject = texture_notes_v2::notes::Subject::new(subject);
                    let notes = texture_notes_v2::notes::Note::from_vec_loose(&notes, &subject, profile.shelf())?;

                    profile.shelf().delete_notes(&subject, &notes)?;
                }, 
                FullInput::SubjectIds { subject_ids } => {
                    let subjects = profile.shelf().get_subjects_by_id(&subject_ids, None)?;
                    
                    profile.shelf().delete_subjects(&subjects)?;
                }, 
                FullInput::Subjects { subjects } => {
                    let subjects = texture_notes_v2::notes::Subject::from_vec(&subjects, profile.shelf())?;

                    for subject in subjects {
                        match subject {
                            Some(s) => profile.shelf().delete_subject(&s)?, 
                            None => continue, 
                        }
                    }
                }
            }
        },
        Command::List { sort, date, reverse } => {
            let profile = texture_notes_v2::Profile::from(&path)?;

            let sort = match sort.as_ref() {
                Some(s) => Some(String::as_str(s)), 
                None => None, 
            };

            let mut entries = profile.list_entries(sort, date)?;
            let shelf = profile.shelf();

            if reverse {
                entries.reverse();
            }

            for entry in entries {
                let subject = entry.0;
                let notes = entry.1;

                if !subject.is_path_exists(shelf) {
                    continue;
                }

                let subject_id = profile.shelf().get_subject_id(&subject)?.unwrap();

                println!("[{}] {}", subject_id, subject.name());

                for note in notes {
                    let note_id = profile.shelf().get_note_id(&subject, &note)?.unwrap();

                    if !note.is_path_exists(&subject, shelf) {
                        continue;
                    }

                    println!("  - ({}) {}", note_id, note.title());
                }
            }
        }, 
        Command::Compile { kind, thread_count } => {
            let profile = texture_notes_v2::Profile::from(&path)?;

            match kind {
                FullInput::Notes { subject, notes } => {
                    let subject = texture_notes_v2::notes::Subject::from(&subject, profile.shelf())?.unwrap();
                    let notes = texture_notes_v2::notes::Note::from_vec_loose(&notes, &subject, profile.shelf())?;
                    
                    let compiled_notes = profile.compile_notes_in_parallel(&subject, &notes, thread_count as i16)?;

                    if compiled_notes.len() == 0 {
                        println!("No notes successfully ran the compile command. Please check for the command if it's valid or the note exists in the filesystem.")
                    } else {
                        println!("Here are the compiled note that successfully run the compile command:");
                        for compiled_note in compiled_notes {
                            println!("  - {}", compiled_note.title());
                        }
                    }
                },
                FullInput::Subjects { subjects } => {
                    for subject in subjects.iter() {
                        let subject = texture_notes_v2::notes::Subject::from(&subject, profile.shelf())?.unwrap();
                        let notes = profile.shelf().get_notes_in_fs(&subject)?;
                        
                        let compiled_notes = profile.compile_notes_in_parallel(&subject, &notes, thread_count as i16)?;
    
                        if compiled_notes.len() == 0 {
                            println!("No notes successfully ran the compile command under subject {:?}. Please check for the command if it's valid or the note exists in the filesystem.", &subject.name())
                        } else {
                            println!("Here are the compiled notes under subject {:?} that successfully run the compile command:", &subject.name());
                            for compiled_note in compiled_notes {
                                println!("  - {}", compiled_note.title());
                            }
                        }
                    }
                }
                _ => println!("{:?} {}", kind, thread_count), 
            }
        }, 
        Command::Open { id, execute } => {
            println!("{} {:?}", id, execute);
        }, 
        _ => ()
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn basic_usage_test() {
        let command_args_as_vec = vec!["texture-notes-v2", "init"];
        let command_args = TextureNotes::from_iter(command_args_as_vec.iter());

        assert_eq!(cli(command_args).is_err(), true);
    }
}