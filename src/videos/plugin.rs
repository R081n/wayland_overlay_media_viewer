use bevy::{asset::RenderAssetUsages, prelude::*, render::render_resource::Extent3d};
use image::DynamicImage;
use std::{
    ops::DerefMut,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::{lifecycle::Closing, videos::videos::FfmpegPlayer};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VideoState {
    Init,
    Playing,
    Paused,
    Start,
    Ready,
    Loading,
    Finished,
    #[allow(dead_code)]
    Stop,
}

#[derive(Component, Clone)]
pub struct VideoPlayer {
    pub state: VideoState,
    pub timer: Arc<Mutex<Timer>>,
    pub uri: String,
    pub pipeline: Option<FfmpegPlayer>,
    pub played_frames: u64,
}

#[derive(Component)]
pub struct VideoTarget {
    pub handle: Handle<Image>,
}

impl VideoPlayer {
    /// Get current playback position in seconds
    pub fn position(&self) -> f64 {
        self.pipeline
            .as_ref()
            .and_then(|p| p.current_position.lock().ok())
            .map(|p| *p)
            .unwrap_or(0.0)
    }

    /// Get total video duration in seconds
    pub fn duration(&self) -> f64 {
        self.pipeline
            .as_ref()
            .and_then(|p| p.duration.lock().ok())
            .map(|d| *d)
            .unwrap_or(0.0)
    }

    /// Get playback progress as a ratio (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        let duration = self.duration();
        if duration > 0.0 {
            (self.position() / duration) as f32
        } else {
            0.0
        }
    }
}

pub struct VideoPlugin;

impl Plugin for VideoPlugin {
    fn build(&self, _: &mut App) {}
}

fn handle_playing_state(
    video_player: &mut VideoPlayer,
    image_handle: &mut VideoTarget,
    images: &mut Assets<Image>,
    material: &MeshMaterial3d<StandardMaterial>,
    materials: &mut Assets<StandardMaterial>,
    time: &Res<Time>,
) {
    if let Ok(mut player_time) = video_player.timer.lock()
        && player_time.tick(time.delta()).just_finished()
        && let Some(ref_pipeline) = video_player.pipeline.as_ref()
    {
        if let Ok(mut frames) = ref_pipeline.frame.lock()
            && let Some(data) = frames.pop_front()
        {
            // Update current position based on the frame being rendered
            if let Ok(mut pos) = ref_pipeline.current_position.lock() {
                *pos = data.position_secs;
            }

            if let Some(rbg_data) = image::RgbaImage::from_raw(data.width, data.height, data.data) {
                let canvas: Image = Image::from_dynamic(
                    DynamicImage::ImageRgba8(rbg_data),
                    false,
                    RenderAssetUsages::default(),
                );

                // must touch this to trigger update
                let mut mat = materials.get_mut(material.id()).unwrap();
                _ = mat.deref_mut();
                video_player.played_frames += 1;

                let mut old = images.get_mut(image_handle.handle.id()).unwrap();
                *old = canvas;
                if let Ok(mut pts) = ref_pipeline.previous_pts.lock() {
                    // Handle first frame: initialize previous_pts
                    if *pts == 0 {
                        *pts = data.pts;
                        player_time.set_duration(Duration::from_millis(33));
                    // ~30fps default
                    } else if data.pts > *pts {
                        let dt = (data.pts - *pts) / 1_000_000;
                        // Clamp dt to reasonable range (1ms - 100ms)
                        let dt = dt.clamp(1, 100);
                        player_time.set_duration(Duration::from_millis(dt));
                        *pts = data.pts;
                    } else {
                        *pts = data.pts;
                    }
                }
            }
        } else if ref_pipeline
            .finished
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            video_player.state = VideoState::Finished;
        }
    }
}

fn initialize_video_player(video_player: &mut VideoPlayer) {
    let pipeline = FfmpegPlayer::new(video_player.uri.as_str());
    let pipeline_clone = Arc::new(Mutex::new(pipeline.clone()));
    thread::spawn(move || {
        if let Ok(mut pipeline) = pipeline_clone.lock() {
            pipeline.start();
        }
    });
    video_player.pipeline = Some(pipeline);
}

pub fn render_video_frame(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut VideoPlayer,
        &mut VideoTarget,
        &MeshMaterial3d<StandardMaterial>,
        Has<Closing>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time>,
) {
    for (id, mut video_player, mut image_handle, material, is_closing) in query.iter_mut() {
        match video_player.state {
            VideoState::Playing => handle_playing_state(
                &mut video_player,
                &mut image_handle,
                &mut images,
                material,
                &mut materials,
                &time,
            ),
            VideoState::Init => {
                // println!("[DEBUG] State: Init -> Ready, initializing video player");

                video_player.state = VideoState::Ready;
                initialize_video_player(&mut video_player);
            }
            VideoState::Start => {
                // println!("[DEBUG] State: Start, checking if ready...");
                // Initialize pipeline if not already done (handles Init -> Start skip)
                if video_player.pipeline.is_none() {
                    // println!("[DEBUG] Pipeline was None, initializing...");
                    initialize_video_player(&mut video_player);
                }
                // Check if video is ready
                let is_ready = video_player
                    .pipeline
                    .as_ref()
                    .map(|p| p.is_ready.load(std::sync::atomic::Ordering::Relaxed))
                    .unwrap_or(false);

                if is_ready {
                    // println!("[DEBUG] Video is ready, starting playback");
                    video_player.state = VideoState::Playing;
                    if let Some(ref pipeline) = video_player.pipeline {
                        pipeline.play();
                    }
                } else {
                    // println!("[DEBUG] Video not ready yet, switching to Loading state");
                    video_player.state = VideoState::Loading;
                }
            }
            VideoState::Loading => {
                // Wait for video to be ready
                let is_ready = video_player
                    .pipeline
                    .as_ref()
                    .map(|p| p.is_ready.load(std::sync::atomic::Ordering::Relaxed))
                    .unwrap_or(false);

                if is_ready {
                    // println!("[DEBUG] Video is now ready, starting playback");
                    video_player.state = VideoState::Playing;
                    if let Some(ref pipeline) = video_player.pipeline {
                        pipeline.play();
                    }
                }
            }
            VideoState::Paused => {
                if let Some(ref pipeline) = video_player.pipeline {
                    pipeline.pause();
                }
            }
            VideoState::Stop => {
                if let Some(ref pipeline) = video_player.pipeline {
                    pipeline.destroy();
                }
            }
            VideoState::Finished => {
                if let Some(ref pipeline) = video_player.pipeline {
                    pipeline.destroy();
                }
                if !is_closing {
                    commands.entity(id).insert(Closing);
                }
            }
            _ => {}
        }
    }
}

pub fn insert_video_component(
    images: &mut Assets<Image>,
    default_size: Vec2,
) -> (impl Bundle, Handle<Image>) {
    let mut canvas = Image::from_dynamic(
        DynamicImage::new_rgb8(500, 500),
        true,
        RenderAssetUsages::default(),
    );
    canvas.resize(Extent3d {
        width: default_size.x as u32,
        height: default_size.y as u32,
        ..default()
    });
    let image_handle = images.add(canvas);
    (
        VideoTarget {
            handle: image_handle.clone(),
        },
        image_handle,
    )
}

pub fn on_video_removed(
    removed: On<Remove, VideoPlayer>,
    mut query: Query<&mut VideoPlayer>,
) -> Result<(), BevyError> {
    let mut player = query.get_mut(removed.entity)?;
    player.state = VideoState::Stop;
    if let Some(pipeline) = &player.pipeline {
        pipeline.destroy();
    };

    Ok(())
}
