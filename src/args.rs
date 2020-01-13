use std::path::PathBuf;
use structopt::StructOpt;

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
    pub shelf: Option<PathBuf>,

    #[structopt(
        short,
        long,
        parse(from_os_str),
        value_name = "path",
        help = "Searches for the profile in the specified directory. By default, it searches in the default configuration folder of the filesystem (e.g., '%USERPROFILE%\\AppData\\Roaming' for Windows, '$HOME/.config/' for Linux)."
    )]
    pub profile: Option<PathBuf>,

    #[structopt(subcommand)]
    pub cmd: Command,
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
