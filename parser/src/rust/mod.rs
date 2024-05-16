//! This module implements the parser for Rust language.
use hotspots_discovery::{File, Lang};
use tree_sitter::{Language, Query};
use tree_sitter_rust::language;

use super::Error;

/// This parser can parse any Rust files.
pub struct RustParser {
    container: super::Container,
    query: Query,
}

impl RustParser {
    /// Creates a new Rust parser. The container should have enough capacity or
    /// capable of growing to hold all the elements in the file.
    pub fn new(c: super::Container) -> Result<Self, Error> {
        let queries = crate::PROJECT_DIR
            .get_file("src/queries/rust.scm")
            .ok_or(Error::FileNotFound("rust.scm not found".to_owned()))?;
        let query = queries
            .contents_utf8()
            .ok_or(Error::ParseFile("Can't parse queries".to_owned()))?;
        let language = language();
        let query = Query::new(&language, query)?;

        Ok(RustParser {
            query,
            container: c,
        })
    }
}

impl super::Parser for RustParser {
    fn container(&mut self) -> &mut super::Container {
        &mut self.container
    }

    fn ro_container(&self) -> &super::Container {
        &self.container
    }

    fn supported(&self, f: &File) -> bool {
        f.lang == Lang::Rust
    }

    fn language(&self) -> Language {
        language()
    }

    fn query(&self) -> &Query {
        &self.query
    }
}

#[cfg(test)]
mod tests;
