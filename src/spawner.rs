use std::{f32::consts::PI, path::PathBuf};

use crate::position::{PopupPosition, PIXELS_PER_METER};
use bevy::{
    asset::{LoadState, UntypedAssetId},
    platform::collections::HashMap,
    prelude::*,
};
use bevy_sprite3d::Sprite3d;

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
            .add_systems(Update, (preload_asset, spawn_object).chain());
    }
}

#[derive(Default, Resource)]
pub struct PendingAssets {
    pending: HashMap<UntypedHandle, Vec<ObjectMessage>>,
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

fn spawn_object(
    mut commands: Commands,
    mut pending: ResMut<PendingAssets>,
    server: ResMut<AssetServer>,
) {
    for (id, pending) in pending
        .pending
        .extract_if(|id, _| server.is_loaded_with_dependencies(id))
    {
        for msg in pending {
            let mut command = match msg.kind {
                ObjectType::Image(popup_image) => {
                    let Ok(asset) = id.clone().try_typed::<Image>() else {
                        continue;
                    };

                    commands.spawn((
                        Sprite {
                            image: asset,
                            ..default()
                        },
                        Sprite3d {
                            pixels_per_metre: PIXELS_PER_METER,
                            unlit: true,
                            alpha_mode: AlphaMode::Blend,
                            double_sided: true,

                            ..default() // pivot: Some(Vec2::new(0.5, 0.5)),
                                        // double_sided: true,
                        },
                    ))
                }
                ObjectType::Video(popup_video) => todo!(),
            };

            match msg.position {
                PopupPosition::Global(vec3) => {
                    command.insert(Transform::from_translation(vec3));
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
