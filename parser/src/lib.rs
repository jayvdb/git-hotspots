//! This crate parses supported languages with their tree-sitter grammers, and
//! collects all the methods and functions in the files.
//!
//! All tree-sitter queries are stored in the `queries` directory.
//! All fixtures for testing are stored in the `fixtures` directory.
#![warn(missing_docs)]
pub mod go;
pub mod lua;
pub mod rust;

use std::io::{BufReader, Read};
use std::ops::Not;
use std::str::Utf8Error;
use std::time::Instant;
use std::{fs, io};

use hotspots_discovery::File;
use include_dir::{include_dir, Dir};
use indicatif::ProgressBar;
use log::{debug, warn};
use thiserror::Error as TError;
use tree_sitter::{Language, LanguageError, Parser as TSParser, Query, QueryCursor, QueryMatch};

static PROJECT_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR");

/// Error enumerates all errors for this application.
#[derive(TError, Debug)]
pub enum Error {
    /// Returned when we can't get the repository information.
    #[error("File is not compatible with this parser")]
    NotCompatible,

    /// Returned when no files were added to the parser.
    #[error("No files have added")]
    NoFilesAdded,

    /// Returned when there is a query error.
    #[error(transparent)]
    TSQuery(#[from] tree_sitter::QueryError),

    /// Returned when we can't parse a file.
    #[error("Can't parse file: {0}")]
    ParseFile(String),

    /// Returned when the file is not found.
    #[error("File not found: {0}")]
    FileNotFound(String),

    /// Returned when we can't open a file.
    #[error(transparent)]
    OpenFile(#[from] io::Error),

    /// Returned when the file contains invalid UTF-8 characters.
    #[error(transparent)]
    Utf8Str(#[from] Utf8Error),

    /// Returned when tree-sitter can't set the given language.
    #[error(transparent)]
    Language(#[from] LanguageError),
}

/// Element represents a function or a method in a file.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Element {
    /// Name of the function or method.
    pub name: String,
    /// File where the function or method is located.
    pub file: String,
    /// Line where the function or method is located.
    pub line: usize,
    index: u32,
}

/// Container holds the files and filters for the Parser.
#[derive(Debug, PartialEq, Eq)]
pub struct Container {
    files: Vec<File>,
    filters: Vec<String>,
}

impl Container {
    /// Returns a new Container with the given capacity.
    pub fn new(cap: usize) -> Self {
        Container {
            files: Vec::with_capacity(cap),
            filters: Vec::with_capacity(cap),
        }
    }
}

fn collect_matches<'a>(
    matches: impl Iterator<Item = QueryMatch<'a, 'a>>,
    source: &'a str,
) -> Vec<(usize, u32, &'a str)> {
    matches
        .filter_map(|m| {
            m.captures.iter().find_map(|capture| {
                if let Ok(line) = capture.node.utf8_text(source.as_bytes()) {
                    Some((
                        capture.node.range().start_point.row + 1,
                        capture.index,
                        line,
                    ))
                } else {
                    None
                }
            })
        })
        .collect()
}

/// Parser provides the functionalities necessary for finding tree-sitter Nodes
/// from a list of given files.
pub trait Parser {
    /// Returns a mutable reference to the container.
    fn container(&mut self) -> &mut Container;

    /// Returns a reference to the container.
    fn ro_container(&self) -> &Container;

    /// Returns true if the file is compatible with the Parser.
    fn supported(&self, f: &File) -> bool;

    /// Will return an error if the file is not compatible with the Parser.
    fn add_file(&mut self, f: File) -> Result<(), Error> {
        if !self.supported(&f) {
            return Err(Error::NotCompatible);
        }
        self.container().files.push(f);
        Ok(())
    }

    /// Returns a tree-sitter language.
    fn language(&self) -> Language;

    /// Returns a tree-sitter Query object for the Parser's language.
    fn query(&self) -> &Query;

    /// Returns a mutable reference to the given files. It returns and error if
    /// the file can't be read.
    fn files(&self) -> Result<&[File], Error> {
        let files = self.ro_container().files.as_slice();
        if files.is_empty() {
            Err(Error::NoFilesAdded)
        } else {
            Ok(files)
        }
    }

    /// Adds the filter for excluding functions.
    fn filter_name(&mut self, s: String) {
        self.container().filters.push(s);
    }

    /// Applies the filter on function names.
    fn filter(&self, p: &str) -> bool {
        self.ro_container().filters.iter().any(|s| p.contains(s))
    }

    /// Returns a new vector with the representation names for functions.
    fn func_repr(&self, v: Vec<Element>) -> (Vec<Element>, usize) {
        (v, 0)
    }

    /// Returns all the functions in all files. It returns and error if the file
    /// can't be read, or the language parser can't parse the contents.
    fn find_functions(&mut self, pb: &ProgressBar) -> Result<Vec<Element>, Error> {
        let mut parser = TSParser::new();
        let language = self.language();
        parser.set_language(&language)?;
        let files = self.files()?;
        let query = self.query();
        let mut ret: Vec<Element> = Vec::with_capacity(files.len());

        let start = Instant::now();
        for file in files {
            let file_handle = fs::File::open(&file.path)?;
            let mut reader = BufReader::new(file_handle);
            let mut source_code = String::new();
            if let Err(err) = reader.read_to_string(&mut source_code) {
                warn!("error while reading {}: {err}", file.path.clone());
                continue;
            };
            let tree = match parser.parse(&source_code, None) {
                Some(tree) => tree,
                None => {
                    warn!("error while parsing {}", file.path.clone());
                    continue;
                },
            };

            let mut cursor = QueryCursor::new();

            let matches = cursor.matches(query, tree.root_node(), source_code.as_bytes());
            let res = collect_matches(matches, &source_code);
            ret.append(
                &mut res
                    .into_iter()
                    .map(|(line, index, name)| {
                        pb.inc_length(1);
                        Element {
                            name: name.to_owned(),
                            file: file.path.clone(),
                            line,
                            index,
                        }
                    })
                    .collect::<Vec<Element>>(),
            );
        }

        debug!("Finding function took {:?}", start.elapsed());

        let (res, mut redacted) = self.func_repr(ret);
        let len = res.len();
        let res = res
            .into_iter()
            .filter(|e| self.filter(&e.name).not())
            .collect::<Vec<Element>>();
        redacted += len - res.len();
        if let Some(len) = pb.length() {
            // To support tests.
            pb.set_length(len - redacted as u64);
        }
        Ok(res)
    }
}
