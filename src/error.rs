use std::convert::From;
use std::error;
use std::fmt;
use std::io;
use std::path;
use std::process;

use globwalk;
use handlebars;
use toml;

/// An enum for errors possible to happen in the Texture Notes library.
#[derive(Debug)]
pub enum Error {
    /// Error when the value is invalid in a function.
    ValueError,

    /// Error when the profile is not valid or does not exists
    InvalidProfileError(path::PathBuf),

    /// Given when the operation requires the profile to be nonexistent.
    ProfileAlreadyExists(path::PathBuf),

    /// Given when the shelf operation requires the shelf to be nonexistent in the filesystem.
    ShelfAlreadyExists(path::PathBuf),

    /// Used when the shelf is not yet exported while attempting to do some filesystem operations.
    UnexportedShelfError(path::PathBuf),

    /// Used when the associated subject is not valid (i.e., no metadata file or the required key/s).
    InvalidSubjectError(path::PathBuf),

    /// IO-related errors mainly given by the official standard library IO library.  
    IoError(io::Error),

    /// Given when a shell process has gone something wrong.
    ProcessError(process::ExitStatus),

    /// Error when a part of the profile data is missing.
    MissingDataError(String),

    /// Related errors for the TOML library.
    TomlValueError(toml::de::Error),
    TomlSerializeError(toml::ser::Error),

    /// Related errors for Handlebars.
    HandlebarsTemplateError(handlebars::TemplateError),
    HandlebarsTemplateFileError(handlebars::TemplateFileError),
    HandlebarsRenderError(handlebars::RenderError),

    /// Given when the glob pattern is not recognizable.
    GlobParsingError(globwalk::GlobError),
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match *self {
            Error::ValueError => write!(f, "Given value is not valid."),
            Error::InvalidProfileError(ref path) => {
                write!(f, "Profile at '{}' is not valid.", path.to_string_lossy())
            }
            Error::ProfileAlreadyExists(ref path) => {
                write!(f, "Profile at '{}' already exists.", path.to_string_lossy())
            }
            Error::ShelfAlreadyExists(ref path) => write!(
                f,
                "The shelf at path '{}' already exists.",
                path.to_string_lossy()
            ),
            Error::UnexportedShelfError(ref path) => write!(
                f,
                "The shelf at path '{}' is not yet exported in the filesystem.",
                path.to_str().unwrap()
            ),
            Error::InvalidSubjectError(ref path) => write!(
                f,
                "The subject at path '{}' is invalid.",
                path.to_string_lossy()
            ),
            Error::ProcessError(ref _exit) => write!(f, "The process is not successful."),
            Error::IoError(ref err) => err.fmt(f),
            Error::MissingDataError(ref p) => write!(f, "{} is missing.", p),
            Error::TomlValueError(ref p) => write!(f, "{} is invalid.", p),
            Error::TomlSerializeError(ref p) => write!(f, "{}", p),
            Error::HandlebarsTemplateError(ref p) => write!(f, "{} is an invalid template.", p),
            Error::HandlebarsTemplateFileError(ref p) => write!(
                f,
                "Handlebars with the instance '{}' has an error occurred.",
                p
            ),
            Error::HandlebarsRenderError(ref p) => {
                write!(f, "{}: Error occurred while rendering.", p)
            }
            Error::GlobParsingError(ref error) => error.fmt(f),
        }
    }
}
