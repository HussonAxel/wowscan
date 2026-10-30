//! Icônes haute définition d'un pack installé dans le dossier du jeu (`Interface\ICONS`).
//!
//! Ces packs (CleanIcons…) sont en TGA, que la vue web ne sait pas afficher : on les convertit en PNG.

use std::io::Cursor;
use std::path::Path;

use image::ImageFormat;

/// L'icône `name` en PNG, si le pack la contient. Le nom vient d'une URL : il ne doit pas pouvoir sortir du dossier.
pub fn hd_icon_png(game_dir: &Path, name: &str) -> Option<Vec<u8>> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    let tga = std::fs::read(game_dir.join("Interface").join("ICONS").join(format!("{name}.tga"))).ok()?;
    let icon = image::load_from_memory_with_format(&tga, ImageFormat::Tga).ok()?;
    let mut png = Cursor::new(Vec::new());
    icon.write_to(&mut png, ImageFormat::Png).ok()?;
    Some(png.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_tga_and_rejects_paths() {
        let dir = tempfile::tempdir().unwrap();
        let icons = dir.path().join("Interface").join("ICONS");
        std::fs::create_dir_all(&icons).unwrap();
        // TGA non compressé de 1×1, 32 bits.
        let mut tga = vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 32, 8];
        tga.extend([10, 20, 30, 255]);
        std::fs::write(icons.join("INV_Test.tga"), tga).unwrap();

        let png = hd_icon_png(dir.path(), "INV_Test").unwrap();
        assert_eq!(&png[1..4], b"PNG");
        assert_eq!(hd_icon_png(dir.path(), "INV_Absent"), None);
        assert_eq!(hd_icon_png(dir.path(), "../ICONS/INV_Test"), None);
    }
}
