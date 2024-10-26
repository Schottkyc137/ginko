use line_index::TextRange;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

#[derive(Clone, Default, Debug)]
pub struct CyclicDependencyEntry {
    path: PathBuf,
    location: TextRange,
}

impl CyclicDependencyEntry {
    pub fn new(path: PathBuf, location: TextRange) -> CyclicDependencyEntry {
        CyclicDependencyEntry { path, location }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn location(&self) -> TextRange {
        self.location
    }
}

impl Hash for CyclicDependencyEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state)
    }
}

impl PartialEq for CyclicDependencyEntry {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for CyclicDependencyEntry {}
