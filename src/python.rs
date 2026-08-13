use std::ops::Range;

use ruff_python_ast::statement_visitor::{self, StatementVisitor};
use ruff_python_ast::token::{TokenKind, Tokens};
use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};
use ruff_python_parser::parse_module;
use ruff_text_size::{Ranged, TextRange};
use thiserror::Error;

use crate::model::NormalizationOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SuppressionKind {
    Disable,
    Enable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SuppressionEvent {
    pub offset: usize,
    pub kind: SuppressionKind,
}

#[derive(Debug)]
pub(crate) struct PythonAnalysis {
    pub masks: Vec<Range<usize>>,
    pub suppressions: Vec<SuppressionEvent>,
}

#[derive(Debug, Error)]
#[error("invalid Python syntax: {message}")]
pub(crate) struct PythonError {
    message: String,
}

pub(crate) fn analyze(
    source: &str,
    options: NormalizationOptions,
) -> Result<PythonAnalysis, PythonError> {
    let parsed = parse_module(source).map_err(|error| PythonError {
        message: error.to_string(),
    })?;

    let tokens = parsed.tokens();
    let mut masks = Vec::new();
    let mut suppressions = Vec::new();

    collect_comments_and_suppressions(
        source,
        tokens,
        options.ignore_comments,
        &mut masks,
        &mut suppressions,
    );

    let mut collector = SyntaxMaskCollector {
        source,
        tokens,
        options,
        masks: &mut masks,
    };

    if options.ignore_docstrings {
        collector.collect_docstring(parsed.suite());
    }

    collector.visit_body(parsed.suite());

    merge_ranges(&mut masks);
    suppressions.sort_by_key(|event| event.offset);

    Ok(PythonAnalysis {
        masks,
        suppressions,
    })
}

fn collect_comments_and_suppressions(
    source: &str,
    tokens: &Tokens,
    ignore_comments: bool,
    masks: &mut Vec<Range<usize>>,
    suppressions: &mut Vec<SuppressionEvent>,
) {
    for token in tokens {
        if token.kind() != TokenKind::Comment {
            continue;
        }

        let range = to_range(token.range());
        let text = &source[range.clone()];
        let directive = text.strip_prefix('#').map(str::trim);

        let suppression = match directive {
            Some("arid: disable") => Some(SuppressionKind::Disable),
            Some("arid: enable") => Some(SuppressionKind::Enable),
            _ => None,
        };

        if let Some(kind) = suppression {
            suppressions.push(SuppressionEvent {
                offset: range.start,
                kind,
            });
        }

        // Arid directives never participate in duplicate matching, even if
        // ordinary comments are configured to remain significant.
        if ignore_comments || suppression.is_some() {
            masks.push(range);
        }
    }
}

struct SyntaxMaskCollector<'a, 'm> {
    source: &'a str,
    tokens: &'a Tokens,
    options: NormalizationOptions,
    masks: &'m mut Vec<Range<usize>>,
}

impl SyntaxMaskCollector<'_, '_> {
    fn collect_docstring(&mut self, body: &[Stmt]) {
        let Some(Stmt::Expr(statement)) = body.first() else {
            return;
        };

        if matches!(statement.value.as_ref(), Expr::StringLiteral(_)) {
            let range = self.extend_statement_range(statement.range());
            self.masks.push(range);
        }
    }

    fn collect_signature(&mut self, function: &StmtFunctionDef) {
        let mut nesting = 0_u32;

        for token in self.tokens.after(function.start()) {
            if token.start() >= function.end() {
                break;
            }

            match token.kind() {
                TokenKind::Lpar | TokenKind::Lsqb | TokenKind::Lbrace => {
                    nesting += 1;
                }
                TokenKind::Rpar | TokenKind::Rsqb | TokenKind::Rbrace => {
                    nesting = nesting.saturating_sub(1);
                }
                TokenKind::Colon if nesting == 0 => {
                    self.masks
                        .push(function.start().to_usize()..token.end().to_usize());
                    return;
                }
                _ => {}
            }
        }
    }

    fn extend_statement_range(&self, range: TextRange) -> Range<usize> {
        let mut start = range.start();
        let mut end = range.end();

        // Prefer consuming a following semicolon:
        //
        //     import os; run()
        //
        // becomes:
        //
        //     run()
        if let Some(next) = self.tokens.after(end).first()
            && next.kind() == TokenKind::Semi
            && !contains_line_break(self.source, end.to_usize(), next.start().to_usize())
        {
            end = next.end();
            return start.to_usize()..end.to_usize();
        }

        // If the removed statement is last on the logical line, consume the
        // preceding semicolon instead:
        //
        //     run(); import os
        //
        // becomes:
        //
        //     run()
        if let Some(previous) = self.tokens.before(start).last()
            && previous.kind() == TokenKind::Semi
            && !contains_line_break(self.source, previous.end().to_usize(), start.to_usize())
        {
            start = previous.start();
        }

        start.to_usize()..end.to_usize()
    }
}

impl<'a> StatementVisitor<'a> for SyntaxMaskCollector<'_, '_> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(function) => {
                if self.options.ignore_signatures {
                    self.collect_signature(function);
                }

                if self.options.ignore_docstrings {
                    self.collect_docstring(&function.body);
                }
            }

            Stmt::ClassDef(class) => {
                if self.options.ignore_docstrings {
                    self.collect_docstring(&class.body);
                }
            }

            Stmt::Import(import) if self.options.ignore_imports => {
                let range = self.extend_statement_range(import.range());
                self.masks.push(range);
            }

            Stmt::ImportFrom(import) if self.options.ignore_imports => {
                let range = self.extend_statement_range(import.range());
                self.masks.push(range);
            }

            _ => {}
        }

        statement_visitor::walk_stmt(self, stmt);
    }
}

fn contains_line_break(source: &str, start: usize, end: usize) -> bool {
    source[start..end]
        .bytes()
        .any(|byte| matches!(byte, b'\n' | b'\r'))
}

fn to_range(range: TextRange) -> Range<usize> {
    range.start().to_usize()..range.end().to_usize()
}

fn merge_ranges(ranges: &mut Vec<Range<usize>>) {
    ranges.sort_by_key(|range| (range.start, range.end));

    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());

    for range in ranges.drain(..) {
        if let Some(last) = merged.last_mut()
            && range.start <= last.end
        {
            last.end = last.end.max(range.end);
            continue;
        }

        merged.push(range);
    }

    *ranges = merged;
}
