use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use crate::util::{impl_deref_wrapped, impl_from_repeated};

use spotipi_core::FileId;

use spotipi_protocol as protocol;
use protocol::metadata::VideoFile as VideoFileMessage;

#[derive(Debug, Clone, Default)]
pub struct VideoFiles(pub Vec<FileId>);

impl_deref_wrapped!(VideoFiles, Vec<FileId>);

impl_from_repeated!(VideoFileMessage, VideoFiles);
