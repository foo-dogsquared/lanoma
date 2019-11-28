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
pub enum Command {
    #[structopt(about = "Add multiple subjects and notes in the database.")]
    Add {
        #[structopt(short, long, help = "Force to replace the resulting files in the filesystem.")]
        force: bool, 

        #[structopt(short, long, min_values = 2, value_names = &["subject", "note"], multiple = true, 
        help = "Add a list of notes for a particular subject. Requires the subject as the first item in the list.")]
        notes: Vec<String>, 

        #[structopt(short, long, help = "Add a list of subjects to be added.", min_values = 1)]
        subjects: Vec<String>, 
    },

    #[structopt(about = "Remove multiple subjects and notes in the database.")]
    Remove {
        #[structopt(short, long, help = "Remove the associated files in the filesystem.")]
        delete: bool, 

        #[structopt(short, long, min_values = 2, value_names = &["subject", "notes"], help = "Remove all of the notes of the listed subjects.")]
        notes: Vec<String>, 
        
        #[structopt(short, long, min_values = 1, help = "Remove the subjects in the database.")]
        subjects: Vec<String>, 

        #[structopt(short = "S", long, min_values = 1, help = "Remove the subjects quickly through its ID in the database.")]
        subjects_id: Vec<i64>, 

        #[structopt(short = "N", long, min_values = 1, help = "Remove the notes quickly through specifying the ID in the database.")]
        notes_id: Vec<i64>, 
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

        #[structopt(short, long, min_values = 1, value_name = "subject", help = "Compile all of the notes of the listed subjects.")]
        notes: Vec<String>, 

        #[structopt(short, long, min_values = 1, value_name = "subject", help = "Compile the main note of the listed subjects.")]
        main: Vec<String>, 

        #[structopt(help = "The ID of the notes to be compiled.")]
        notes_id: Vec<i64>, 

        #[structopt(help = "The ID of the subjects to be compiled.")]
        subjects_id: Vec<i64>, 
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
        Err(_) => match texture_notes_v2::Profile::new(&path) {
            Ok(p) => p, 
            Err(_) => process::exit(1), 
        }
    };

    match args.cmd {
        Command::Add { notes, subjects, force } => {
            let subjects = subjects.iter().map(| name | name.as_str()).collect();

            match profile.add_entries(&subjects, &vec![], true) {
                Ok(()) => (), 
                Err(_) => process::exit(1), 
            };
        },
        Command::Remove { subjects_id, subjects, notes_id, notes, delete } => {
            println!("{:?} {:?} {:?} {}", subjects_id, subjects, notes, delete);
        },
        Command::List { sort } => {
            profile.list_entries(None);
        }, 
        Command::Compile { subjects_id, notes_id, main, notes, cache } => {
            println!("{:?} {:?} {:?} {}", subjects_id, main, notes, cache);
        }, 
        Command::Open { id, execute } => {
            println!("{} {:?}", id, execute);
        }, 
        _ => ()
    }
}
