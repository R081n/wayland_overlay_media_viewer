use std::ops::{Add, Div};

use bevy::{
    math::I64Vec2,
    prelude::*,
};
use bevy_rand::{global::GlobalRng, prelude::WyRand};
use rand::RngExt;

use crate::{
    position::{PopupPosition, ScreenPosition, PIXELS_PER_METER},
    spawner::{ObjectMessage, TargetOpacity},
};

pub fn insert_components(commands: &mut EntityCommands<'_>, msg: ObjectMessage) {
    match msg.position {
        PopupPosition::Global(vec3) => {
            commands.insert(Transform::from_translation(vec3));
        }
        PopupPosition::SceenSpace(_, _vec2) => todo!(),
        PopupPosition::Random => {
            commands.trigger(PlaceAtRandomPosition);
        }
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
        app.add_systems(Update, handle_slide_fade_in)
            .add_observer(handle_random_position);
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

#[derive(EntityEvent)]
struct PlaceAtRandomPosition(Entity);

fn handle_random_position(
    trigger: On<PlaceAtRandomPosition>,
    screens: Query<&ScreenPosition>,
    mut objects: Query<&mut Transform>,
    mut rng: Single<&mut WyRand, With<GlobalRng>>,
) -> Result<(), BevyError> {
    let mut obj = objects.get_mut(trigger.0)?;

    let pixels_per_meter = PIXELS_PER_METER as f64;

    // calculations in pixels so that every pixel can be hit
    let mut image_size = (obj.scale.xy() * PIXELS_PER_METER).as_u64vec2();

    // The total amount of possible pixel positions for this image
    let total = screens
        .iter()
        .map(|s| s.pixel_size.element_product())
        .sum::<u64>();

    let mut rand = rng.random_range(..total);
    let z = rng.random_range(..(1e10 as u64)) as f32 / 1e12;

    for screen in screens.iter() {
        let rect = screen.pixel_size;

        if image_size.x > rect.x {
            image_size.x = 0;
        }

        if image_size.y > rect.y {
            image_size.y = 0;
        }

        let size = rect.element_product();

        if size < rand {
            rand -= size;
            continue;
        }

        let x = rand / rect.y;
        let y = rand % rect.y;

        // Make the chance a pixel is hit uniform for better coverage
        let x_range = (x.saturating_sub(image_size.x))..=((rect.x - image_size.x).min(x));
        let y_range = (y.saturating_sub(image_size.y))..=((rect.y - image_size.y).min(y));

        let x = rng.random_range(x_range);
        let y = rng.random_range(y_range);

        let pos = ((I64Vec2::new(x as i64, y as i64) + dbg!(screen.pixel_min))
            .as_dvec2()
            .add(image_size.div(2).as_dvec2())
            / pixels_per_meter)
            .as_vec2();
        obj.translation.x = pos.x;
        obj.translation.y = pos.y;
        obj.translation.z = z;

        break;
    }

    Ok(())
}
