//! A library for encoding/decoding Unix archive files.
//!
//! This library provides utilities necessary to manage [Unix archive
//! files](https://en.wikipedia.org/wiki/Ar_(Unix)) (as generated by the
//! standard `ar` command line utility) abstracted over a reader or writer.
//! This library provides a streaming interface that avoids having to ever load
//! a full archive entry into memory.
//!
//! The API of this crate is meant to be similar to that of the
//! [`tar`](https://crates.io/crates/tar) crate.
//!
//! # Format variants
//!
//! Unix archive files come in several variants, of which three are the most
//! common:
//!
//! * The *common variant*, used for Debian package (`.deb`) files among other
//!   things, which only supports filenames up to 16 characters.
//! * The *BSD variant*, used by the `ar` utility on BSD systems (including Mac
//!   OS X), which is backwards-compatible with the common variant, but extends
//!   it to support longer filenames and filenames containing spaces.
//! * The *GNU variant*, used by the `ar` utility on GNU and many other systems
//!   (including Windows), which is similar to the common format, but which
//!   stores filenames in a slightly different, incompatible way, and has its
//!   own strategy for supporting long filenames.
//!
//! This crate supports reading and writing all three of these variants.
//!
//! # Example usage
//!
//! Writing an archive:
//!
//! ```no_run
//! use ar::Builder;
//! use std::collections::BTreeMap;
//! use std::fs::File;
//! // Create a new archive that will be written to foo.a:
//! let mut builder = Builder::new(File::create("foo.a").unwrap(), BTreeMap::new()).unwrap();
//! // Add foo/bar.txt to the archive, under the name "bar.txt":
//! builder.append_path("foo/bar.txt").unwrap();
//! // Add foo/baz.txt to the archive, under the name "hello.txt":
//! let mut file = File::open("foo/baz.txt").unwrap();
//! builder.append_file(b"hello.txt", &mut file).unwrap();
//! ```
//!
//! Reading an archive:
//!
//! ```no_run
//! use ar::Archive;
//! use std::fs::File;
//! use std::io;
//! use std::str;
//! // Read an archive from the file foo.a:
//! let mut archive = Archive::new(File::open("foo.a").unwrap());
//! // Iterate over all entries in the archive:
//! while let Some(entry_result) = archive.next_entry() {
//!     let mut entry = entry_result.unwrap();
//!     // Create a new file with the same name as the archive entry:
//!     let mut file = File::create(
//!         str::from_utf8(entry.header().identifier()).unwrap(),
//!     ).unwrap();
//!     // The Entry object also acts as an io::Read, so we can easily copy the
//!     // contents of the archive entry into the file:
//!     io::copy(&mut entry, &mut file).unwrap();
//! }
//! ```

#![warn(missing_docs)]

mod read;
mod write;

pub use read::{Archive, Entry, SymbolTableEntry, Symbols};
pub use write::{Builder, GnuBuilder};

use std::fs::Metadata;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

// ========================================================================= //

const GLOBAL_HEADER_LEN: usize = 8;
const GLOBAL_HEADER: &'static [u8; GLOBAL_HEADER_LEN] = b"!<arch>\n";

const ENTRY_HEADER_LEN: usize = 60;

const BSD_SYMBOL_LOOKUP_TABLE_ID: &str = "__.SYMDEF";
const BSD_SORTED_SYMBOL_LOOKUP_TABLE_ID: &str = "__.SYMDEF SORTED";

const GNU_NAME_TABLE_ID: &str = "//";
const GNU_SYMBOL_LOOKUP_TABLE_ID: &str = "/";

// ========================================================================= //

/// Variants of the Unix archive format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Variant {
    /// Used by Debian package files; allows only short filenames.
    Common,
    /// Used by BSD `ar` (and OS X); backwards-compatible with common variant.
    BSD,
    /// Used by GNU `ar` (and Windows); incompatible with common variant.
    GNU,
}

// ========================================================================= //

/// Representation of an archive entry header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Header {
    identifier: Vec<u8>,
    mtime: u64,
    uid: u32,
    gid: u32,
    mode: u32,
    size: u64,
}

impl Header {
    /// Creates a header with the given file identifier and size, and all
    /// other fields set to zero.
    pub fn new(identifier: Vec<u8>, size: u64) -> Header {
        Header {
            identifier,
            mtime: 0,
            uid: 0,
            gid: 0,
            mode: 0,
            size,
        }
    }

    /// Creates a header with the given file identifier and all other fields
    /// set from the given filesystem metadata.
    #[cfg(unix)]
    pub fn from_metadata(identifier: Vec<u8>, meta: &Metadata) -> Header {
        Header {
            identifier,
            mtime: meta.mtime() as u64,
            uid: meta.uid(),
            gid: meta.gid(),
            mode: meta.mode(),
            size: meta.len(),
        }
    }

    #[cfg(not(unix))]
    pub fn from_metadata(identifier: Vec<u8>, meta: &Metadata) -> Header {
        Header::new(identifier, meta.len())
    }

    /// Returns the file identifier.
    pub fn identifier(&self) -> &[u8] { &self.identifier }

    /// Sets the file identifier.
    pub fn set_identifier(&mut self, identifier: Vec<u8>) {
        self.identifier = identifier;
    }

    /// Returns the last modification time in Unix time format.
    pub fn mtime(&self) -> u64 { self.mtime }

    /// Sets the last modification time in Unix time format.
    pub fn set_mtime(&mut self, mtime: u64) { self.mtime = mtime; }

    /// Returns the value of the owner's user ID field.
    pub fn uid(&self) -> u32 { self.uid }

    /// Sets the value of the owner's user ID field.
    pub fn set_uid(&mut self, uid: u32) { self.uid = uid; }

    /// Returns the value of the group's user ID field.
    pub fn gid(&self) -> u32 { self.gid }

    /// Returns the value of the group's user ID field.
    pub fn set_gid(&mut self, gid: u32) { self.gid = gid; }

    /// Returns the mode bits for this file.
    pub fn mode(&self) -> u32 { self.mode }

    /// Sets the mode bits for this file.
    pub fn set_mode(&mut self, mode: u32) { self.mode = mode; }

    /// Returns the length of the file, in bytes.
    pub fn size(&self) -> u64 { self.size }

    /// Sets the length of the file, in bytes.
    pub fn set_size(&mut self, size: u64) { self.size = size; }
}
// ========================================================================= //
