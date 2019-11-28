extern crate regex;

use std::fs::{ self, DirBuilder };
use std::path::{ PathBuf, Path };
use regex::Regex;

use crate::error::Error;

pub fn kebab_case(string: &str) -> String {
    // Saving memory by compiling the regexes only once throughout the program 
    // with the use of the `lazy_static` crate
    lazy_static! {
        static ref WHITESPACE_CHARACTERS: Regex = Regex::new(r"\s+|-+").unwrap();
        static ref INVALID_CHARACTERS: Regex = Regex::new(r"[^A-Za-z0-9]+").unwrap();
    }

    // TODO: Optimize this. This is horrible btw
    // This is the implementation derived from v1
    // There has to be a better way
    let words: Vec<&str> = WHITESPACE_CHARACTERS.split(&string).collect();
    let mut filtered_words: Vec<String> = Vec::new();

    for word in words.iter() {
        if word.is_empty() {
            continue;
        }

        let filtered_word: String = INVALID_CHARACTERS.replace(word, "").to_lowercase();
        
        if filtered_word.is_empty() {
            continue;
        }

        filtered_words.push(filtered_word);
    }

    filtered_words.join("-")
}

pub fn regex_match(regex_str: &str, value: &str) -> bool {
    let compiled_regex: Regex = Regex::new(regex_str).unwrap();
    
    compiled_regex.is_match(value)
}

pub fn create_folder<P: AsRef<Path>>(dir_builder: &DirBuilder, path: P) -> Result<(), Error> {
    let path = path.as_ref();
    let path_str = match path.to_str() {
        Some(text) => text, 
        None => return Err(Error::ValueError), 
    };
    
    match dir_builder.create(path_str) {
        Err(reason) => return Err(Error::IoError(reason)), 
        _ => Ok(())
    }
}

/// Move folder from the specified locations. 
/// When a safety string is provided, the destination folder will be renamed first before moving the source folder. 
/// The name of the already existing destination will be appended with the safety string. 
pub fn move_folder<T: AsRef<Path>, U: AsRef<Path>>(from: T, to: U, safety_string: Option<&str>) -> Result<(), Error> {
    let from = from.as_ref();
    let to = to.as_ref();

    if to.is_dir() && safety_string.is_some() {
        if let Some(safety_string) = safety_string {
            let mut replacement_path: PathBuf = to.clone().to_path_buf();
            replacement_path.push(&format!("-{}", safety_string));

            fs::rename(&from, &replacement_path).map_err(Error::IoError)?;
        }
    }

    match fs::rename(&from, &to) {
        Ok(_v) => Ok(()), 
        Err(err) => Err(Error::IoError(err)) 
    }
}

pub fn read_file_or_default<'str, T: AsRef<Path>, U: AsRef<&'str str>>(path: T, default_value: U) -> String {
    let path = path.as_ref();
    let default_value = default_value.as_ref();

    match fs::read_to_string(path) {
        Ok(string) => string, 
        Err(_err) => default_value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*; 

    #[test]
    fn title_case_to_kebab_case() {
        {
            let test_case = String::from("The Quick Brown Fox Jumps Over The Lazy Dog.");
            let result = kebab_case(&test_case);

            assert_eq!(result, "the-quick-brown-fox-jumps-over-the-lazy-dog");
        };

        {
            let test_case = String::from("The---Quick---Brown Fox Jumps Over---The---Lazy Dog.");
            let result = kebab_case(&test_case);

            assert_eq!(result, "the-quick-brown-fox-jumps-over-the-lazy-dog");
        };

        {
            let test_case = String::from("The Quick Brown Fox: [It Jumps Over The Lazy Dog].");
            let result = kebab_case(&test_case);

            assert_eq!(result, "the-quick-brown-fox-it-jumps-over-the-lazy-dog");
        };
    }

    #[test]
    fn basic_regex_match() {
        {
            let test_case = String::from(r"^\w+$");
            let result = regex_match(&test_case.to_string(), "browing");

            assert_eq!(result, true);
        }
    }
}