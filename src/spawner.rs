use std::{
    f32::consts::PI,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{
    lifecycle,
    position::{PopupPosition, PIXELS_PER_METER},
    videos::plugin::{insert_video_component, VideoPlayer, VideoState, VideoTarget},
};
use bevy::{
    asset::{LoadState, UntypedAssetId},
    platform::collections::HashMap,
    prelude::*,
};

#[derive(Message, Clone)]
pub struct ObjectMessage {
    pub kind: ObjectType,
    pub position: PopupPosition,
    pub popup_animation: PopupAnimation,
    pub behaviour: PopupInteraction,
}

#[derive(Debug, Clone)]
pub enum ObjectType {
    Image(PopupImage),
    Video(PopupVideo),
    // Text,
    //Shader,
}

#[derive(Debug, Clone)]
pub enum PopupInteraction {
    ClickThough,
}

#[derive(Debug, Clone)]
pub enum PopupAnimation {
    None,
    SlideFadeIn,
}

#[derive(Debug, Clone)]
pub struct PopupImage {
    pub uri: String,
}

#[derive(Debug, Clone)]
pub struct PopupVideo {
    pub uri: String,
}

pub struct PopupPlugin;

impl Plugin for PopupPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PendingAssets::default())
            .add_message::<ObjectMessage>()
            .insert_resource(DefaultMeshes::default())
            .add_systems(Startup, init_rect_mesh)
            .add_systems(
                Update,
                (
                    preload_asset,
                    spawn_object,
                    spawn_videos,
                    wait_for_video_to_load,
                )
                    .chain(),
            );
    }
}

#[derive(Default, Resource)]
pub struct PendingAssets {
    pending: HashMap<UntypedHandle, Vec<ObjectMessage>>,
}

#[derive(Default, Resource)]
pub struct DefaultMeshes {
    rect: Handle<Mesh>,
}

fn init_rect_mesh(mut meshes: ResMut<Assets<Mesh>>, mut default_meshes: ResMut<DefaultMeshes>) {
    let handle = meshes.add(Rectangle::from_size(Vec2::ONE));
    default_meshes.rect = handle;
}

fn preload_asset(
    mut assets: ResMut<AssetServer>,
    mut new: MessageReader<ObjectMessage>,
    mut pendnig: ResMut<PendingAssets>,
) {
    for new in new.read() {
        let handle = match &new.kind {
            ObjectType::Image(popup_image) => assets.load::<Image>(&popup_image.uri).untyped(),

            // Handeled differently
            ObjectType::Video(popup_video) => continue,
        };

        pendnig
            .pending
            .entry(handle.clone())
            .or_default()
            .push(new.clone());
    }
}

struct ImageMesh {
    handle: Handle<Mesh>,
}

struct CommonProps<'a> {
    commands: EntityCommands<'a>,
    scale: Vec3,
}
fn spawn_object(
    mut commands: Commands,
    mut pending: ResMut<PendingAssets>,
    server: ResMut<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut default_meshes: Res<DefaultMeshes>,
) {
    for (id, pending) in pending
        .pending
        .extract_if(|id, _| server.is_loaded_with_dependencies(id))
    {
        for msg in pending {
            let mut common = match &msg.kind {
                ObjectType::Image(popup_image) => {
                    let Ok(asset) = id.clone().try_typed::<Image>() else {
                        continue;
                    };

                    let image = images.get(&asset).expect("asset to exist");

                    CommonProps {
                        commands: commands.spawn((
                            Mesh3d(default_meshes.rect.clone()),
                            MeshMaterial3d(materials.add(StandardMaterial {
                                base_color: Color::Srgba(Srgba::new(1., 1., 1., 0.1)),
                                unlit: true,
                                base_color_texture: Some(asset),
                                alpha_mode: AlphaMode::Blend,
                                ..default()
                            })),
                        )),
                        scale: Vec3::new(image.width() as f32, image.height() as f32, 0.)
                            / PIXELS_PER_METER,
                    }
                }
                // Not handled here
                ObjectType::Video(_) => continue,
            };

            lifecycle::insert_components(&mut common.commands, msg);

            common
                .commands
                .entry::<Transform>()
                .or_default()
                .and_modify(move |mut t| *t = t.with_scale(common.scale));
        }
    }

    for (_, _) in pending.pending.extract_if(|id, _| {
        if let LoadState::Failed(failed) = server.load_state(id) {
            error!("Faild to load asset {:?}", failed);
            true
        } else {
            false
        }
    }) {}
}

#[derive(Component)]
struct VideoDeferredObjectMessage(ObjectMessage);

fn spawn_videos(
    mut commands: Commands,
    mut assets: ResMut<AssetServer>,
    mut new: MessageReader<ObjectMessage>,
    mut pendnig: ResMut<PendingAssets>,
    mut images: ResMut<Assets<Image>>,
    mut default_meshes: Res<DefaultMeshes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for new in new.read() {
        let ObjectType::Video(video) = &new.kind else {
            continue;
        };

        let video_player = VideoPlayer {
            uri: video.uri.clone(),
            state: VideoState::Start,
            timer: Arc::new(Mutex::new(Timer::from_seconds(0.001, TimerMode::Repeating))),
            pipeline: None,
            played_frames: 0,
        };

        let (component, handle) = insert_video_component(&mut images, Vec2::new(1.0, 2.0));

        commands
            .spawn((
                component,
                Mesh3d(default_meshes.rect.clone()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    unlit: true,
                    base_color_texture: Some(handle),
                    base_color: Color::WHITE,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                })),
                Visibility::Visible,
                VideoDeferredObjectMessage(new.clone()),
            ))
            .insert(video_player);
    }
}

fn wait_for_video_to_load(
    mut commands: Commands,
    mut videos: Query<(
        Entity,
        &VideoPlayer,
        &mut Visibility,
        &VideoDeferredObjectMessage,
        &VideoTarget,
    )>,
    images: Res<Assets<Image>>,
) -> Result<(), BevyError> {
    for (id, player, mut vis, msg, target) in videos.iter_mut() {
        if player.played_frames == 0 {
            continue;
        }

        let msg = msg.0.clone();

        *vis = Visibility::Visible;

        let mut commands = commands.entity(id);
        commands.remove::<VideoDeferredObjectMessage>();
        lifecycle::insert_components(&mut commands, msg);

        let target = images.get(target.handle.id()).unwrap();

        let scale = Vec3::new(target.width() as f32, target.height() as f32, 0.) / PIXELS_PER_METER;

        commands
            .entry::<Transform>()
            .or_default()
            .and_modify(move |mut t| *t = t.with_scale(scale));
    }

    Ok(())
}
