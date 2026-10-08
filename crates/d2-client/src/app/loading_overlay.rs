// Spec: specs/ui/frontend-loading.md (L3–L7)
//! The loading screen over the running game: a black cover while
//! [`LoadingState`] is active, with frame `n` of `loadingscreen.dc6`
//! (Loading palette) at `placement` when the game's files are given.
//!
//! `// d2rs-own, unverified` and PROVISIONAL (REC-236): a UI node over the
//! world stands for the original's full-frame present; the frame is drawn
//! as an image node, not by the sprite compositor.

use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use d2_formats::dc6::Dc6;
use d2_formats::palette::Palette;

use crate::app::front_start::LoadingState;
use crate::assets::path::FileSource;
use crate::bridge::BridgeResource;
use crate::ui::front_end::screens::loading::{placement, Presented};

#[derive(Component)]
struct Cover;

#[derive(Component)]
struct Art;

#[derive(Resource, Default)]
struct LoadingFrames(Vec<Handle<Image>>);

/// RGBA of every frame of the DC6 under `palette` (index 0 transparent).
pub fn frames_rgba(dc6: &Dc6, palette: &Palette) -> Vec<(u32, u32, Vec<u8>)> {
    dc6.frames
        .iter()
        .map(|f| {
            let mut px = Vec::with_capacity((f.width * f.height * 4) as usize);
            for &i in &f.pixels {
                let c = palette.colors[usize::from(i)];
                px.extend_from_slice(&[c.r, c.g, c.b, if i == 0 { 0 } else { 255 }]);
            }
            (f.width, f.height, px)
        })
        .collect()
}

/// Installs the loading screen. `files`: the game's archives (art and the
/// Loading palette); without them the cover is plain black.
pub fn add_loading(app: &mut App, files: Option<Arc<dyn FileSource>>) {
    let frames = files
        .and_then(|s| {
            let dc6 = Dc6::parse(
                &s.read_file(crate::ui::front_end::screens::loading::art_path(0))?
                    .ok()?,
            )
            .ok()?;
            let pal =
                Palette::parse(&s.read_file(r"data\global\palette\Loading\pal.dat")?.ok()?).ok()?;
            Some(frames_rgba(&dc6, &pal))
        })
        .unwrap_or_default();
    app.insert_resource(LoadingState::default())
        .insert_resource(PendingFrames(frames))
        .init_resource::<LoadingFrames>()
        .add_systems(Startup, spawn_cover)
        .add_systems(Update, track);
}

#[derive(Resource)]
struct PendingFrames(Vec<(u32, u32, Vec<u8>)>);

fn spawn_cover(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut pending: ResMut<PendingFrames>,
    mut frames: ResMut<LoadingFrames>,
) {
    for (w, h, px) in pending.0.drain(..) {
        let mut img = Image::new(
            Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            px,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        img.sampler = bevy::image::ImageSampler::nearest();
        frames.0.push(images.add(img));
    }
    commands
        .spawn((
            Cover,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(1000),
        ))
        .with_children(|p| {
            p.spawn((
                Art,
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                Visibility::Hidden,
            ));
        });
}

/// One client pass: feed the model state, show or hide the cover, set the
/// art frame.
fn track(
    mut state: ResMut<LoadingState>,
    bridge: Res<BridgeResource>,
    frames: Res<LoadingFrames>,
    windows: Query<&Window>,
    mut cover: Query<&mut Visibility, (With<Cover>, Without<Art>)>,
    mut art: Query<(&mut Visibility, &mut Node, Option<&mut ImageNode>, Entity), With<Art>>,
    mut commands: Commands,
) {
    let presented = state.frame(bridge.0.world());
    let covering = state.covering();
    for mut v in &mut cover {
        *v = if covering {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let Some(Presented::Loading { frame }) = presented.filter(|_| covering) else {
        for (mut v, ..) in &mut art {
            *v = Visibility::Hidden;
        }
        return;
    };
    let Ok(w) = windows.single() else { return };
    let (x, y) = placement(w.width() as i32, w.height() as i32);
    let Some(h) = frames.0.get(frame as usize) else {
        return;
    };
    for (mut v, mut node, img, e) in &mut art {
        // Cel position is (x, bottom y): the node's top is above it.
        node.left = Val::Px(x as f32);
        node.top = Val::Px(y as f32 - 256.0);
        *v = Visibility::Inherited;
        match img {
            Some(mut i) => i.image = h.clone(),
            None => {
                commands.entity(e).insert(ImageNode::new(h.clone()));
            }
        }
    }
}
