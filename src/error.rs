use std::error;
use std::fmt;
use std::io;

use rusqlite;

// An enum for errors possible to happen in the libtexturenotes
#[derive(Debug)]
pub enum Error {
    // Erro when the value is invalid 
    ValueError, 

    // Error when the profile is not valid or does not exists
    ProfileInvalidityError, 

    // Error when the database failed to open 
    DatabaseError(rusqlite::Error),

    // Error when the file is missing or nonexistent 
    IoError(io::Error), 

    // Error when a part of the profile data is missing
    MissingDataError(String), 
}

impl error::Error for Error { }

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::ValueError => write!(f, "Given value is not valid."), 
            Error::ProfileInvalidityError => write!(f, "Profile location is not valid."), 
            Error::DatabaseError(ref err) => err.fmt(f), 
            Error::IoError(ref err) => err.fmt(f), 
            Error::MissingDataError(ref p) => write!(f, "{} is missing.", p),
        }
    }
}