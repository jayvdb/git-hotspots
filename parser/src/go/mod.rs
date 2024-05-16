//! This module implements the parser for Go language.
use hotspots_discovery::{File, Lang};
use tree_sitter::{Language, Query};
use tree_sitter_go::language;

use super::{Element, Error};

/// This parser can parse any Go files.
pub struct GoParser {
    container: super::Container,
    query: Query,
}

impl GoParser {
    /// Creates a new Go parser. The container should have enough capacity or
    /// capable of growing to hold all the elements in the file.
    pub fn new(c: super::Container) -> Result<Self, Error> {
        let queries = crate::PROJECT_DIR
            .get_file("src/queries/go.scm")
            .ok_or(Error::FileNotFound("go.scm not found".to_owned()))?;
        let query = queries
            .contents_utf8()
            .ok_or(Error::ParseFile("Can't parse queries".to_owned()))?;
        let language = language();
        let query = Query::new(&language, query)?;

        Ok(GoParser {
            container: c,
            query,
        })
    }
}

impl super::Parser for GoParser {
    fn container(&mut self) -> &mut super::Container {
        &mut self.container
    }

    fn ro_container(&self) -> &super::Container {
        &self.container
    }

    fn supported(&self, f: &File) -> bool {
        f.lang == Lang::Go
    }

    fn language(&self) -> Language {
        language()
    }

    fn query(&self) -> &Query {
        &self.query
    }

    /// Returns a new vector with the representation names for functions. In
    /// case of go, we want to remove the receiver from the method name, and
    /// inform the caller that we removed one element from the vector.
    fn func_repr(&self, v: Vec<Element>) -> (Vec<Element>, usize) {
        // When the index is zero, it is the method receiver. We should keep the value
        // until the next element to concatinate.
        let mut prev: Option<String> = None;
        let mut redacted = 0usize;
        let res: Vec<Element> = v
            .into_iter()
            .filter_map(|mut e| {
                if e.index == 0 {
                    prev = Some(e.name);
                    redacted += 1;
                    return None;
                } else if let Some(p) = prev.as_ref() {
                    // Removing anything up to the first space, and prepend '(' to make the method
                    // look right.
                    let name = p.split(' ').nth(1).unwrap_or(&p[1..]);
                    let name = format!("({} {}", name, e.name);
                    prev = None;
                    e.name = name;
                }
                Some(e)
            })
            .collect();
        (res, redacted)
    }
}

#[cfg(test)]
mod tests;
