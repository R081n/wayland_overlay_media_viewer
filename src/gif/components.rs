use std::time::Duration;

use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    prelude::*,
};
use image::{
    codecs::{gif::GifDecoder, webp::WebPDecoder},
    metadata::LoopCount,
    AnimationDecoder as _, ImageError,
};
use thiserror::Error;

/// Entity used to spawn a [Sprite] with an animated texture.
/// This is the main and might be the only struct you will use from this crate.
///
/// ```ignore
/// commands.spawn(Gif { handle: "frog.gif" })
/// ```
#[derive(Component, Debug, Clone, Default, FromTemplate)]
#[require(Sprite, GifPlayer)]
pub struct Gif {
    pub handle: Handle<GifAsset>,
}

/// Internal state of a [Gif]. Store the current frame index, its associated timer,
/// and the number of remaining repetitions, minus the one currently running.
///
/// That means a [GifPlayer] with `remaining` being 0 and its `timer` not paused will
/// still update the [Sprite] for a last rotation.
///
/// Also: `remaining` == None is different from `remaining` == Some(0)
/// The former means: Repeat indefinitely.
/// The latter: Do not repeat _anymore_.
/// Ultimately, `remaining` == Some(n: n!= 0) means: Repeat n more time(s).
#[derive(Component, Debug, Clone)]
pub struct GifPlayer {
    pub current: usize,
    pub timer: Timer,
    pub remaining: Option<u32>,
}

impl Default for GifPlayer {
    fn default() -> Self {
        Self {
            current: 0,
            timer: Timer::new(Duration::from_millis(100), TimerMode::Repeating),
            remaining: None,
        }
    }
}

/// Contains the data of one frame of a GIF
///
/// What really distinguish this from using a [TextureAtlas] is the unique [Duration] of each frame,
/// stored within the asset.
#[derive(Debug, Clone, Reflect)]
pub struct GifFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub duration: Duration,
}

/// Contains the data of a GIF
///
/// Careful: `times` represents the raw value of the GIF repeat metadata, which can
/// be interpreted as "how many times will I _repeat_", with an emphasis on _repeat_.
/// For a GIF that plays a total of 5 loops, this value is going to be 4.
#[derive(Asset, Debug, Clone, Reflect)]
pub struct GifAsset {
    pub frames: Vec<GifFrame>,
    pub handles: Vec<Handle<Image>>,
    pub times: Option<u32>,
    pub frame_end: Vec<f64>,
}

#[derive(Error, Debug)]
pub(crate) enum AnimationDecodeError {
    /// An [IO](std::io) Error
    #[error("Could not load asset: {0}")]
    Io(#[from] std::io::Error),

    /// A data error
    #[error("Image rerror: {0}")]
    ImageError(#[from] ImageError),

    #[error("Format not supported")]
    UnsupportedFormat,
}

/// Allow to load GIF files properly with the AssetServer
#[derive(Default, TypePath)]
pub(crate) struct AnimationImageLoader;

impl AssetLoader for AnimationImageLoader {
    type Asset = GifAsset;
    type Settings = bool;
    type Error = AnimationDecodeError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();

        let mut frame_end = Vec::new();
        let mut current_time = 0.0;

        // 1. Read raw file bytes into the buffer
        reader.read_to_end(&mut bytes).await?;

        // 2. Automatically deduce the format (GIF or WebP) from the header magic numbers
        let format = image::guess_format(&bytes)?;
        let cursor = std::io::Cursor::new(bytes);

        // 3. Dynamically allocate the correct trait object to decode frames automatically
        let (loop_count, frames_iterator) = match format {
            image::ImageFormat::Gif => {
                let decoder = GifDecoder::new(cursor)?;
                (decoder.loop_count(), decoder.into_frames())
            }
            image::ImageFormat::WebP => {
                let decoder = WebPDecoder::new(cursor)?;
                (decoder.loop_count(), decoder.into_frames())
            }
            _ => return Err(AnimationDecodeError::UnsupportedFormat),
        };

        let mut frames = Vec::new();

        // 4. Iterate over the frames. Under the hood, `image` has already performed
        // all frame differencing, blending, and background/previous disposal methods.
        for frame_result in frames_iterator {
            let frame = frame_result?;

            let duration = Duration::from(frame.delay());

            current_time += duration.as_secs_f64();
            frame_end.push(current_time);

            // Convert the automatically composited underlying frame directly to an RGBA8 buffer
            let image_buffer = frame.into_buffer();
            let width = image_buffer.width();
            let height = image_buffer.height();
            let rgba = image_buffer.into_raw();

            frames.push(GifFrame {
                width,
                height,
                rgba,
                duration,
            });
        }

        // Create the GifAsset and set it as the default loaded asset
        let asset = GifAsset {
            frames,
            handles: vec![], // will be loaded in `initialize_gifs`
            times: match loop_count {
                LoopCount::Infinite => None,
                LoopCount::Finite(count) => Some(count.get()),
            },
            frame_end,
        };
        Ok(asset)
    }

    fn extensions(&self) -> &[&str] {
        &["gif", "webp"]
    }
}

/// Insert this component next to a non-infinite [Gif] to despawn the
/// entity when its loops are over.
///
/// See [despawn example](examples/despawn.rs)
///
/// It has no effect on infinite-looping GIF files.
#[derive(Component, Clone, Default)]
pub struct GifDespawn;

/// Ui component to display a gif file.
///
/// Works the same than [Gif]
#[derive(Component, Debug, Clone, FromTemplate)]
#[require(ImageNode, GifPlayer)]
pub struct GifNode {
    pub handle: Handle<GifAsset>,
}

/// 3d component to display a gif file on a 3d object.
///
/// It needs to be spawned alongside a [Mesh3d].
///
/// Works almost the same than [Gif]
#[derive(Component, Debug, Clone, FromTemplate)]
#[require(MeshMaterial3d<StandardMaterial>, GifPlayer)]
pub struct Gif3d {
    pub handle: Handle<GifAsset>,
}
