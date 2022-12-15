use std::path::Path;

use super::Type;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Index {
    pub path: Box<Path>,
    pub r#type: Type,
    pub size: u64,
}
