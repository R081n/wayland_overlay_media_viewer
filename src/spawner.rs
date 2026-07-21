use std::path::PathBuf;

use crate::position::PopupPosition;
use bevy::{asset::UntypedAssetId, platform::collections::HashMap, prelude::*};

#[derive(Message)]
pub struct ObjectMessage {
    kind: ObjectType,
    position: PopupPosition,
    popup_animation: PopupAnimation,
    behaviour: PopupInteraction,
}

pub enum ObjectType {
    Image(PopupImage),
    Video(PopupVideo),
    // Text,
    //Shader,
}
pub enum PopupInteraction {
    ClickThough,
}

pub enum PopupAnimation {
    None,
}

pub struct PopupImage {
    uri: PathBuf,
}
pub struct PopupVideo {
    uri: PathBuf,
}

pub struct PopupPlugin;

impl Plugin for PopupPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PendingAssets::default())
            .add_systems(Update, (preload_asset));
    }
}

#[derive(Default, Resource)]
pub struct PendingAssets {
    pending: HashMap<UntypedAssetId, Vec<ObjectMessage>>,
}

fn preload_asset(
    mut assets: ResMut<AssetServer>,
    mut comands: Commands,
    mut new: MessageReader<ObjectMessage>,
) {
    for new in new.read() {
        match &new.kind {
            ObjectType::Image(popup_image) => todo!(),
            ObjectType::Video(popup_video) => todo!(),
        }
    }
}
