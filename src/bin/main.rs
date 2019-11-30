use std::env;
use std::process;

// this is where the setup for the program will happen 
// and where the setup for the hook scripts
use structopt::StructOpt; 

use std::path::{ PathBuf };

extern crate texture_notes_v2;

// Search for a pattern in a file and display the lines that contain it.
#[derive(Debug, StructOpt)]
#[structopt(name = "Texture Notes", about = "Manage your LaTeX study notes.")]
pub struct TextureNotes {
    #[structopt(short, long, parse(from_os_str), value_name = "path", help = "Sets the target directory to be used.")]
    target: Option<PathBuf>,

    #[structopt(short, long, parse(from_os_str), value_name = "path", help = "Sets the custom config file to be used.")]
    config: Option<PathBuf>, 

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
        #[structopt(min_values = 2, value_names = &["subject", "notes"], multiple = true, 
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
        #[structopt(min_values = 2, value_names = &["subject", "note"], multiple = true, 
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
    Init, 

    #[structopt(about = "Add multiple subjects and notes in the database.")]
    Add {
        #[structopt(short, long, help = "Force to replace the resulting files in the filesystem.")]
        force: bool, 

        #[structopt(subcommand)]
        kind: Input, 
    },

    #[structopt(about = "Remove multiple subjects and notes in the database.")]
    Remove {
        #[structopt(short, long, help = "Remove the associated files in the filesystem.")]
        delete: bool, 

        #[structopt(subcommand)]
        kind: FullInput, 
    },

    #[structopt(about = "Lists the subjects and its notes from the database.")]
    List {
        #[structopt(short, long, possible_values = &["id", "title", "date"], help = "Sort the entries.")]
        sort: Option<String>, 
    },

    #[structopt(about = "Compile the notes.")]
    Compile {
        #[structopt(short, long, help = "Indicates if the build directory should be kept in instead of being deleted.")]
        cache: bool, 

        #[structopt(short, long, min_values = 1, value_name = "subject", help = "Compile the main note of the listed subjects.")]
        main: Vec<String>, 

        #[structopt(subcommand)]
        kind: FullInput
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

    let mut path: PathBuf = match args.target {
        Some(p) => p, 
        None => PathBuf::from("./")
    };

    let mut profile = match texture_notes_v2::Profile::open(&path) {
        Ok(p) => p, 
        Err(_) => match texture_notes_v2::Profile::init(&path, true) {
            Ok(p) => p, 
            Err(_) => process::exit(1), 
        }
    };

    match args.cmd {
        Command::Add { kind, force } => {
            println!("{:?}", kind);
        },
        Command::Remove { kind, delete } => {
            println!("{:?} {}", kind, delete);
        },
        Command::List { sort } => {
            profile.list_entries(None);
        }, 
        Command::Compile { kind, main, cache } => {
            println!("{:?} {:?} {}", kind, main, cache);
        }, 
        Command::Open { id, execute } => {
            println!("{} {:?}", id, execute);
        }, 
        _ => ()
    }
}
