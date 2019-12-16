use std::error;
use std::fmt;
use std::io;
use std::path;
use std::process;
use std::convert::From;

use rusqlite;
use r2d2;
use serde_json;
use handlebars;
use globwalk;

/// An enum for errors possible to happen in the Texture Notes library. 
#[derive(Debug)]
pub enum Error {
    /// Error when the value is invalid in a function. 
    ValueError, 

    /// Error when the profile is not valid or does not exists
    InvalidProfileError(path::PathBuf), 

    /// Given when the operation requires the profile to be nonexistent. 
    ProfileAlreadyExists(path::PathBuf), 

    /// Used when the shelf has no database while attempting to do some database operations. 
    NoShelfDatabase(path::PathBuf), 

    /// Given when the shelf operation requires the shelf to be nonexistent in the filesystem. 
    ShelfAlreadyExists(path::PathBuf), 

    /// Used when the shelf is not yet exported while attempting to do some filesystem operations. 
    UnexportedShelfError(path::PathBuf), 

    /// Used when the associated subject is missing in the shelf database. 
    DanglingSubjectError(path::PathBuf), 

    /// Related errors to Rusqlite.  
    DatabaseError(rusqlite::Error),

    /// IO-related errors mainly given by the official standard library IO library.  
    IoError(io::Error), 

    /// Given when a shell process has gone something wrong. 
    ProcessError(process::ExitStatus),

    /// Error when a part of the profile data is missing.
    MissingDataError(String), 

    /// Related errors for Serde.
    SerdeValueError(serde_json::Error), 

    /// Related errors for Handlebars.
    HandlebarsTemplateError(handlebars::TemplateError), 
    HandlebarsTemplateFileError(handlebars::TemplateFileError), 
    HandlebarsRenderError(handlebars::RenderError), 

    /// Related erros for r2d2. 
    R2D2Error(r2d2::Error), 

    /// Given when the glob pattern is not recognizable. 
    GlobParsingError(globwalk::GlobError), 
}

impl From<&Error> for i32 {
    fn from(error: &Error) -> Self {
        match error {
            Error::ValueError => 1, 
            Error::InvalidProfileError (_) => 2, 
            Error::ProfileAlreadyExists (_) => 3, 
            Error::NoShelfDatabase (_) => 4, 
            Error::UnexportedShelfError (_) => 10,
            Error::ShelfAlreadyExists (_) => 11, 
            Error::DanglingSubjectError (_) => 12, 
            Error::DatabaseError (_) => 12, 
            Error::R2D2Error (_) => 20, 
            Error::IoError (_) => 21, 
            Error::ProcessError (_) => 22, 
            Error::SerdeValueError (_) => 23, 
            Error::HandlebarsRenderError (_) => 24, 
            Error::HandlebarsTemplateError (_) => 24, 
            Error::HandlebarsTemplateFileError (_) => 24, 
            Error::GlobParsingError (_) => 50, 
            _ => -1
        }
    }
}

impl error::Error for Error { }

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::ValueError => write!(f, "Given value is not valid."), 
            Error::InvalidProfileError(ref path) => write!(f, "Profile at '{}' is not valid.", path.to_string_lossy()), 
            Error::ProfileAlreadyExists(ref path) => write!(f, "Profile at '{}' already exists.", path.to_string_lossy()), 
            Error::NoShelfDatabase(ref path) => write!(f, "The shelf at path '{}' has no database for the operations.", path.to_str().unwrap()), 
            Error::ShelfAlreadyExists(ref path) => write!(f, "The shelf at path '{}' already exists.", path.to_string_lossy()), 
            Error::UnexportedShelfError(ref path) => write!(f, "The shelf at path '{}' is not yet exported in the filesystem.", path.to_str().unwrap()), 
            Error::DanglingSubjectError(ref path) => write!(f, "The subject at path '{}' is missing", path.to_string_lossy()),
            Error::DatabaseError(ref err) => err.fmt(f), 
            Error::ProcessError(ref _exit) => write!(f, "The process is not successful."), 
            Error::IoError(ref err) => err.fmt(f), 
            Error::MissingDataError(ref p) => write!(f, "{} is missing.", p),
            Error::SerdeValueError(ref p) => write!(f, "{} is invalid.", p),
            Error::HandlebarsTemplateError(ref p) => write!(f, "{} is an invalid template.", p),
            Error::HandlebarsTemplateFileError(ref p) => write!(f, "Handlebars with the instance '{}' has an error occurred.", p),
            Error::HandlebarsRenderError(ref p) => write!(f, "{}: Error occurred while rendering.", p),
            Error::R2D2Error(ref error) => error.fmt(f), 
            Error::GlobParsingError(ref error) => error.fmt(f), 
        }
    }
}