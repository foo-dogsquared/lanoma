pub mod string {
    use regex::Regex;
    
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
}

pub mod filesystem {
    use std::fs::{ self, DirBuilder };
    use std::os;
    use std::path::{ self, Path, PathBuf, Component };

    use crate::error::Error;

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

    #[cfg(target_family = "unix")]
    /// Create a symlink pointing from the source. 
    pub fn create_symlink<P: AsRef<Path>, Q: AsRef<Path>> (src: P, dst: Q) -> Result<(), Error> {
        let src = src.as_ref();
        let dst = dst.as_ref();
        
        os::unix::fs::symlink(src, dst).map_err(Error::IoError)
    }

    #[cfg(target_family = "windows")]
    /// Create a symlink pointing from the source. 
    pub fn create_symlink<P: AsRef<Path>, Q: AsRef<Path>> (src: P, dst: Q) -> Result<(), Error> {
        let from = src.as_ref();
        let to = dst.as_ref();

        let result = match from.is_dir() {
            true => os::windows::fs::symlink_dir(from, to), 
            false => os::windows::fs::symlink_file(from, to),
        };

        result.map_err(Error::IoError)
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
        base: Q
    ) -> Option<PathBuf> {
        let base = base.as_ref();
        let dst = dst.as_ref();

        // checking if both of them are the same type of filepaths
        if base.is_absolute() != dst.is_absolute() {
            match dst.is_absolute() {
                true => Some(PathBuf::from(dst)), 
                false => None
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
                    },
                    
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

    /// Normalize the given path. 
    /// Unlike the standard library `std::fs::canonicalize` function, it does not need the file to be in the filesystem. 
    /// 
    /// That said, this leaves compromise the implementation to be very naive. 
    /// All resulting path will be based on the current directory. 
    /// 
    /// If the resulting normalized path is empty, 
    pub fn naively_normalize_path<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
        let path = path.as_ref();

        let mut normalized_components = vec![];

        for component in path.components() {
            match &component {
                Component::CurDir => continue, 
                Component::ParentDir => { normalized_components.pop(); () }, 
                _ => normalized_components.push(component),
            }
        }

        let mut normalized_path = PathBuf::new();
        for component in normalized_components {
            normalized_path.push(component.as_os_str());
        }

        match normalized_path.to_string_lossy().is_empty() {
            true => None, 
            false => Some(normalized_path), 
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*; 
    use std::path::{ Path, PathBuf };

    #[test]
    fn title_case_to_kebab_case() {
        {
            let test_case = String::from("The Quick Brown Fox Jumps Over The Lazy Dog.");
            let result = string::kebab_case(&test_case);

            assert_eq!(result, "the-quick-brown-fox-jumps-over-the-lazy-dog");
        };

        {
            let test_case = String::from("The---Quick---Brown Fox Jumps Over---The---Lazy Dog.");
            let result = string::kebab_case(&test_case);

            assert_eq!(result, "the-quick-brown-fox-jumps-over-the-lazy-dog");
        };

        {
            let test_case = String::from("The Quick Brown Fox: [It Jumps Over The Lazy Dog].");
            let result = string::kebab_case(&test_case);

            assert_eq!(result, "the-quick-brown-fox-it-jumps-over-the-lazy-dog");
        };
    }

    #[test]
    fn basic_regex_match() {
        {
            let test_case = String::from(r"^\w+$");
            let result = string::regex_match(&test_case.to_string(), "browing");

            assert_eq!(result, true);
        }
    }

    #[test]
    fn relative_path_from_test() {
        {
            let base = PathBuf::from("./tests/texture-notes-profile/notes/calculus");
            let dst = PathBuf::from("./tests/texture-notes-profile/common");

            assert_eq!(filesystem::relative_path_from(dst, base), Some("../../common".into()));
        }

        // testing if both input are of different filesystems
        {
            let base = PathBuf::from("/dev/sda/calculus-drive");
            let dst = PathBuf::from("./tests/texture-notes-profile/common");

            assert_eq!(filesystem::relative_path_from(dst, base), None);
        }

        {
            let base = PathBuf::from("/dev/sda/calculus-drive");
            let dst = PathBuf::from("/tests/texture-notes-profile/common");

            assert_eq!(filesystem::relative_path_from(dst.clone(), base), Some("../../../tests/texture-notes-profile/common".into()));
        }
    }
}