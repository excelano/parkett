//! Parkett: reading a Parquet file for a window that shows it.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

mod blocks;

/// The messages Parkett shows, in the language the desktop asks for.
pub mod i18n {
    pub use potext::fill;

    potext::catalog!();
}
pub mod copy;
pub mod export;
mod footer;
mod staging;
mod table;
pub mod values;

pub use blocks::Blocks;
pub use footer::{Chunk, Column, Field, Footer, RowGroup};
pub use table::{BLOCK_ROWS, BlockId, Table};
