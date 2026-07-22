use bevy::prelude::*;

use crate::{
    position::PopupPosition,
    spawner::{ObjectMessage, TargetOpacity},
};

pub fn insert_components(commands: &mut EntityCommands<'_>, msg: ObjectMessage) {
    match msg.position {
        PopupPosition::Global(vec3) => {
            commands.insert(Transform::from_translation(vec3));
        }
        PopupPosition::SceenSpace(_, _vec2) => todo!(),
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

    commands.insert(TargetOpacity(msg.opacity));
}

pub struct LiveCyclePlugin;

impl Plugin for LiveCyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_slide_fade_in);
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
            duration_secs: 0.2,
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
        &TargetOpacity,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    const DISTANCE: f32 = 1.0;
    for (id, mut transform, mesh_material3d, mut animation, target_opacity) in objects.iter_mut() {
        if !animation.started {
            transform.translation.z -= DISTANCE;
            animation.start_depth = transform.translation.z;
            animation.started = true;

            if let Some(mut mat) = materials.get_mut(mesh_material3d.id()) {
                mat.base_color = mat.base_color.to_srgba().with_alpha(0.0).into();
            }
            continue;
        }

        let last = ExponentialOutCurve.sample(animation.progress).unwrap_or(1.) * DISTANCE;
        animation.progress += time.delta_secs() / animation.duration_secs;
        animation.progress = animation.progress.clamp(0., 1.);

        let current_percent = ExponentialOutCurve.sample(animation.progress).unwrap_or(1.);
        let current = current_percent * DISTANCE;
        let diff = current - last;
        transform.translation.z += diff;

        if let Some(mut mat) = materials.get_mut(mesh_material3d.id()) {
            mat.base_color = mat
                .base_color
                .to_srgba()
                .with_alpha(
                    ExponentialOutCurve
                        .sample(animation.progress)
                        .unwrap_or(1.0)
                        * target_opacity.0,
                )
                .into();
        }

        if animation.progress == 1.0 {
            commands.entity(id).remove::<SlideFadeInAnimation>();
        }
    }
}
