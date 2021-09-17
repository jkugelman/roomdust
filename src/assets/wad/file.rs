use std::collections::HashMap;
use std::convert::TryInto;

use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use byteorder::{LittleEndian, ReadBytesExt};
use lazy_static::lazy_static;
use regex::Regex;

use super::wad::{self, Lump, ResultExt};

/// A single IWAD or PWAD file stored in a [`Wad`] stack.
///
/// [`Wad`]: crate::wad::Wad
#[derive(Debug)]
pub(super) struct WadFile {
    path: PathBuf,
    kind: WadKind,
    lumps: Vec<Lump>,
    lump_indices: HashMap<String, Vec<usize>>,
}

#[derive(Debug)]
struct Header {
    pub kind: WadKind,
    pub lump_count: usize,
    pub directory_offset: u64,
}

/// WAD files can be either IWADs or PWADs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WadKind {
    /// An IWAD or "internal wad" such as `doom.wad` that contains all of the data necessary to
    /// play.
    Iwad,
    /// A PWAD or "patch wad" containing extra levels, textures, or other assets that are overlaid
    /// on top of other wads.
    Pwad,
}

#[derive(Debug)]
struct Directory {
    pub lump_locations: Vec<LumpLocation>,
}

#[derive(Debug)]
struct LumpLocation {
    pub offset: u64,
    pub size: usize,
    pub name: String,
}

impl fmt::Display for LumpLocation {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(
            fmt,
            "{} (offset {}, size {})",
            self.name, self.offset, self.size
        )
    }
}

lazy_static! {
    static ref LUMP_NAME_REGEX: Regex = Regex::new(r"^[A-Z0-9\[\]\-_\\]+$").unwrap();
}

impl WadFile {
    /// Reads a WAD file from disk.
    pub fn open(path: impl AsRef<Path>) -> wad::Result<Self> {
        Self::open_impl(path.as_ref())
    }

    fn open_impl(path: &Path) -> wad::Result<Self> {
        let file = File::open(path).err_path(path)?;
        let mut file = BufReader::new(file);

        let Header {
            kind,
            lump_count,
            directory_offset,
        } = Self::read_header(path, &mut file)?;

        let Directory { lump_locations } =
            Self::read_directory(path, &mut file, lump_count, directory_offset)?;

        let mut wad_file = WadFile {
            path: path.to_owned(),
            kind,
            lumps: Vec::new(),
            lump_indices: HashMap::new(),
        };
        wad_file.build_indices(&lump_locations);
        wad_file.read_lumps(path, &mut file, lump_locations)?;

        Ok(wad_file)
    }

    fn read_header(path: &Path, mut file: impl Read + Seek) -> wad::Result<Header> {
        file.seek(SeekFrom::Start(0)).err_path(path)?;

        let kind = Self::read_kind(path, &mut file)?;
        let lump_count = file.read_u32::<LittleEndian>().err_path(path)?;
        let directory_offset = file.read_u32::<LittleEndian>().err_path(path)?;

        Ok(Header {
            kind,
            lump_count: lump_count.try_into().unwrap(),
            directory_offset: directory_offset.try_into().unwrap(),
        })
    }

    fn read_kind(path: &Path, file: impl Read) -> wad::Result<WadKind> {
        let mut buffer = Vec::new();
        file.take(4).read_to_end(&mut buffer).err_path(path)?;

        match &buffer[..] {
            b"IWAD" => Ok(WadKind::Iwad),
            b"PWAD" => Ok(WadKind::Pwad),
            _ => Err(wad::Error::malformed(path, "not a WAD file")),
        }
    }

    fn read_directory(
        path: &Path,
        mut file: impl Read + Seek,
        lump_count: usize,
        offset: u64,
    ) -> wad::Result<Directory> {
        file.seek(SeekFrom::Start(offset.into())).err_path(path)?;

        // The WAD is untrusted so clamp how much memory is pre-allocated. For comparison,
        // `doom.wad` has 1,264 lumps and `doom2.wad` has 2,919.
        let mut lump_locations = Vec::with_capacity(lump_count.clamp(0, 4096));

        for _ in 0..lump_count {
            let offset = file.read_u32::<LittleEndian>().err_path(path)?;
            let size = file.read_u32::<LittleEndian>().err_path(path)?;
            let mut name = [0u8; 8];
            file.read_exact(&mut name).err_path(path)?;

            // Strip trailing NULs and convert into a `String`. Stay away from `str::from_utf8` so
            // we don't have to deal with UTF-8 decoding errors.
            let name = name
                .iter()
                .take_while(|&&b| b != 0u8)
                .map(|&b| b as char)
                .collect::<String>();

            // Verify that the lump name is all uppercase, digits, and a handful of acceptable
            // symbols.
            if !LUMP_NAME_REGEX.is_match(&name) {
                return Err(wad::Error::malformed(
                    path,
                    &format!("illegal lump name {:?}", name),
                ));
            }

            lump_locations.push(LumpLocation {
                offset: offset.into(),
                size: size.try_into().unwrap(),
                name,
            });
        }

        Ok(Directory { lump_locations })
    }

    fn build_indices(&mut self, locations: &[LumpLocation]) {
        for (index, location) in locations.iter().enumerate() {
            self.lump_indices
                .entry(location.name.clone())
                .and_modify(|indices: &mut Vec<usize>| indices.push(index))
                .or_insert(vec![index]);
        }
    }

