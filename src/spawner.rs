use std::{f32::consts::PI, path::PathBuf};

use crate::position::{PopupPosition, PIXELS_PER_METER};
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
            .add_systems(Update, (preload_asset, spawn_object).chain());
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
            ObjectType::Video(popup_video) => todo!(),
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
            let mut common = match msg.kind {
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
                ObjectType::Video(popup_video) => todo!(),
            };

            match msg.position {
                PopupPosition::Global(vec3) => {
                    common
                        .commands
                        .insert(Transform::from_translation(vec3).with_scale(common.scale));
                }
                PopupPosition::SceenSpace(_, vec2) => todo!(),
                PopupPosition::Random => todo!(),
            }
        }
    }

    for (id, pending) in pending.pending.extract_if(|id, _| {
        if let LoadState::Failed(failed) = server.load_state(id) {
            eprintln!("Faild to load asset {:?}", failed);
            true
        } else {
            false
        }
    }) {}
}
