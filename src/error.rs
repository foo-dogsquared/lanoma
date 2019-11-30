use std::error;
use std::fmt;
use std::io;
use std::path;

use rusqlite;
use serde_json;
use handlebars;

use crate::notes;

// An enum for errors possible to happen in the libtexturenotes
#[derive(Debug)]
pub enum Error {
    /// Error when the value is invalid in a function. 
    ValueError, 

    /// Error when the profile is not valid or does not exists
    ProfileInvalidityError, 

    /// Used when the shelf has no database while attempting to do some database operations. 
    NoShelfDatabase(path::PathBuf), 

    /// Used when the shelf is not yet exported while attempting to do some filesystem operations. 
    UnexportedShelfError(path::PathBuf), 

    /// Used when the associated subject is missing in the shelf (either in the database or the filesystem). 
    MissingSubjectError(path::PathBuf), 

    /// Related errors to Rusqlite.  
    DatabaseError(rusqlite::Error),

    // IO-related errors.  
    IoError(io::Error), 

    // Error when a part of the profile data is missing.
    MissingDataError(String), 

    // Related errors for Serde.
    SerdeValueError(serde_json::Error), 

    // Related errors for Handlebars.
    HandlebarsTemplateError(handlebars::TemplateError), 
    HandlebarsRenderError(handlebars::RenderError), 
}

impl error::Error for Error { }

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::ValueError => write!(f, "Given value is not valid."), 
            Error::ProfileInvalidityError => write!(f, "Profile location is not valid."), 
            Error::NoShelfDatabase(ref path) => write!(f, "The shelf at path '{}' has no database for the operations.", path.to_str().unwrap()), 
            Error::UnexportedShelfError(ref path) => write!(f, "The shelf at path '{}' is not yet exported in the filesystem.", path.to_str().unwrap()), 
            Error::MissingSubjectError(ref path) => write!(f, "The subject at path '{}' is missing", path.to_string_lossy()),
            Error::DatabaseError(ref err) => err.fmt(f), 
            Error::IoError(ref err) => err.fmt(f), 
            Error::MissingDataError(ref p) => write!(f, "{} is missing.", p),
            Error::SerdeValueError(ref p) => write!(f, "{} is invalid.", p),
            Error::HandlebarsTemplateError(ref p) => write!(f, "{} is an invalid template.", p),
            Error::HandlebarsRenderError(ref p) => write!(f, "{}: Error occurred while rendering.", p),
        }
    }
}