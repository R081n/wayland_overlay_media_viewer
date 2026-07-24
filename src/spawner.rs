use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    animated_image::{Gif3d, GifAsset},
    lifecycle::handle_slide_fade_in,
    shader::{DynamicMaterial, SHADER_TEMPLATE, ShaderRepeatPeriod},
};
use crate::{
    lifecycle,
    position::{PIXELS_PER_METER, PopupPosition},
    videos::plugin::{VideoPlayer, VideoState, VideoTarget, insert_video_component},
};
use bevy::{asset::LoadState, platform::collections::HashMap, prelude::*};

#[derive(Message, Clone, Debug)]
pub struct ObjectMessage {
    pub kind: ObjectType,
    pub position: PopupPosition,
    pub popup_animation: PopupInAnimation,
    pub close_animation: PopupOutAnimation,
    pub behaviour: PopupInteraction,
    pub close_condition: ObjectCloseCondition,
    pub opacity: f32,
}

#[derive(Clone, Debug)]
pub struct ObjectCloseCondition {
    pub duration: Option<Duration>,
    pub click: Option<CloseClickSettings>,
}

#[derive(Clone, Debug, Default)]
pub struct CloseClickSettings {}

#[derive(Component)]
pub struct TargetOpacity(pub f32);

#[derive(Debug, Clone)]
pub enum ObjectType {
    Image(PopupImage),
    Video(PopupVideo),
    Shader(CustomShaderSource),
    // Text,
}

#[derive(Debug, Clone)]
pub enum PopupInteraction {
    ClickThough,
    Clickable,
}

