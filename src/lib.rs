//! Parkett: reading a Parquet file for a window that shows it.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

mod blocks;
pub mod copy;
mod footer;
mod table;
pub mod values;

pub use blocks::Blocks;
pub use footer::{Chunk, Column, Field, Footer, RowGroup};
pub use table::{BLOCK_ROWS, BlockId, Table};
