//! This module provides machinery for collecting and reporting diagnostics that
//! arise during site generation.
//!
//! It builds heavily on the `miette` crate.

use miette::{Diagnostic, IntoDiagnostic};
use thiserror::Error;
use tracing::debug;

pub struct DiagnosticContext {
    diagnostics: Vec<miette::Report>,
    any_errors: bool,
}

impl DiagnosticContext {
    pub fn with<T, E, R>(f: impl FnOnce(&mut Self) -> R) -> Result<T, ErrorSet>
    where
        R: IntoDiagnostic<T, E>,
    {
        let mut this = Self {
            diagnostics: Vec::new(),
            any_errors: false,
        };

        match f(&mut this).into_diagnostic() {
            Ok(value) => {
                if this.any_errors {
                    return Err(ErrorSet {
                        errors: this.diagnostics,
                    });
                };
                if !this.diagnostics.is_empty() {
                    debug!("generating report for {} warnings", this.diagnostics.len());
                    let warnings = WarningSet {
                        warnings: this.diagnostics,
                    };
                    let warnings = miette::Report::new(warnings);
                    eprintln!("{:?}", warnings);
                }
                Ok(value)
            }
            Err(error) => {
                this.record_report(error);
                Err(ErrorSet {
                    errors: this.diagnostics,
                })
            }
        }
    }

    // FIXME: this method should be pulled into a trait so I can implement it
    // for RenderContext as well.

    pub fn record(&mut self, diagnostic: impl Diagnostic + Send + Sync + 'static) {
        self.record_report(miette::Report::new(diagnostic))
    }

    fn record_report(&mut self, report: miette::Report) {
        debug!("recording diagnostic: {}", report);
        if <_ as AsRef<(dyn Diagnostic + 'static)>>::as_ref(&report)
            .severity()
            .unwrap_or(miette::Severity::Error)
            >= miette::Severity::Error
        {
            self.any_errors = true;
        }

        self.diagnostics.push(report);
    }
}

/// A collection of errors and warnings that occured while running code under a
/// [`DiagnosticContext`].
#[derive(Diagnostic, Error, Debug)]
#[error("Errors and warnings")]
#[diagnostic(severity(error))]
pub struct ErrorSet {
    #[related]
    errors: Vec<miette::Report>,
}

impl ErrorSet {
    pub fn iter(&self) -> impl Iterator<Item = &miette::Report> {
        self.errors.iter()
    }
}

#[derive(Diagnostic, Error, Debug)]
#[error("Warnings")]
#[diagnostic(severity(warning))]
struct WarningSet {
    #[related]
    warnings: Vec<miette::Report>,
}