#[derive(Debug, Clone)]
pub enum PopupInAnimation {
    None,
    SlideFadeIn,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum PopupOutAnimation {
    #[default]
    None,
    FadeOut {
        decay_rate: f32,
    },
}

#[derive(Debug, Clone)]
pub struct PopupImage {
    pub uri: String,
}

#[derive(Debug, Clone)]
pub enum FileType {
    StaticImage,
    WebP,
    Gif,
}

#[derive(Debug, Clone)]
pub struct PopupVideo {
    pub uri: String,
}

#[derive(Debug, Clone)]
pub struct CustomShaderSource {
    pub code: String,
    pub duraton: f32,
}
pub struct PopupPlugin;

impl Plugin for PopupPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PendingAssets::default())
            .add_message::<ObjectMessage>()
            .insert_resource(DefaultMeshes::default())
            .add_systems(Startup, init_rect_mesh)
            .add_systems(
                PreUpdate,
                (
                    preload_asset,
                    spawn_object,
                    spawn_videos,
                    spawn_shaders,
                    wait_for_video_to_load,
                )
                    .chain()
                    .before(handle_slide_fade_in),
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

pub fn preload_asset(
    assets: ResMut<AssetServer>,
    mut new: MessageReader<ObjectMessage>,
    mut pendnig: ResMut<PendingAssets>,
) {
    for new in new.read() {
        let handle = match &new.kind {
            ObjectType::Image(popup_image) => {
                match PathBuf::from(popup_image.uri.clone()).extension() {
                    Some(ext) => match ext.to_str().unwrap_or_default() {
                        "gif" => assets.load::<GifAsset>(&popup_image.uri).untyped(),
                        "webp" => assets.load::<GifAsset>(&popup_image.uri).untyped(),
                        _ => assets.load::<Image>(&popup_image.uri).untyped(),
                    },
                    None => assets.load::<Image>(&popup_image.uri).untyped(),
                }
            }

            // Handeled differently
            ObjectType::Video(_) => continue,
            ObjectType::Shader(_) => continue,
        };

        pendnig
            .pending
            .entry(handle.clone())
            .or_default()
            .push(new.clone());
    }
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
    images: ResMut<Assets<Image>>,
    gifs: ResMut<Assets<GifAsset>>,
    default_meshes: Res<DefaultMeshes>,
) {
    for (id, pending) in pending
        .pending
        .extract_if(|id, _| server.is_loaded_with_dependencies(id))
    {
        for msg in pending {
            let mut common = match &msg.kind {
                ObjectType::Image(_) => {
                    if let Ok(asset) = id.clone().try_typed::<Image>() {
                        let image = images.get(&asset).expect("asset to exist");

                        CommonProps {
                            commands: commands.spawn((
                                Mesh3d(default_meshes.rect.clone()),
                                MeshMaterial3d(materials.add(StandardMaterial {
                                    base_color: Color::Srgba(Srgba::new(1., 1., 1., msg.opacity)),
                                    unlit: true,
                                    base_color_texture: Some(asset),
                                    alpha_mode: AlphaMode::Blend,
                                    ..default()
                                })),
                            )),
                            scale: Vec3::new(image.width() as f32, image.height() as f32, 1.)
                                / PIXELS_PER_METER,
                        }
                    } else if let Ok(asset) = id.clone().try_typed::<GifAsset>() {
                        let image = gifs.get(&asset).expect("asset to exist");
                        let scale = image
                            .frames
                            .first()
                            .map(|i| Vec2::new(i.width as f32, i.height as f32))
                            .unwrap_or(Vec2::splat(100.));

                        CommonProps {
                            commands: commands.spawn((
                                Gif3d {
                                    handle: asset.clone(),
                                },
                                Mesh3d(default_meshes.rect.clone()),
                                MeshMaterial3d(materials.add(StandardMaterial {
                                    base_color: Color::Srgba(Srgba::new(1., 1., 1., msg.opacity)),
                                    unlit: true,
                                    alpha_mode: AlphaMode::Blend,
                                    ..default()
                                })),
                            )),
                            scale: scale.extend(1.) / PIXELS_PER_METER,
                        }
                    } else {
                        continue;
                    }
                }
                // Not handled here
                ObjectType::Video(_) => continue,
                ObjectType::Shader(_) => continue,
            };

            common
                .commands
                .entry::<Transform>()
                .or_default()
                .and_modify(move |mut t| *t = t.with_scale(common.scale));

            lifecycle::insert_components(&mut common.commands, &msg);
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
    _assets: ResMut<AssetServer>,
    mut new: MessageReader<ObjectMessage>,
    _pendnig: ResMut<PendingAssets>,
    mut images: ResMut<Assets<Image>>,
    default_meshes: Res<DefaultMeshes>,
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
                    base_color: Color::WHITE.with_alpha(new.opacity),
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                })),
                Visibility::Hidden,
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

        *vis = Visibility::Visible;

        let mut commands = commands.entity(id);
        commands.remove::<VideoDeferredObjectMessage>();

        let target = images.get(target.handle.id()).unwrap();

        let scale = Vec3::new(target.width() as f32, target.height() as f32, 1.) / PIXELS_PER_METER;

        commands
            .entry::<Transform>()
            .or_default()
            .and_modify(move |mut t| *t = t.with_scale(scale));

        lifecycle::insert_components(&mut commands, &msg.0);
    }

    Ok(())
}

fn spawn_shaders(
    mut commands: Commands,
    mut new: MessageReader<ObjectMessage>,
    default_meshes: Res<DefaultMeshes>,
    mut shaders: ResMut<Assets<Shader>>,
    mut shader_material: ResMut<Assets<DynamicMaterial>>,
) {
    for new in new.read() {
        let ObjectType::Shader(custom_shader_source) = &new.kind else {
            continue;
        };

        let shader_asset = Shader::from_wgsl(
            SHADER_TEMPLATE.replace("###CodeHere###", &custom_shader_source.code),
            "runtime_extension.wgsl",
        );

        let shader = shaders.add(shader_asset);
        let dyn_shader = shader_material.add(DynamicMaterial::new(new.opacity, shader));

        let mut commands = commands.spawn((
            MeshMaterial3d(dyn_shader.clone()),
            Mesh3d(default_meshes.rect.clone()),
            ShaderRepeatPeriod::new(custom_shader_source.duraton),
            Transform::from_scale(Vec3::ONE),
        ));

        lifecycle::insert_components(&mut commands, new);
    }
}
