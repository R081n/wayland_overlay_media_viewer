use bevy::prelude::*;

use crate::{position::PopupPosition, spawner::ObjectMessage};

pub fn insert_components(commands: &mut EntityCommands<'_>, msg: ObjectMessage) {
    match msg.position {
        PopupPosition::Global(vec3) => {
            commands.insert(Transform::from_translation(vec3));
        }
        PopupPosition::SceenSpace(_, vec2) => todo!(),
        PopupPosition::Random => todo!(),
    }

    match msg.popup_animation {
        crate::spawner::PopupAnimation::None => {}
        crate::spawner::PopupAnimation::SlideFadeIn => {
            commands.insert(SlideFadeInAnimation::new());
        }
    }

    match msg.behaviour {
        crate::spawner::PopupInteraction::ClickThough => {}
    }
}

pub struct LiveCyclePlugin;

impl Plugin for LiveCyclePlugin {
    fn build(&self, app: &mut App) {
        todo!()
    }
}

#[derive(Component, Default)]
struct SlideFadeInAnimation {
    progress: f32,
    started: bool,
    start_depth: f32,
    duration_secs: f32,
}

impl SlideFadeInAnimation {
    fn new() -> Self {
        Self {
            duration_secs: 3.0,
            ..Default::default()
        }
    }
}

fn handle_slide_fade_in(
    mut commands: Commands,
    time: Res<Time>,
    mut objects: Query<(
        Entity,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
        &mut SlideFadeInAnimation,
    )>,
    materials: ResMut<Assets<StandardMaterial>>,
) {
    const DISTANCE: f32 = 1.0;
    for (id, mut transform, mesh_material3d, mut animation) in objects.iter_mut() {
        if !animation.started {
            transform.translation.z -= DISTANCE;
            animation.start_depth = transform.translation.z;

            continue;
        }

        let last = CubicOutCurve.sample(animation.progress).unwrap_or(1.);
        animation.progress += time.delta_secs() / animation.duration_secs;
        animation.progress = animation.progress.clamp(0., 1.);

        let current = CubicOutCurve.sample(animation.progress).unwrap_or(1.);
        let diff = current - last;
        transform.translation.z += diff;

        if animation.progress == 1.0 {
            commands.entity(id).remove::<SlideFadeInAnimation>();
        }
    }
}
