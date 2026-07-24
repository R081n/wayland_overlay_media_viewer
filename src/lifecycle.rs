use std::{
    ops::{Add, Div},
    sync::atomic::AtomicI64,
    time::Duration,
};

use bevy::{math::I64Vec2, prelude::*};
use bevy_rand::{global::GlobalRng, prelude::WyRand};
use rand::RngExt;

use crate::{
    draw_order::DrawOrder,
    position::{PopupPosition, ScreenPosition, PIXELS_PER_METER},
    spawner::{ObjectMessage, PopupOutAnimation, TargetOpacity},
    Clickable,
};
static NEXT_ID: AtomicI64 = AtomicI64::new(0);

pub fn get_new_topmost_id() -> DrawOrder {
    DrawOrder(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

pub fn insert_components(commands: &mut EntityCommands<'_>, msg: ObjectMessage) {
    match msg.position {
        PopupPosition::Global(vec3) => {
            commands
                .entry::<Transform>()
                .or_default()
                .and_modify(move |mut t| *t = t.with_translation(vec3));
        }
        PopupPosition::SceenSpace(_, _vec2) => todo!(),
        PopupPosition::Random => {
            commands.trigger(PlaceAtRandomPosition);
        }
    }

    match msg.popup_animation {
        crate::spawner::PopupInAnimation::None => {}
        crate::spawner::PopupInAnimation::SlideFadeIn => {
            commands.insert(SlideFadeInAnimation::new());
        }
    }

    let close = &msg.close_condition;
    if let Some(duration) = close.duration {
        commands.trigger(|entity| CloseInDuration {
            entity,
            duration,
            kind: msg.close_animation,
        });
    }

    match msg.behaviour {
        crate::spawner::PopupInteraction::ClickThough => {}
        crate::spawner::PopupInteraction::Clickable => {
            commands.insert((
                Clickable,
                Pickable {
                    is_hoverable: true,
                    // Order is done via the DrawOrder struct
                    should_block_lower: false,
                },
            ));
        }
    }

    commands.insert((TargetOpacity(msg.opacity), get_new_topmost_id()));
}

pub struct LiveCyclePlugin;

impl Plugin for LiveCyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (handle_slide_fade_in, handle_closing).chain())
            .add_observer(handle_random_position)
            .add_observer(on_close_in);
    }
}

#[derive(Component, Default)]
pub(crate) struct SlideFadeInAnimation {
    progress: f32,
    started: bool,
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

pub(crate) fn handle_slide_fade_in(
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
    const DISTANCE: f32 = 0.2;
    for (id, mut transform, mesh_material3d, mut animation, target_opacity) in objects.iter_mut() {
        if !animation.started {
            transform.scale -= DISTANCE;
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
        transform.scale += diff;

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

        let pos = ((I64Vec2::new(x as i64, y as i64) + screen.pixel_min)
            .as_dvec2()
            .add(image_size.div(2).as_dvec2())
            / pixels_per_meter)
            .as_vec2();
        obj.translation = pos.extend(0.0);

        break;
    }

    Ok(())
}

#[derive(EntityEvent)]
struct CloseInDuration {
    entity: Entity,
    duration: Duration,
    kind: PopupOutAnimation,
}

fn on_close_in(trigger: On<CloseInDuration>, mut commands: Commands) {
    commands
        .delayed()
        .duration(trigger.duration)
        .entity(trigger.entity)
        .try_insert(Closing { kind: trigger.kind });
}

#[derive(Component, Default)]
pub struct Closing {
    kind: PopupOutAnimation,
}

fn handle_closing(
    mut commands: Commands,
    time: Res<Time>,
    mut closing: Query<(Entity, &Closing, &mut MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (id, closing, handle) in closing.iter_mut() {
        match closing.kind {
            PopupOutAnimation::None => commands.entity(id).despawn(),
            PopupOutAnimation::FadeOut => {
                let delta = time.delta_secs();
                if let Some(mut mat) = materials.get_mut(handle.id()) {
                    let mut alpha = mat.base_color.alpha();
                    if alpha < 0.000001 {
                        commands.entity(id).despawn();
                    }

                    alpha.smooth_nudge(&0.0, 1., delta);
                    mat.base_color.set_alpha(alpha);
                }
            }
        }
    }
}
