#![warn(missing_docs)]
#![doc=include_str!("../README.md")]

#[macro_use]
extern crate log;

use spotipi_core as core;
use spotipi_playback as playback;
use spotipi_protocol as protocol;

mod context_resolver;
mod model;
mod shuffle_vec;
mod spirc;
mod state;

pub use model::*;
pub use spirc::*;
pub use state::*;
