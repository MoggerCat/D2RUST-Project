// Spec: specs/formats/mpq.md (Archive set); loaders for specs/formats/*.md
//! Bevy asset source `mpq://` backed by the user's archives, and asset
//! loaders for the D2 formats.
//!
//! Paths use `/`: `mpq://data/global/palette/act1/pal.dat`.

use std::path::Path;
use std::sync::Arc;

use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, PathStream, Reader, VecReader,
};
use bevy::asset::{AssetLoader, LoadContext};
use bevy::prelude::*;
use d2_formats::dc6::Dc6;
use d2_formats::dcc::Dcc;
use d2_formats::ds1::Ds1;
use d2_formats::dt1::Dt1;
use d2_formats::mpq::ArchiveSet;
use d2_formats::palette::Palette;

/// Asset source name: `mpq://...`.
pub const SOURCE: &str = "mpq";

/// Archive path (`\` separators) for an asset path (`/` separators).
pub fn archive_name(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\")
}

/// Asset path (with source) for an archive path.
pub fn asset_path(archive_path: &str) -> String {
    format!("{SOURCE}://{}", archive_path.replace('\\', "/"))
}

struct MpqReader {
    archives: Arc<ArchiveSet>,
}

impl AssetReader for MpqReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let name = archive_name(path);
        match self.archives.find(&name) {
            None => Err(AssetReaderError::NotFound(path.to_path_buf())),
            Some(archive) => archive
                .read(&name)
                .map(VecReader::new)
                .map_err(|e| AssetReaderError::Io(Arc::new(std::io::Error::other(e.to_string())))),
        }
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        // Archives hold no .meta files; Bevy then uses default settings.
        Err::<VecReader, _>(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn is_directory<'a>(&'a self, _path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(false)
    }
}

/// Registers the `mpq://` source. Must be added **before** `DefaultPlugins`.
pub struct MpqSourcePlugin {
    pub archives: Arc<ArchiveSet>,
}

impl Plugin for MpqSourcePlugin {
    fn build(&self, app: &mut App) {
        let archives = self.archives.clone();
        app.register_asset_source(
            SOURCE,
            AssetSourceBuilder::new(move || {
                Box::new(MpqReader {
                    archives: archives.clone(),
                })
            }),
        );
    }
}

#[derive(Debug, thiserror::Error)]
pub enum D2LoadError {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Format(#[from] d2_formats::FormatError),
}

/// Declares an asset wrapper and a loader that parses it with `parse`.
macro_rules! d2_asset {
    ($asset:ident, $loader:ident, $inner:ty, $parse:path, [$($ext:literal),*]) => {
        #[derive(Asset, TypePath, Debug)]
        pub struct $asset(pub $inner);

        #[derive(Default, TypePath)]
        pub struct $loader;

        impl AssetLoader for $loader {
            type Asset = $asset;
            type Settings = ();
            type Error = D2LoadError;

            async fn load(
                &self,
                reader: &mut dyn Reader,
                _settings: &(),
                _load_context: &mut LoadContext<'_>,
            ) -> Result<$asset, D2LoadError> {
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes).await?;
                Ok($asset($parse(&bytes)?))
            }

            fn extensions(&self) -> &[&str] {
                &[$($ext),*]
            }
        }
    };
}

d2_asset!(Ds1Asset, Ds1Loader, Ds1, Ds1::parse, ["ds1"]);
d2_asset!(Dt1Asset, Dt1Loader, Dt1, Dt1::parse, ["dt1"]);
d2_asset!(
    PaletteAsset,
    PaletteLoader,
    Palette,
    Palette::parse,
    ["dat"]
);
d2_asset!(Dc6Asset, Dc6Loader, Dc6, Dc6::parse, ["dc6"]);
d2_asset!(DccAsset, DccLoader, Dcc, Dcc::parse, ["dcc"]);

/// Registers the D2 asset types and loaders. Add **after** `DefaultPlugins`.
pub struct D2AssetsPlugin;

impl Plugin for D2AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Ds1Asset>()
            .init_asset::<Dt1Asset>()
            .init_asset::<PaletteAsset>()
            .init_asset::<Dc6Asset>()
            .init_asset::<DccAsset>()
            .register_asset_loader(Ds1Loader)
            .register_asset_loader(Dt1Loader)
            .register_asset_loader(PaletteLoader)
            .register_asset_loader(Dc6Loader)
            .register_asset_loader(DccLoader);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_conversion() {
        assert_eq!(
            archive_name(Path::new("data/global/palette/act1/pal.dat")),
            r"data\global\palette\act1\pal.dat"
        );
        assert_eq!(
            asset_path(r"data\global\tiles\ACT1\TOWN\floor.dt1"),
            "mpq://data/global/tiles/ACT1/TOWN/floor.dt1"
        );
    }
}
