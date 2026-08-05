use std::{
    ops::{Add as _, Div},
    sync::atomic::AtomicI64,
    time::Duration,
};

use bevy::{
    light::NotShadowCaster,
    math::{I64Vec2, VectorSpace},
    prelude::*,
    render::batching::NoAutomaticBatching,
};
use bevy_rand::{global::GlobalRng, prelude::WyRand};
use rand::RngExt;

use crate::{
    draw_order::DrawOrder,
    position::{
        FullScreenMode, PopupPosition, ScreenPosition, ScreenspacePosition, PIXELS_PER_METER,
    },
    spawner::{
        CdBounce, ObjectMessage, PopupLayer, PopupOutAnimation, PopupSize, ProxyOpacity,
        TargetOpacity,
    },
    Clickable, RequestInputRecalc,
};
static NEXT_ID_BELOW: AtomicI64 = AtomicI64::new(i64::MIN);
static NEXT_ID_NORMAL: AtomicI64 = AtomicI64::new(0);
static NEXT_ID_ABOVE: AtomicI64 = AtomicI64::new(i64::MAX / 2);

pub fn get_new_topmost_id(layer: PopupLayer) -> DrawOrder {
    match layer {
        PopupLayer::Below => {
            DrawOrder(NEXT_ID_BELOW.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        }
        PopupLayer::Normal => {
            DrawOrder(NEXT_ID_NORMAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        }
        PopupLayer::Above => {
            DrawOrder(NEXT_ID_ABOVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        }
    }
}

#[derive(Component)]
struct SetPopupVisibleNextFrame;

pub fn insert_components(commands: &mut EntityCommands<'_>, msg: &ObjectMessage) {
    commands.insert((
        // Almost no shared materials
        NoAutomaticBatching,
        // No Shadows
        NotShadowCaster,
        Visibility::Hidden,
        SetPopupVisibleNextFrame,
        msg.layer,
        msg.close_animation,
    ));

    // Needs to happen before placing the popup
    commands.trigger(|entity| SetPopupSize {
        entity,
        size: msg.size,
    });

    match msg.position {
        PopupPosition::Global(vec3) => {
            commands
                .entry::<Transform>()
                .or_default()
                .and_modify(move |mut t| *t = t.with_translation(vec3));
        }
        PopupPosition::SceenSpace(position) => {
            commands.trigger(|entity| PlaceAtScreenspacePosition { entity, position });
        }
        PopupPosition::Random => {
            commands.trigger(PlaceAtRandomPosition);
        }
        PopupPosition::FullScreen {
            screen,
            relative_center,
            mode,
        } => {
            commands.trigger(|entity| PlaceAtFullScreenScreenPos {
                entity,
                screen,
                relative_center,
                mode,
            });
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
        commands.trigger(|entity| CloseInDuration { entity, duration });
    }

    if close.click.is_some() {
        commands.insert(CloseOnClick);
    }

    match msg.behaviour {
        crate::spawner::PopupInteraction::ClickThrough => {}
        crate::spawner::PopupInteraction::Clickable => {
            commands.insert(Clickable);
        }
        crate::spawner::PopupInteraction::Draggable => {
            commands.insert((Draggable, Clickable));
        }
    }

    match msg.movement {
        crate::spawner::Movement::None => {}
        crate::spawner::Movement::Linear(vec2) => {
            commands.insert(LinearMovement(vec2));
        }
        crate::spawner::Movement::CdBounce(cd_bounce) => {
            commands.insert(cd_bounce);
        }
    }

    commands.insert((
        TargetOpacity(msg.opacity),
        ProxyOpacity(msg.opacity),
        get_new_topmost_id(msg.layer),
    ));
}

pub struct LiveCyclePlugin;

impl Plugin for LiveCyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (handle_slide_fade_in, handle_closing).chain())
            //No special ordering
            .add_systems(Update, (handle_moving, handle_cd_bounce).chain())
            .add_systems(First, make_visible_on_the_second_frame)
            .add_observer(handle_random_position)
            .add_observer(place_at_sceen_pos)
            .add_observer(handle_screenspace_position)
            .add_observer(set_popup_size)
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
        Option<&MeshMaterial3d<StandardMaterial>>,
        &mut SlideFadeInAnimation,
        &mut ProxyOpacity,
        &TargetOpacity,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut input_recalc: ResMut<RequestInputRecalc>,
) {
    const DISTANCE: f32 = 0.2;
    for (id, mut transform, mesh_material3d, mut animation, mut proxy_opacity, target_opacity) in
        objects.iter_mut()
    {
        if !animation.started {
            transform.scale -= DISTANCE;
            animation.started = true;

            if let Some(mut mat) = mesh_material3d.and_then(|m| materials.get_mut(m.id())) {
                mat.base_color = mat.base_color.to_srgba().with_alpha(0.0).into();
            }
            proxy_opacity.0 = 0.0;
            continue;
        }

        let last = ExponentialOutCurve.sample(animation.progress).unwrap_or(1.) * DISTANCE;
        animation.progress += time.delta_secs() / animation.duration_secs;
        animation.progress = animation.progress.clamp(0., 1.);

        let current_percent = ExponentialOutCurve.sample(animation.progress).unwrap_or(1.);
        let current = current_percent * DISTANCE;
        let diff = current - last;
        transform.scale += diff;
        input_recalc.request();

        let alpha = ExponentialOutCurve
            .sample(animation.progress)
            .unwrap_or(1.0)
            * target_opacity.0;

        if let Some(mut mat) = mesh_material3d.and_then(|m| materials.get_mut(m.id())) {
            mat.base_color = mat.base_color.to_srgba().with_alpha(alpha).into();
        }

        proxy_opacity.0 = alpha;

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
    mut input_recalc: ResMut<RequestInputRecalc>,
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
        input_recalc.request();

        break;
    }

    Ok(())
}

#[derive(EntityEvent)]
struct PlaceAtScreenspacePosition {
    entity: Entity,
    position: ScreenspacePosition,
}

fn handle_screenspace_position(
    trigger: On<PlaceAtScreenspacePosition>,
    mut objects: Query<&mut Transform>,
    screens: Query<&ScreenPosition>,
    mut input_recalc: ResMut<RequestInputRecalc>,
) -> Result<(), BevyError> {
    let mut obj = objects.get_mut(trigger.entity)?;

    let image_size = obj.scale.xy();

    let position = trigger.position;

    let target = match trigger.position.screen {
        Some(index) => screens
            .iter()
            .find(|s| s.index == index)
            .map(|s| s.rect)
            .ok_or("Screen does not exist")?,
        None => screens
            .iter()
            .map(|s| s.rect)
            .reduce(|a, b| a.union(b))
            .ok_or("No screens")?,
    };

    let x = match position.anchor.horizontal {
        crate::position::HorizontalAnchor::Left => {
            target.min.x + position.position.x / PIXELS_PER_METER + image_size.x / 2.0
        }
        crate::position::HorizontalAnchor::Right => {
            target.max.x - position.position.x / PIXELS_PER_METER - image_size.x / 2.0
        }
    };

    let y = match position.anchor.vertical {
        crate::position::VerticalAnchor::Bottom => {
            target.min.y + position.position.y / PIXELS_PER_METER + image_size.y / 2.0
        }
        crate::position::VerticalAnchor::Top => {
            target.max.y - position.position.y / PIXELS_PER_METER - image_size.y / 2.0
        }
    };

    obj.translation = Vec3::new(x, y, 0.0);

    input_recalc.request();

    Ok(())
}

#[derive(Component, Debug, Clone, Default)]
pub struct Draggable;

#[derive(Component)]
pub struct CloseOnClick;

#[derive(EntityEvent)]
struct CloseInDuration {
    entity: Entity,
    duration: Duration,
}

fn on_close_in(trigger: On<CloseInDuration>, mut commands: Commands) {
    commands
        .delayed()
        .duration(trigger.duration)
        .entity(trigger.entity)
        .try_insert(Closing);
}

#[derive(Component, Default)]
pub struct Closing;

fn handle_closing(
    mut commands: Commands,
    time: Res<Time>,
    mut closing: Query<
        (
            Entity,
            Option<&MeshMaterial3d<StandardMaterial>>,
            &PopupOutAnimation,
            Has<Clickable>,
            &mut ProxyOpacity,
        ),
        With<Closing>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut input_recalc: ResMut<RequestInputRecalc>,
) {
    for (id, handle, animation, clickable, mut proxy_opacity) in closing.iter_mut() {
        if clickable {
            commands.entity(id).remove::<Clickable>();
            input_recalc.request();
        }

        match *animation {
            PopupOutAnimation::None => {
                commands.entity(id).despawn();
                input_recalc.request();
            }
            PopupOutAnimation::FadeOut { decay_rate } => {
                let delta = time.delta_secs();

                if proxy_opacity.0 < 0.000001 {
                    commands.entity(id).despawn();
                    input_recalc.request();
                }

                proxy_opacity.0.smooth_nudge(&0.0, decay_rate, delta);
                if let Some(mut mat) = handle.and_then(|m| materials.get_mut(m.id())) {
                    mat.base_color.set_alpha(proxy_opacity.0);
                }
            }
        }
    }
}

#[derive(EntityEvent)]
struct PlaceAtFullScreenScreenPos {
    entity: Entity,
    screen: u32,
    relative_center: Vec2,
    mode: FullScreenMode,
}

fn place_at_sceen_pos(
    trigger: On<PlaceAtFullScreenScreenPos>,
    mut objects: Query<&mut Transform>,
    screens: Query<&ScreenPosition>,
    mut input_recalc: ResMut<RequestInputRecalc>,
) -> Result<(), BevyError> {
    let screen = screens
        .iter()
        .find(|s| s.index == trigger.screen)
        // Fall back to main screen
        .unwrap_or_else(|| screens.iter().min_by_key(|s| s.index).unwrap());

    let target_rect = match trigger.mode {
        FullScreenMode::One => screen.rect,
        FullScreenMode::All => screens
            .iter()
            .map(|s| s.rect)
            .reduce(|a, b| a.union(b))
            .ok_or("No screens")?,
    };
    let center = screen.rect.center() + trigger.relative_center * screen.rect.size() * 0.5;

    let x_distance = [target_rect.min.x, target_rect.max.x]
        .iter()
        .map(|t| (center.x - t).abs())
        .max_by(f32::total_cmp)
        .unwrap_or(100.0);

    let y_distance = [target_rect.min.y, target_rect.max.y]
        .iter()
        .map(|t| (center.y - t).abs())
        .max_by(f32::total_cmp)
        .unwrap_or(100.0);

    let mut object = objects.get_mut(trigger.entity)?;

    let scale = match trigger.mode {
        FullScreenMode::One => scale_to_fit(object.scale.xy(), screen.rect.size()),
        FullScreenMode::All => scale_to_fit_but_the_other_way(
            object.scale.xy(),
            Vec2::new(x_distance, y_distance) * 2.0,
        ),
    };

    object.translation = center.extend(-1.0);
    object.scale = scale.extend(1.0);
    input_recalc.request();

    Ok(())
}

#[derive(EntityEvent)]
struct SetPopupSize {
    entity: Entity,
    size: PopupSize,
}

fn set_popup_size(
    trigger: On<SetPopupSize>,
    mut objects: Query<&mut Transform>,
    mut input_recalc: ResMut<RequestInputRecalc>,
) -> Result<(), BevyError> {
    let size = trigger.size;

    let mut object = objects.get_mut(trigger.entity)?;

    let size = match size {
        PopupSize::Auto => return Ok(()), // Already calculated by the respective spawner
        PopupSize::ProportionalMax(max) => {
            scale_to_fit(object.scale.xy(), Vec2::splat(max / PIXELS_PER_METER))
        }
        PopupSize::ProportionalMin(min) => {
            scale_to_fit_but_the_other_way(object.scale.xy(), Vec2::splat(min / PIXELS_PER_METER))
        }
        PopupSize::Custom(vec2) => vec2 / PIXELS_PER_METER,
    };

    object.scale = size.extend(1.0);

    input_recalc.request();

    Ok(())
}

fn scale_to_fit(source: Vec2, target: Vec2) -> Vec2 {
    let scale_factors = target / source;

    let max_scale = scale_factors.min_element();

    source * max_scale
}

fn scale_to_fit_but_the_other_way(source: Vec2, target: Vec2) -> Vec2 {
    let scale_factors = target / source;

    let max_scale = scale_factors.max_element();

    source * max_scale
}

fn make_visible_on_the_second_frame(
    mut commands: Commands,
    mut popup: Query<(Entity, &mut Visibility), With<SetPopupVisibleNextFrame>>,
    mut input_recalc: ResMut<RequestInputRecalc>,
) {
    for (id, mut vis) in &mut popup {
        *vis = Visibility::Visible;

        commands.entity(id).remove::<SetPopupVisibleNextFrame>();
        input_recalc.request();
    }
}

#[derive(Component)]
struct LinearMovement(Vec2);
fn handle_moving(
    query: Query<(&mut Transform, &LinearMovement)>,
    time: Res<Time>,

    mut input_recalc: ResMut<RequestInputRecalc>,
) {
    for (mut transform, movement) in query {
        let delta = time.delta_secs();
        transform.translation += movement.0.extend(0.0) * delta;
        input_recalc.request();
    }
}

fn handle_cd_bounce(
    mut commands: Commands,
    query: Query<(Entity, &mut Transform, &mut CdBounce)>,
    time: Res<Time>,
    screens: Query<&ScreenPosition>,
    mut input_recalc: ResMut<RequestInputRecalc>,
    mut rng: Single<&mut WyRand, With<GlobalRng>>,
) {
    for (id, mut transform, mut movement) in query {
        let Some(screen) = screens
            .iter()
            .find(|s| s.rect.contains(transform.translation.xy()))
        else {
            warn!(
                "CdBounce was not inside a screen, choosing random {}",
                transform.translation
            );
            commands.entity(id).trigger(PlaceAtRandomPosition);
            continue;
        };

        let delta = time.delta_secs();
        let movement = &mut *movement;
        transform.translation += movement.speed.extend(0.0) * delta;

        let rect = transform_to_rect(&transform);
        let translation = &mut transform.translation;
        let speed = &mut movement.speed;

        let mut bounced = false;
        for (larger, smaller, bounce_dir) in [
            (rect.max, screen.rect.max, -1.),
            (screen.rect.min, rect.min, 1.),
        ] {
            for (pos, speed, larger, smaller) in [
                (&mut translation.x, &mut speed.x, larger.x, smaller.x),
                (&mut translation.y, &mut speed.y, larger.y, smaller.y),
            ] {
                if larger > smaller {
                    *speed *= -1.;
                    *pos += (larger - smaller) * bounce_dir;
                    bounced = true;
                }
            }
        }

        if bounced {
            let mut angle = speed.to_angle();
            let mut len = speed.length();
            angle += (rng.random::<f32>() - rng.random::<f32>()) * movement.angle_uncertanty;
            len += ((rng.random::<f32>() - rng.random::<f32>()) * movement.speed_uncertanty)
                .clamp(movement.speed_range.0, movement.speed_range.1);

            *speed = Vec2::from_angle(angle) * len;
        }

        input_recalc.request();
    }
}

fn transform_to_rect(transform: &Transform) -> Rect {
    let center = transform.translation.xy();
    let size = transform.scale.xy().max(Vec2::ZERO);
    Rect::from_center_size(center, size)
}
