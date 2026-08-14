use std::ops::Range;

use ruff_python_ast::statement_visitor::{self, StatementVisitor};
use ruff_python_ast::token::{TokenKind, Tokens};
use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};
use ruff_python_parser::parse_module;
use ruff_text_size::{Ranged, TextRange};
use thiserror::Error;

use crate::model::{NormalizationOptions, StructuralContext, StructuralScope};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StructuralRegion {
    pub range: Range<usize>,
    pub context: StructuralContext,
    pub scope: StructuralScope,
}

#[derive(Debug)]
pub(crate) struct PythonAnalysis {
    pub masks: Vec<Range<usize>>,
    pub suppressions: Vec<SuppressionEvent>,
    pub structural_regions: Vec<StructuralRegion>,
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
    let mut structural_regions = Vec::new();

    collect_comments_and_suppressions(
        source,
        tokens,
        options.ignore_comments,
        &mut masks,
        &mut suppressions,
    );

    let mut collector = SyntaxCollector {
        source,
        tokens,
        options,
        masks: &mut masks,
        structural_regions: &mut structural_regions,
        scope: StructuralScope::Module,
        nesting_depth: 0,
    };

    if options.ignore_docstrings {
        collector.collect_docstring(parsed.suite());
    }

    collector.visit_body(parsed.suite());

    merge_ranges(&mut masks);
    suppressions.sort_by_key(|event| event.offset);
    structural_regions.sort_by_key(|region| (region.range.start, region.range.end));

    Ok(PythonAnalysis {
        masks,
        suppressions,
        structural_regions,
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

struct SyntaxCollector<'a, 'm> {
    source: &'a str,
    tokens: &'a Tokens,
    options: NormalizationOptions,
    masks: &'m mut Vec<Range<usize>>,
    structural_regions: &'m mut Vec<StructuralRegion>,
    scope: StructuralScope,
    nesting_depth: u32,
}

impl SyntaxCollector<'_, '_> {
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
        let mut signature_start = None;

        for token in self.tokens.after(function.start()) {
            if token.start() >= function.end() {
                break;
            }

            match token.kind() {
                TokenKind::Async if nesting == 0 && signature_start.is_none() => {
                    signature_start = Some(token.start());
                }

                TokenKind::Def if nesting == 0 => {
                    signature_start.get_or_insert(token.start());
                }

                TokenKind::Lpar | TokenKind::Lsqb | TokenKind::Lbrace => {
                    nesting += 1;
                }

                TokenKind::Rpar | TokenKind::Rsqb | TokenKind::Rbrace => {
                    nesting = nesting.saturating_sub(1);
                }

                TokenKind::Colon if nesting == 0 => {
                    let Some(start) = signature_start else {
                        continue;
                    };

                    self.masks.push(start.to_usize()..token.end().to_usize());
                    return;
                }

                _ => {}
            }
        }
    }

    fn collect_structural_region(&mut self, stmt: &Stmt) {
        let (context, scope) = match stmt {
            Stmt::FunctionDef(_) => (StructuralContext::Declarative, StructuralScope::Function),
            Stmt::ClassDef(_) => (StructuralContext::Declarative, StructuralScope::Class),

            Stmt::TypeAlias(_)
            | Stmt::Assign(_)
            | Stmt::AnnAssign(_)
            | Stmt::Import(_)
            | Stmt::ImportFrom(_)
                if self.scope != StructuralScope::Function && self.nesting_depth == 0 =>
            {
                (StructuralContext::Declarative, self.scope)
            }

            Stmt::Return(_)
            | Stmt::Delete(_)
            | Stmt::TypeAlias(_)
            | Stmt::Assign(_)
            | Stmt::AugAssign(_)
            | Stmt::AnnAssign(_)
            | Stmt::For(_)
            | Stmt::While(_)
            | Stmt::If(_)
            | Stmt::With(_)
            | Stmt::Match(_)
            | Stmt::Raise(_)
            | Stmt::Try(_)
            | Stmt::Assert(_)
            | Stmt::Import(_)
            | Stmt::ImportFrom(_)
            | Stmt::Global(_)
            | Stmt::Nonlocal(_)
            | Stmt::Expr(_)
            | Stmt::Pass(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::IpyEscapeCommand(_) => (StructuralContext::Executable, self.scope),
        };

        self.structural_regions.push(StructuralRegion {
            range: to_range(stmt.range()),
            context,
            scope,
        });
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

impl<'a> StatementVisitor<'a> for SyntaxCollector<'_, '_> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        self.collect_structural_region(stmt);

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

        let previous_scope = self.scope;
        let previous_nesting_depth = self.nesting_depth;

        match stmt {
            Stmt::FunctionDef(_) => {
                self.scope = StructuralScope::Function;
                self.nesting_depth = 0;
            }
            Stmt::ClassDef(_) => {
                self.scope = StructuralScope::Class;
                self.nesting_depth = 0;
            }
            _ => {
                self.nesting_depth = self.nesting_depth.saturating_add(1);
            }
        }

        statement_visitor::walk_stmt(self, stmt);

        self.scope = previous_scope;
        self.nesting_depth = previous_nesting_depth;
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
