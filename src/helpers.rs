use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{self, Path, PathBuf};
use std::process;

use toml;

use lanoma_lib::config::SubjectConfig;
use lanoma_lib::error::Error;
use lanoma_lib::masternote::MasterNote;
use lanoma_lib::modify_toml_table;
use lanoma_lib::note::Note;
use lanoma_lib::profile::Profile;
use lanoma_lib::shelf::{Shelf, ShelfData};
use lanoma_lib::subjects::Subject;
use lanoma_lib::Object;

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

pub fn create_master_note_from_subject_str(
    subject: &str,
    shelf: &Shelf,
) -> Result<MasterNote, Error> {
    let subject = Subject::from_shelf(subject, &shelf)?;
    let subject_config = subject.get_config(&shelf).unwrap_or(SubjectConfig::new());
    let notes = subject.get_notes_in_fs(&subject_config.files, &shelf)?;

    // Creating the master note instance and initializing the values.
    let mut master_note = MasterNote::new(subject);
    for note in notes {
        master_note.push(&note);
    }

    Ok(master_note)
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

/// Get the relative path from two paths similar to Python `os.path.relpath`.
///
/// This does not check whether the path exists in the filesystem.
///
/// Furthermore, this code is adapted from the [`pathdiff`](https://github.com/Manishearth/pathdiff/blob/master/src/lib.rs) crate
/// which in turn adapted from the `rustc` code at
/// https://github.com/rust-lang/rust/blob/e1d0de82cc40b666b88d4a6d2c9dcbc81d7ed27f/src/librustc_back/rpath.rs .
pub fn relative_path_from<P: AsRef<Path>, Q: AsRef<Path>>(
    dst: P,
    base: Q,
) -> Option<PathBuf> {
    let base = base.as_ref();
    let dst = dst.as_ref();

    // checking if both of them are the same type of filepaths
    if base.is_absolute() != dst.is_absolute() {
        match dst.is_absolute() {
            true => Some(PathBuf::from(dst)),
            false => None,
        }
    } else {
        let mut dst_components = dst.components();
        let mut base_path_components = base.components();

        let mut common_components: Vec<path::Component> = vec![];

        // looping into each components
        loop {
            match (dst_components.next(), base_path_components.next()) {
                // if both path are now empty
                (None, None) => break,

                // if the dst path has more components
                (Some(c), None) => {
                    common_components.push(c);
                    common_components.extend(dst_components.by_ref());
                    break;
                }

                // if the base path has more components
                (None, _) => common_components.push(path::Component::ParentDir),
                (Some(a), Some(b)) if common_components.is_empty() && a == b => (),
                (Some(a), Some(b)) if b == path::Component::CurDir => common_components.push(a),
                (Some(_), Some(b)) if b == path::Component::ParentDir => return None,
                (Some(a), Some(_)) => {
                    common_components.push(path::Component::ParentDir);
                    for _ in base_path_components {
                        common_components.push(path::Component::ParentDir);
                    }
                    common_components.push(a);
                    common_components.extend(dst_components.by_ref());
                    break;
                }
            }
        }

        Some(common_components.iter().map(|c| c.as_os_str()).collect())
    }
}
