use std::{io, path::Path, sync::Arc};

use crate::{Lump, Wad, WadFile, WadType};

/// A stack of one or more WAD files layered on top of each other, with later
/// files overlaying earlier ones. Usually the first WAD is a IWAD and the rest
/// are PWADs, but that's not a strict requirement. Other combinations are
/// allowed.
#[derive(Clone)]
pub struct WadStack {
    wads: Vec<Arc<dyn Wad>>,
}

impl WadStack {
    /// Creates a stack starting with a IWAD such as `doom.wad`.
    pub fn iwad(file: impl AsRef<Path>) -> io::Result<Self> {
        Self::iwad_impl(file.as_ref())
    }

    fn iwad_impl(file: &Path) -> io::Result<Self> {
        let wad = WadFile::open(file)?;

        match wad.wad_type() {
            WadType::Iwad => Ok(Self {
                wads: vec![Arc::new(wad)],
            }),
            WadType::Pwad => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} not an IWAD", file.display()),
            )),
        }
    }

    /// Adds a PWAD that overlays files earlier in the stack.
    pub fn pwad(mut self, file: impl AsRef<Path>) -> io::Result<Self> {
        self.add_pwad(file)?;
        Ok(self)
    }

    /// Adds a PWAD that overlays files earlier in the stack.
    pub fn add_pwad(&mut self, file: impl AsRef<Path>) -> io::Result<()> {
        self.add_pwad_impl(file.as_ref())
    }

    fn add_pwad_impl(&mut self, file: &Path) -> io::Result<()> {
        let wad = WadFile::open(file)?;

        match wad.wad_type() {
            WadType::Pwad => {
                self.wads.push(Arc::new(wad));
                Ok(())
            }
            WadType::Iwad => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} not a PWAD", file.display()),
            )),
        }
    }

    /// Creates an empty stack. Use this if you want to bypass IWAD/PWAD type
    /// checking.
    pub fn new() -> Self {
        Self { wads: Vec::new() }
    }

    /// Adds a generic [`Wad`] to the stack. Use this if you want to bypass
    /// IWAD/PWAD type checking.
    pub fn add(&mut self, wad: impl Wad + 'static) {
        self.wads.push(Arc::new(wad));
    }
}

impl Wad for WadStack {
    /// Retrieves a named lump. The name must be unique.
    ///
    /// Lumps in later files override lumps from earlier ones.
    fn lump(&self, name: &str) -> Option<&Lump> {
        self.wads.iter().rev().find_map(|wad| wad.lump(name))
    }

    /// Retrieves a block of `size` lumps following a named marker. The marker lump
    /// is not included in the result.
    ///
    /// Blocks in later files override entire blocks from earlier files.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use kdoom::WadStack;
    ///
    /// let wad = WadStack::iwad("doom.wad")?.pwad("killer.wad")?;
    /// let map = wad.lumps_after("E1M5", 10);
    /// # Ok::<(), std::io::Error>(())
    /// ```
    fn lumps_after(&self, start: &str, size: usize) -> Option<&[Lump]> {
        self.wads
            .iter()
            .rev()
            .find_map(|wad| wad.lumps_after(start, size))
    }

    /// Retrieves a block of lumps between start and end markers. The marker lumps
    /// are not included in the result.
    ///
    /// Blocks in later wads override entire blocks from earlier files.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use kdoom::WadStack;
    ///
    /// let wad = WadStack::iwad("doom2.wad")?.pwad("biotech.wad")?;
    /// let sprites = wad.lumps_between("SS_START", "SS_END");
    /// # Ok::<(), std::io::Error>(())
    /// ```
    fn lumps_between(&self, start: &str, end: &str) -> Option<&[Lump]> {
        self.wads
            .iter()
            .rev()
            .find_map(|wad| wad.lumps_between(start, end))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn iwad_then_pwads() {
        // IWAD + PWAD = success.
        WadStack::iwad(test_path("doom.wad"))
            .unwrap()
            .pwad(test_path("killer.wad"))
            .unwrap();

        // IWAD + IWAD = error.
        let mut wad = WadStack::iwad(test_path("doom.wad")).unwrap();
        assert!(wad.add_pwad(test_path("doom2.wad")).is_err());

        // Can't start with a PWAD.
        assert!(WadStack::iwad(test_path("killer.wad")).is_err());
    }

    #[test]
    fn layering() {
        let mut wad = WadStack::iwad(test_path("doom2.wad")).unwrap();
        assert_eq!(wad.lump("DEMO3").unwrap().size(), 17898);
        assert_eq!(
            wad.lumps_after("MAP01", 10)
                .unwrap()
                .iter()
                .map(|lump| (lump.name.as_str(), lump.size()))
                .collect::<Vec<_>>(),
            [
                ("THINGS", 690),
                ("LINEDEFS", 5180),
                ("SIDEDEFS", 15870),
                ("VERTEXES", 1532),
                ("SEGS", 7212),
                ("SSECTORS", 776),
                ("NODES", 5404),
                ("SECTORS", 1534),
                ("REJECT", 436),
                ("BLOCKMAP", 6418),
            ],
        );
        assert_eq!(wad.lumps_between("S_START", "S_END").unwrap().len(), 1381);

        wad.add_pwad(test_path("biotech.wad")).unwrap();
        assert_eq!(wad.lump("DEMO3").unwrap().size(), 9490);
        assert_eq!(
            wad.lumps_after("MAP01", 10)
                .unwrap()
                .iter()
                .map(|lump| (lump.name.as_str(), lump.size()))
                .collect::<Vec<_>>(),
            [
                ("THINGS", 1050),
                ("LINEDEFS", 5040),
                ("SIDEDEFS", 17400),
                ("VERTEXES", 1372),
                ("SEGS", 7536),
                ("SSECTORS", 984),
                ("NODES", 6860),
                ("SECTORS", 2184),
                ("REJECT", 882),
                ("BLOCKMAP", 4362),
            ],
        );
        assert_eq!(wad.lumps_between("S_START", "S_END").unwrap().len(), 1381);
        assert_eq!(wad.lumps_between("SS_START", "SS_END").unwrap().len(), 263);
    }

    #[test]
    fn no_type_checking() {
        let mut super_wad = WadStack::new();

        // Nonsensical ordering.
        super_wad.add(WadFile::open(test_path("killer.wad")).unwrap());
        super_wad.add(WadFile::open(test_path("doom2.wad")).unwrap());
        super_wad.add(WadFile::open(test_path("doom.wad")).unwrap());
        super_wad.add(WadFile::open(test_path("biotech.wad")).unwrap());

        assert!(super_wad.lump("E1M1").is_some());
        assert!(super_wad.lump("MAP01").is_some());
    }

    #[test]
    fn add_static_refs() {
        let wad: &'static _ = Box::leak(Box::new(WadStack::new()));
        let mut stack = WadStack::new();
        stack.add(wad);
    }

    #[test]
    fn add_trait_objects() {
        let boxed: Box<dyn Wad> = Box::new(WadStack::new());
        let arced: Arc<dyn Wad> = Arc::new(WadStack::new());

        let mut stack = WadStack::new();
        stack.add(boxed);
        stack.add(arced);
    }

    fn test_path(path: impl AsRef<Path>) -> PathBuf {
        Path::new("test").join(path)
    }
}
