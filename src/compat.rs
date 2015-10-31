use std::fs::metadata;
use std::path::Path;

pub trait PathExt {
    fn exists(&self) -> bool;
    fn is_dir(&self) -> bool;
}

impl PathExt for Path {
    fn exists(&self) -> bool { metadata(self).is_ok() }
    fn is_dir(&self) -> bool { metadata(self).map(|s| s.is_dir()).unwrap_or(false) }
}
