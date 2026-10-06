// Spec: specs/client/assets.md
// Spec: specs/formats/mpq.md (Archive set); loaders for specs/formats/*.md
//! Bevy asset source `mpq://` backed by the user's archives, and asset
//! loaders for the D2 formats.
//!
//! Paths are canonical (§A1, [`path`]): lowercase, `/` separators:
//! `mpq://data/global/palette/act1/pal.dat`. Residency and budgets are the
//! plain-Rust [`cache`].

pub mod cache;
pub mod path;
pub mod size;
pub mod tbl;

use std::path::Path;
use std::sync::Arc;

use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, PathStream, Reader, VecReader,
};
use bevy::asset::{AssetLoader, LoadContext};
use bevy::prelude::*;
use d2_formats::cof::Cof;
use d2_formats::dc6::Dc6;
use d2_formats::dcc::Dcc;
use d2_formats::ds1::Ds1;
use d2_formats::dt1::Dt1;
use d2_formats::mpq::ArchiveSet;
use d2_formats::palette::{Palette, Pl2};

pub use self::path::{CanonicalPath, FileSource};
pub use self::tbl::TblAsset;

/// Asset source name: `mpq://...`.
pub const SOURCE: &str = "mpq";

/// Archive path (`\` separators) for an asset path (`/` separators).
pub fn archive_name(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\")
}

/// Asset path (with source) for an archive path, in canonical spelling
/// (§A1): `DATA\Global\X.DC6` → `mpq://data/global/x.dc6`. Total; a path
/// that is not valid ([`CanonicalPath::new`]) is refused by the reader.
pub fn asset_path(archive_path: &str) -> String {
    format!("{SOURCE}://{}", path::fold(archive_path))
}

struct MpqReader<S: ?Sized> {
    archives: Arc<S>,
}

impl<S: FileSource + ?Sized> AssetReader for MpqReader<S> {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let name = path.to_string_lossy();
        match path::read_asset(&*self.archives, &name) {
            Ok(bytes) => Ok(VecReader::new(bytes)),
            Err(path::ReadError::NotFound(_)) => {
                Err(AssetReaderError::NotFound(path.to_path_buf()))
            }
            Err(e) => Err(AssetReaderError::Io(Arc::new(std::io::Error::other(
                e.to_string(),
            )))),
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
        register_source(app, self.archives.clone());
    }
}

/// Registers `mpq://` over any [`FileSource`] (a windowless test app, a
/// tool). Must be added **before** `DefaultPlugins`, like
/// [`MpqSourcePlugin`].
pub struct FileSourcePlugin {
    pub source: Arc<dyn FileSource>,
}

impl Plugin for FileSourcePlugin {
    fn build(&self, app: &mut App) {
        register_source(app, self.source.clone());
    }
}

fn register_source<S: FileSource + ?Sized>(app: &mut App, archives: Arc<S>) {
    app.register_asset_source(
        SOURCE,
        AssetSourceBuilder::new(move || {
            Box::new(MpqReader {
                archives: archives.clone(),
            })
        }),
    );
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

        d2_loader!($asset, $loader, |bytes| Ok($asset($parse(bytes)?)), [$($ext),*]);
    };
}

