use bevy::prelude::*;

use crate::videos::plugin::VideoPlayer;

pub struct CleanupPlugin;

impl Plugin for CleanupPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, remove_faulted_viedos);
    }
}

fn remove_faulted_viedos(mut commands: Commands, videos: Query<(Entity, &VideoPlayer)>) {
    for (id, player) in videos.iter() {
        if player
            .pipeline
            .as_ref()
            .is_some_and(|p| p.faulted.load(std::sync::atomic::Ordering::Relaxed))
        {
            commands.entity(id).despawn();
        }
    }
}
