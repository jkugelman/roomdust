pub mod flat;
pub mod image;
pub mod map;
pub mod palette;
pub mod patch;
pub mod texture;
pub mod wad;

use self::flat::FlatBank;
use self::palette::PaletteBank;
use self::texture::TextureBank;
use self::wad::Wad;

/// Holds all of the assets loaded from a [`Wad`]: maps, sprites, textures, sounds, etc.
#[derive(Debug)]
pub struct Assets {
    _palette_bank: PaletteBank,
    _flat_bank: FlatBank,
    _texture_bank: TextureBank,
}

impl Assets {
    /// Loads assets from a [`Wad`].
    pub fn load(wad: &Wad) -> wad::Result<Self> {
        let palette_bank = PaletteBank::load(&wad)?;
        let flat_bank = FlatBank::load(&wad)?;
        let texture_bank = TextureBank::load(&wad)?;

        Ok(Assets {
            _palette_bank: palette_bank,
            _flat_bank: flat_bank,
            _texture_bank: texture_bank,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::*;

    #[test]
    fn load() {
        Assets::load(&DOOM_WAD).unwrap();
        Assets::load(&DOOM2_WAD).unwrap();
        Assets::load(&KILLER_WAD).unwrap();
        Assets::load(&BIOTECH_WAD).unwrap();
    }
}