/// Declares a loader that builds `$asset` from the file bytes.
macro_rules! d2_loader {
    ($asset:ident, $loader:ident, |$bytes:ident| $build:expr, [$($ext:literal),*]) => {
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
                let mut buf = Vec::new();
                reader.read_to_end(&mut buf).await?;
                let $bytes: &[u8] = &buf;
                $build
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
d2_asset!(Pl2Asset, Pl2Loader, Pl2, Pl2::parse, ["pl2"]);
d2_asset!(CofAsset, CofLoader, Cof, Cof::parse, ["cof"]);
d2_loader!(
    TblAsset,
    TblLoader,
    |bytes| Ok(TblAsset::parse(bytes)?),
    ["tbl"]
);

/// Registers the D2 asset types and loaders. Add **after** `DefaultPlugins`.
pub struct D2AssetsPlugin;

impl Plugin for D2AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Ds1Asset>()
            .init_asset::<Dt1Asset>()
            .init_asset::<PaletteAsset>()
            .init_asset::<Dc6Asset>()
            .init_asset::<DccAsset>()
            .init_asset::<Pl2Asset>()
            .init_asset::<CofAsset>()
            .init_asset::<TblAsset>()
            .register_asset_loader(Ds1Loader)
            .register_asset_loader(Dt1Loader)
            .register_asset_loader(PaletteLoader)
            .register_asset_loader(Dc6Loader)
            .register_asset_loader(DccLoader)
            .register_asset_loader(Pl2Loader)
            .register_asset_loader(CofLoader)
            .register_asset_loader(TblLoader);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::LoadState;

    // Covers: specs/client/assets.md §a1-paths-and-identity
    #[test]
    fn path_conversion() {
        assert_eq!(
            archive_name(Path::new("data/global/palette/act1/pal.dat")),
            r"data\global\palette\act1\pal.dat"
        );
        assert_eq!(
            asset_path(r"data\global\tiles\ACT1\TOWN\floor.dt1"),
            "mpq://data/global/tiles/act1/town/floor.dt1"
        );
    }

    /// A windowless app with `mpq://` over `files`.
    fn app(files: path::MemorySource) -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            FileSourcePlugin {
                source: Arc::new(files),
            },
            AssetPlugin::default(),
            D2AssetsPlugin,
        ));
        app
    }

    /// Runs the app until `id` leaves the loading states.
    fn settle(app: &mut App, id: impl Into<bevy::asset::UntypedAssetId> + Copy) -> LoadState {
        let start = std::time::Instant::now();
        loop {
            app.update();
            let state = app.world().resource::<AssetServer>().load_state(id);
            if !matches!(state, LoadState::NotLoaded | LoadState::Loading) {
                return state;
            }
            assert!(start.elapsed().as_secs() < 30, "asset never settled");
            std::thread::yield_now();
        }
    }

    // Covers: specs/client/assets.md §a1-paths-and-identity, §a2-loaders
    #[test]
    fn two_spellings_one_handle() {
        // Test vector §A1, through the AssetServer; and §A2 `tbl` with the
        // font magic loads as `TblAsset::Font`.
        let mut font = b"Woo!".to_vec();
        font.extend_from_slice(&1u16.to_le_bytes());
        font.extend_from_slice(&[0, 0, 0, 0, 16, 12]);
        let mut files = path::MemorySource::default();
        files.insert(r"data\local\font\latin\font16.tbl", font);
        let mut app = app(files);
        let server = app.world().resource::<AssetServer>().clone();
        let a: Handle<TblAsset> = server.load(asset_path(r"DATA\Local\FONT\LATIN\Font16.TBL"));
        let b: Handle<TblAsset> = server.load(asset_path(r"data\local\font\latin\font16.tbl"));
        assert_eq!(a.id(), b.id());
        assert!(settle(&mut app, a.id()).is_loaded());
        let assets = app.world().resource::<Assets<TblAsset>>();
        assert!(matches!(assets.get(&a), Some(TblAsset::Font(_))));
    }

    // Covers: specs/client/assets.md §a1-paths-and-identity
    #[test]
    fn missing_asset_fails_naming_the_path() {
        // Test vector §A1: no fallback asset.
        let mut app = app(path::MemorySource::default());
        let server = app.world().resource::<AssetServer>().clone();
        let h: Handle<CofAsset> = server.load(asset_path(r"data\global\missing.cof"));
        match settle(&mut app, h.id()) {
            LoadState::Failed(e) => {
                assert!(e.to_string().contains("data/global/missing.cof"), "{e}")
            }
            other => panic!("expected a failure, got {other:?}"),
        }
    }
}
