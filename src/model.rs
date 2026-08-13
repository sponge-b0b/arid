use std::ops::Range;
use std::path::PathBuf;

pub type FileId = u32;
pub type LineId = u32;
pub type CorpusPos = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizationOptions {
    pub ignore_comments: bool,
    pub ignore_docstrings: bool,
    pub ignore_imports: bool,
    pub ignore_signatures: bool,
}

impl Default for NormalizationOptions {
    fn default() -> Self {
        Self {
            ignore_comments: true,
            ignore_docstrings: true,
            ignore_imports: true,
            ignore_signatures: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedFile {
    pub path: PathBuf,
    pub source: String,
    pub normalized: String,
    pub lines: Vec<NormalizedLine>,
    pub segments: Vec<NormalizedSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedLine {
    pub text_range: Range<u32>,
    pub source_line: u32,
    pub effective: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedSegment {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Occurrence {
    pub file: FileId,
    pub normalized_start: u32,
    pub normalized_len: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub effective_lines: u32,
    pub normalized_len: u32,
    pub occurrences: Vec<Occurrence>,
}
