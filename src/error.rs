//! Error and result types for the xyzcadrs pipeline.

use std::io;

/// Errors raised while rendering or exporting a field.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The seed search could not find any change of the field value within
    /// the search volume (or the evaluation budget was exhausted).
    #[error(
        "no solid found: the field returns the same value at every probed point \
         within [{min_val}, {max_val}] (evaluation budget: {budget} calls). \
         Check that the field actually contains material and that the search \
         volume covers it."
    )]
    NoSolidFound {
        /// Lower bound of the searched volume.
        min_val: f64,
        /// Upper bound of the searched volume.
        max_val: f64,
        /// Number of field evaluations allowed for the seed search.
        budget: usize,
    },

    /// A non-positive resolution was requested.
    #[error("resolution must be positive, got {0}")]
    InvalidResolution(f64),

    /// Wrapped I/O error while writing an output file.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, Error>;
