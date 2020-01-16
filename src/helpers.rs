use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::process;

use toml;

use lanoma_lib::error::Error;
use lanoma_lib::masternote::MasterNote;
use lanoma_lib::note::Note;
use lanoma_lib::profile::Profile;
use lanoma_lib::shelf::{Shelf, ShelfData};
use lanoma_lib::subjects::Subject;
use lanoma_lib::Object;
#[macro_use]
use lanoma_lib::{modify_toml_table};

pub fn master_note_full_object(
    profile: &Profile,
    shelf: &Shelf,
    master_note: &MasterNote,
) -> toml::Value {
    let subject_as_toml = ShelfData::data(master_note.subject(), &shelf);
    let master_note_as_toml = ShelfData::data(master_note, &shelf);
    let profile_config = Object::data(profile);
    let shelf_data = Object::data(shelf);

    let mut metadata = toml::Value::from(HashMap::<String, toml::Value>::new());
    modify_toml_table! {metadata,
        ("profile", profile_config),
        ("subject", subject_as_toml),
        ("master", master_note_as_toml),
        ("shelf", shelf_data)
    }

    metadata
}

pub fn note_full_object(
    profile: &Profile,
    shelf: &Shelf,
    note: &Note,
    subject: &Subject,
) -> toml::Value {
    let subject_toml = ShelfData::data(subject, &shelf);
    let note_toml = ShelfData::data(note, (&subject, &shelf));
    let profile_config = Object::data(profile);
    let shelf_data = Object::data(shelf);

    // The metadata is guaranteed to be valid since the codebase enforces it to be valid either at creation
    // or at retrieval from a folder.
    // It is safe to call `unwrap` from here.
    let mut metadata = toml::Value::from(HashMap::<String, toml::Value>::new());
    modify_toml_table! {metadata,
        ("profile", profile_config),
        ("subject", subject_toml),
        ("note", note_toml),
        ("shelf", shelf_data)
    };

    metadata
}

/// A generic function for writing a shelf item (as a file).
pub fn write_file<P, S>(
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
