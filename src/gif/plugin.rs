use bevy::prelude::*;

use crate::gif::{
    GifAsset,
    components::AnimationImageLoader,
    messages::GifDespawnMessage,
    systems::{animate_gifs, despawn_gifs, initialize_gifs},
};

pub struct GifPlugin;

impl Plugin for GifPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<GifAsset>();
        app.add_message::<GifDespawnMessage>();
        app.init_asset_loader::<AnimationImageLoader>();
        app.add_systems(
            Update,
            (initialize_gifs, animate_gifs, despawn_gifs).chain(),
        );
    }
}
