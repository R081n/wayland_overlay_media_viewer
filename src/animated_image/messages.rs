use bevy::{asset::Handle, ecs::message::Message};

use crate::animated_image::GifAsset;

#[derive(Message)]
pub(crate) struct GifDespawnMessage(pub Handle<GifAsset>);
