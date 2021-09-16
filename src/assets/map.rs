mod geom;

pub use geom::*;

use std::fmt;

use super::wad::{self, LumpRef, Wad};

pub struct Map {
    name: String,
    _things: (),
    _vertices: (),
    _sides: (),
    _lines: (),
    _sectors: (),
}

impl Map {
    /// Load a map, typically named `ExMy` for DOOM or `MAPnn` for DOOM II.
    ///
    /// Returns `Ok(None)` if the map is missing.
    pub fn load(wad: &Wad, name: &str) -> wad::Result<Option<Self>> {
        let lumps = wad.try_lumps_following(name, 11)?;
        if lumps.is_none() {
            return Ok(None);
        }
        let lumps = lumps.unwrap();

        let name = name.to_string();
        let things = Self::read_things(lumps.get_with_name(1, "THINGS")?);
        let vertices = Self::read_vertices(lumps.get_with_name(4, "VERTEXES")?);
        let sectors = Self::read_sectors(lumps.get_with_name(8, "SECTORS")?);
        let sides = Self::read_sides(lumps.get_with_name(3, "SIDEDEFS")?);
        let lines = Self::read_lines(lumps.get_with_name(2, "LINEDEFS")?);

        Ok(Some(Map {
            name,
            _things: things,
            _vertices: vertices,
            _sides: sides,
            _lines: lines,
            _sectors: sectors,
        }))
    }

    fn read_things(_lump: LumpRef) {}
    fn read_vertices(_lump: LumpRef) {}
    fn read_sectors(_lump: LumpRef) {}
    fn read_sides(_lump: LumpRef) {}
    fn read_lines(_lump: LumpRef) {}
}

impl fmt::Debug for Map {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{:?}", self.name)
    }
}

impl fmt::Display for Map {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{}", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::*;

    #[test]
    fn load() {
        assert_matches!(Map::load(&*DOOM_WAD, "E1M1"), Ok(Some(_)));
        assert_matches!(Map::load(&*DOOM_WAD, "E9M9"), Ok(None));
    }
}