    fn read_lumps(
        &mut self,
        path: &Path,
        mut file: impl Read + Seek,
        locations: Vec<LumpLocation>,
    ) -> wad::Result<()> {
        for location in locations {
            let LumpLocation { offset, size, name } = location;

            // The WAD is untrusted so clamp how much memory is pre-allocated. For comparison,
            // `doom.wad` has a 68,168 byte `WIMAP0`, and `killer.wad` has a 95,1000 `SIDEDEFS`.
            let mut data = Vec::with_capacity(size.clamp(0, 65_536));

            file.seek(SeekFrom::Start(offset.into())).err_path(path)?;
            file.by_ref()
                .take(size.try_into().unwrap())
                .read_to_end(&mut data)
                .err_path(path)?;

            if data.len() < size {
                return Err(wad::Error::malformed(
                    path,
                    &format!("{} outside of file", LumpLocation { offset, size, name }),
                ));
            }
            assert!(data.len() == size);

            self.lumps.push(Lump { name, data });
        }

        Ok(())
    }

    /// The file's path on disk.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns whether this is an IWAD or PWAD.
    pub fn kind(&self) -> WadKind {
        self.kind
    }

    /// Checks that the file is the correct kind.
    pub fn expect(self, expected: WadKind) -> wad::Result<Self> {
        if self.kind() == expected {
            Ok(self)
        } else {
            Err(wad::Error::WrongType {
                path: self.path().to_owned(),
                expected,
            })
        }
    }

    /// Retrieves a unique lump by name.
    ///
    /// It is an error if the lump is missing.
    pub fn lump(&self, name: &str) -> wad::Result<&Lump> {
        self.try_lump(name)?
            .ok_or_else(|| self.error(&format!("{} missing", name)))
    }

    /// Retrieves a unique lump by name.
    ///
    /// Returns `Ok(None)` if the lump is missing.
    pub fn try_lump(&self, name: &str) -> wad::Result<Option<&Lump>> {
        let index = self.try_lump_index(name)?;
        if index.is_none() {
            return Ok(None);
        }
        let index = index.unwrap();

        Ok(Some(&self.lumps[index]))
    }

    /// Retrieves a block of `size > 0` lumps following a unique named marker. The marker lump is
    /// included in the result.
    ///
    /// It is an error if the block is missing.
    ///
    /// # Panics
    ///
    /// Panics if `size == 0`.
    pub fn lumps_following(&self, start: &str, size: usize) -> wad::Result<&[Lump]> {
        self.try_lumps_following(start, size)?
            .ok_or_else(|| self.error(&format!("{} missing", start)))
    }

    /// Retrieves a block of `size > 0` lumps following a unique named marker. The marker lump is
    /// included in the result.
    ///
    /// Returns `Ok(None)` if the block is missing.
    ///
    /// # Panics
    ///
    /// Panics if `size == 0`.
    pub fn try_lumps_following(&self, start: &str, size: usize) -> wad::Result<Option<&[Lump]>> {
        assert!(size > 0);

        let start_index = self.try_lump_index(start)?;
        if start_index.is_none() {
            return Ok(None);
        }
        let start_index = start_index.unwrap();

        if start_index + size >= self.lumps.len() {
            return Err(self.error(&format!("{} missing lumps", start)));
        }

        Ok(Some(&self.lumps[start_index..start_index + size]))
    }

    /// Retrieves a block of lumps between unique start and end markers. The marker lumps are
    /// included in the result.
    ///
    /// It is an error if the block is missing.
    pub fn lumps_between(&self, start: &str, end: &str) -> wad::Result<&[Lump]> {
        self.try_lumps_between(start, end)?
            .ok_or_else(|| self.error(&format!("{} and {} missing", start, end)))
    }

    /// Retrieves a block of lumps between unique start and end markers. The marker lumps are
    /// included in the result.
    ///
    /// Returns `Ok(None)` if the block is missing.
    pub fn try_lumps_between(&self, start: &str, end: &str) -> wad::Result<Option<&[Lump]>> {
        let start_index = self.try_lump_index(start)?;
        let end_index = self.try_lump_index(end)?;

        match (start_index, end_index) {
            (Some(_), Some(_)) => {}

            (None, None) => {
                return Ok(None);
            }

            (Some(_), None) => {
                return Err(self.error(&format!("{} without {}", start, end)));
            }

            (None, Some(_)) => {
                return Err(self.error(&format!("{} without {}", end, start)));
            }
        }

        let start_index = start_index.unwrap();
        let end_index = end_index.unwrap();

        if start_index > end_index {
            return Err(self.error(&format!("{} after {}", start, end)));
        }

        Ok(Some(&self.lumps[start_index..end_index + 1]))
    }

    /// Looks up a lump's index. It's an error if the lump isn't unique.
    fn try_lump_index(&self, name: &str) -> wad::Result<Option<usize>> {
        let indices: Option<&[usize]> = self.lump_indices.get(name).map(Vec::as_slice);

        match indices {
            Some(&[index]) => Ok(Some(index)),
            Some(indices) => Err(self.error(&format!("{} found {} times", name, indices.len()))),
            None => Ok(None),
        }
    }

    /// Creates a [`wad::Error::Malformed`] blaming this file.
    pub fn error(&self, desc: &str) -> wad::Error {
        wad::Error::malformed(&self.path, desc)
    }
}

impl fmt::Display for WadFile {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.path.display())
    }
}

#[cfg(test)]
mod tests {
    //! This file is covered by tests in [`crate::wad::wad`].
}
