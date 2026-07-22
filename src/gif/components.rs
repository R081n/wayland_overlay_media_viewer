use std::time::Duration;

use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    prelude::*,
};
use gif::{ColorOutput, DecodeOptions, Repeat};
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
    pub remaining: Option<u16>,
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
#[derive(Debug, Clone)]
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
#[derive(Asset, TypePath, Debug, Clone)]
pub struct GifAsset {
    pub frames: Vec<GifFrame>,
    pub handles: Vec<Handle<Image>>,
    pub times: Option<u16>,
    pub frame_end: Vec<f64>,
}

#[derive(Error, Debug)]
pub(crate) enum GifLoaderError {
    /// An [IO](std::io) Error
    #[error("Could not load asset: {0}")]
    Io(#[from] std::io::Error),
    /// A [gif](gif) DecodingError
    #[error("Could not decode asset: {0}")]
    Decode(#[from] gif::DecodingError),
    /// A data error
    #[error("Decoded gif frame size mismatch: {0} != {1}")]
    SizeMismatch(usize, usize),
}

/// Allow to load GIF files properly with the AssetServer
#[derive(Default, TypePath)]
pub(crate) struct GifLoader;

impl AssetLoader for GifLoader {
    type Asset = GifAsset;
    type Settings = bool;
    type Error = GifLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let mut decoder = DecodeOptions::new();
        decoder.set_color_output(ColorOutput::RGBA);
        let mut decoder = decoder.read_info(std::io::Cursor::new(bytes))?;

        let canvas_width = decoder.width() as u32;
        let canvas_height = decoder.height() as u32;

        // The active canvas used for rendering the current frame
        let mut canvas_rgba = vec![0u8; (canvas_width as usize) * (canvas_height as usize) * 4];

        // A backup canvas to support DisposalMethod::Previous
        let mut previous_canvas = canvas_rgba.clone();
        let mut frame_end = Vec::new();
        let mut current_time = 0.0;

        let mut frames = Vec::new();
        while let Some(frame) = decoder.read_next_frame()? {
            let f_width = frame.width as u32;
            let f_height = frame.height as u32;
            let f_left = frame.left as u32;
            let f_top = frame.top as u32;
            let f_rgba = frame.buffer.to_vec();

            if f_rgba.len() != (f_width as usize) * (f_height as usize) * 4 {
                return Err(Self::Error::SizeMismatch(
                    f_rgba.len(),
                    (f_width as usize) * (f_height as usize) * 4,
                ));
            }

            // BACKUP STEP: If the NEXT frame needs to restore to the current state,
            // we save the canvas state *before* we apply the new frame modifications.
            // (Only needed if the current disposal method is NOT 'Previous')
            if frame.dispose != gif::DisposalMethod::Previous {
                previous_canvas = canvas_rgba.clone();
            }

            // Draw the current frame onto the active canvas
            for y in 0..f_height {
                let canvas_y = f_top + y;
                if canvas_y >= canvas_height {
                    continue;
                }

                for x in 0..f_width {
                    let canvas_x = f_left + x;
                    if canvas_x >= canvas_width {
                        continue;
                    }

                    let f_idx = ((y * f_width) + x) as usize * 4;
                    let canvas_idx = ((canvas_y * canvas_width) + canvas_x) as usize * 4;

                    let alpha = f_rgba[f_idx + 3];

                    // Only overwrite if the incoming pixel is not transparent
                    if alpha > 0 {
                        canvas_rgba[canvas_idx] = f_rgba[f_idx];
                        canvas_rgba[canvas_idx + 1] = f_rgba[f_idx + 1];
                        canvas_rgba[canvas_idx + 2] = f_rgba[f_idx + 2];
                        canvas_rgba[canvas_idx + 3] = f_rgba[f_idx + 3];
                    }
                }
            }

            let ms = (frame.delay as u64).saturating_mul(10);
            current_time += ms as f64 / 1000.;
            frame_end.push(current_time);
            let duration = Duration::from_millis(ms.max(1));

            // Save the fully compiled frame
            frames.push(GifFrame {
                width: canvas_width,
                height: canvas_height,
                rgba: canvas_rgba.clone(),
                duration,
            });

            // DISPOSAL STEP: Handle the canvas after saving the frame
            match frame.dispose {
                gif::DisposalMethod::Background => {
                    // Clear only the area occupied by the current frame
                    for y in 0..f_height {
                        let canvas_y = f_top + y;
                        if canvas_y >= canvas_height {
                            continue;
                        }

                        for x in 0..f_width {
                            let canvas_x = f_left + x;
                            if canvas_x >= canvas_width {
                                continue;
                            }

                            let canvas_idx = ((canvas_y * canvas_width) + canvas_x) as usize * 4;
                            canvas_rgba[canvas_idx] = 0;
                            canvas_rgba[canvas_idx + 1] = 0;
                            canvas_rgba[canvas_idx + 2] = 0;
                            canvas_rgba[canvas_idx + 3] = 0;
                        }
                    }
                }
                gif::DisposalMethod::Previous => {
                    // Roll back the entire canvas to the state before this frame was drawn
                    canvas_rgba = previous_canvas.clone();
                }
                _ => {
                    // Keep / Any: Do nothing, let the next frame build directly on top
                }
            }
        }

        let times = match decoder.repeat() {
            Repeat::Infinite => None,
            Repeat::Finite(n) => Some(n),
        };

        // Create the GifAsset and set it as the default loaded asset
        let asset = GifAsset {
            frames,
            handles: vec![], // will be loaded in `initialize_gifs`
            times,
            frame_end,
        };
        Ok(asset)
    }

    fn extensions(&self) -> &[&str] {
        &["gif"]
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
