use std::fs::{DirBuilder};
use std::path::{self, Component, Path, PathBuf};

use crate::error::Error;

pub fn create_folder<P: AsRef<Path>>(
    dir_builder: &DirBuilder,
    path: P,
) -> Result<(), Error> {
    let path = path.as_ref();
    let path_str = match path.to_str() {
        Some(text) => text,
        None => return Err(Error::ValueError),
    };

    match dir_builder.create(path_str) {
        Err(reason) => return Err(Error::IoError(reason)),
        _ => Ok(()),
    }
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

/// Normalize the given path.
/// Unlike the standard library `std::fs::canonicalize` function, it does not need the file to be in the filesystem.
///
/// That said, this leaves compromise the implementation to be very naive.
/// All resulting path will be based on the current directory.
///
/// If the resulting normalized path is empty, it will return `None`.
pub fn naively_normalize_path<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
    let path = path.as_ref();

    let mut normalized_components = vec![];

    for component in path.components() {
        match &component {
            Component::CurDir => continue,
            // The condition below can be safe to execute.
            // It will immediately continue to the if block if one of them is true which is why
            // the ordering of the conditions is important.
            // If the vector is empty, it will never reach the second condition.
            // That said, there has to be a better way than this.
            Component::ParentDir => match normalized_components.is_empty()
                || is_parent_dir(normalized_components[normalized_components.len() - 1])
            {
                true => normalized_components.push(component),
                false => {
                    normalized_components.pop();
                    ()
                }
            },
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

fn is_parent_dir(component: Component) -> bool {
    match component {
        Component::ParentDir => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn relpath_to_common_relpaths() {
        let base = PathBuf::from("./tests/lanoma-profile/notes/calculus");
        let dst = PathBuf::from("./tests/lanoma-profile/common");

        assert_eq!(relative_path_from(dst, base), Some("../../common".into()));
    }

    #[test]
    fn relpath_to_leading_common_relpaths() {
        let base = PathBuf::from("./tests/lanoma-profile/common");
        let dst = PathBuf::from("./tests/lanoma-profile/common/calculus");

        assert_eq!(relative_path_from(dst, base), Some("calculus".into()));
    }

    #[test]
    fn relpath_to_the_same_input() {
        let base = PathBuf::from("./tests/lanoma-profile/common");
        let dst = PathBuf::from("./tests/lanoma-profile/common");

        assert_eq!(relative_path_from(dst, base), Some("".into()));
    }

    #[test]
    fn relpath_with_dst_parent_dir() {
        let base = PathBuf::from("./");
        let dst = PathBuf::from("../rust");

        assert_eq!(relative_path_from(dst, base), Some("../rust".into()));
    }

    #[test]
    fn relpath_with_base_parent_dir() {
        let base = PathBuf::from("../rust");
        let dst = PathBuf::from("./");

        assert_eq!(relative_path_from(dst, base), None);
    }

    #[test]
    fn relpath_with_common_parent_dir() {
        let base = PathBuf::from("../rust/");
        let dst = PathBuf::from("../rust/././bin");

        assert_eq!(relative_path_from(dst, base), Some("bin".into()));
    }

    #[test]
    fn relpath_with_common_parent_dirs() {
        let base = PathBuf::from("../rust/../../../");
        let dst = PathBuf::from("../rust");

        assert_eq!(relative_path_from(dst, base), Some("../../..".into()));
    }

    #[cfg(unix)]
    #[test]
    fn relpath_from_root_to_current_dir() {
        let base = PathBuf::from("/dev/sda/calculus-drive");
        let dst = PathBuf::from("./tests/lanoma-profile/common");

        assert_eq!(relative_path_from(dst, base), None);
    }

    #[cfg(windows)]
    #[test]
    fn relpath_from_root_to_current_dir() {
        let base = PathBuf::from("C:\\Windows");
        let dst = PathBuf::from("./tests");

        assert_eq!(relative_path_from(dst, base), None);
    }

    #[cfg(windows)]
    #[test]
    fn relpath_to_common_root() {
        let base = PathBuf::from("C:\\dev\\sda\\calculus-drive");
        let dst = PathBuf::from("C:\\tests\\lanoma-profile\\common");

        assert_eq!(
            relative_path_from(dst.clone(), base),
            Some("../../../tests/lanoma-profile/common".into())
        );
    }

    #[cfg(unix)]
    #[test]
    fn relpath_to_common_root() {
        let base = PathBuf::from("/dev/sda/calculus-drive");
        let dst = PathBuf::from("/tests/lanoma-profile/common");

        assert_eq!(
            relative_path_from(dst, base),
            Some("../../../tests/lanoma-profile/common".into())
        );
    }

    #[test]
    fn leading_current_dir_naive_normalized() {
        let test_case = PathBuf::from("./tests/lanoma-profile/notes/calculus");

        assert_eq!(
            naively_normalize_path(test_case),
            Some("tests/lanoma-profile/notes/calculus".into())
        );
    }

    #[test]
    fn parent_dirs_naively_normalized() {
        let test_case = PathBuf::from("../case/..");

        assert_eq!(naively_normalize_path(test_case), Some("..".into()));
    }

    #[test]
    fn multiple_parent_dirs_naively_normalized() {
        let test_case = PathBuf::from("../case/../tests/../../../of");

        assert_eq!(
            naively_normalize_path(test_case),
            Some("../../../of".into())
        );
    }

    #[test]
    fn leading_current_dir_with_parent_dirs_normalized() {
        {
            let test_case = PathBuf::from("./tests/../calculus/calculus-i/../");

            assert_eq!(naively_normalize_path(test_case), Some("calculus".into()));
        }
    }

    #[test]
    fn leading_current_dir_with_space_normalized() {
        let test_case = PathBuf::from("./Calculus/Calculus I");

        assert_eq!(
            naively_normalize_path(test_case),
            Some("Calculus/Calculus I".into())
        );
    }

    #[test]
    fn leading_current_dir_with_multiple_parent_dir_normalized() {
        let test_case = PathBuf::from("./Calculus/../Calculus I/../../p");

        assert_eq!(naively_normalize_path(test_case), Some("../p".into()));
    }
}
