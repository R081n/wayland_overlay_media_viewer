use std::time::Duration;

use bevy::{
    camera::{NormalizedRenderTarget, RenderTarget},
    picking::{
        PickingSystems,
        backend::{
            HitData, PointerHits,
            ray::{RayId, RayMap},
        },
        pointer::{PointerButton, PointerId, PointerLocation},
    },
    prelude::*,
};

use crate::{
    Clickable, RequestInputRecalc,
    draw_order::DrawOrder,
    input::{MediaSource, StartMediaDrag},
    lifecycle::{CloseOnClick, Closing, Draggable, get_new_topmost_id},
    spawner::PopupLayer,
};

pub struct PopupInteractionPlugin;

impl Plugin for PopupInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(First, clear_drag_speed).add_systems(
            First,
            evaluate_hits_manually_fallback
                // Run after our manual RayMap calculation step finishes
                .after(populate_ray_map_manually)
                .before(bevy::picking::PickingSystems::Input),
        );
        app.add_systems(
            First,
            populate_ray_map_manually
                // Execute AFTER Bevy's native pipeline clears/populates the map...
                .after(RayMap::repopulate)
                // ...but BEFORE any downstream raycasting backends try to read it!
                .before(PickingSystems::Backend),
        );

        app
            // We register standard global observers for clean event propagation
            .add_observer(on_left_click)
            .add_observer(on_right_drag_start)
            .add_observer(on_right_dragging)
            .add_observer(on_left_drag_start)
            .add_observer(on_right_drag_end);
    }
}

// --- Main World Components ---

pub type NotCurrentlyDragging = (Without<RightDragging>, Without<RightResizing>);
pub type CurrentlyDragging = (RightDragging, RightResizing);

/// Tracks state parameters while an item is actively manipulated by a specific pointer.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct RightDragging {
    pub pointer_id: PointerId,
    pub camera_entity: Entity,
    pub initial_offset: Vec3,
    pub drag_speed: Vec2,
}
/// State stored during a resize operation.
#[derive(Component)]
pub struct RightResizing {
    pub pointer_id: PointerId,
    pub camera_entity: Entity,
    pub initial_scale: Vec3,
    pub initial_hit_local: Vec3,
    pub object_size: Vec2,
    pub initial_translation: Vec3,
    /// The static 3D world position of the top-left corner when the drag started
    pub top_left_world_anchor: Vec3,
    /// The initial vector from the top-left anchor to the initial click point
    pub initial_anchor_to_hit_distance: f32,
}

/// Determines if the click occurred in the bottom-right corner of the entity.
fn is_bottom_right_corner(local_hit: Vec3, size: Vec2) -> bool {
    let half_width = size.x / 2.0;
    let half_height = size.y / 2.0;

    // Bottom-right in Bevy 2D coordinates (Positive X, Negative Y)
    let target_x = half_width;
    let target_y = -half_height;

    // Define a grab threshold (e.g., within 20% of the object's total size)
    let threshold_x = size.x * 0.2;
    let threshold_y = size.y * 0.2;

    (local_hit.x - target_x).abs() < threshold_x && (local_hit.y - target_y).abs() < threshold_y
}

fn clear_drag_speed(query: Query<&mut RightDragging>) {
    for mut drag in query {
        drag.drag_speed = Vec2::ZERO;
    }
}

/// Listens globally for drag actions to filter for Right Mouse clicks, branching into drag or resize.
fn on_right_drag_start(
    trigger: On<Pointer<DragStart>>,
    mut commands: Commands,
    query: Query<(&Transform, &PopupLayer), With<Draggable>>, // The base mesh is always the same 1x1 recangle
) {
    let event = trigger.event();
    if event.button != PointerButton::Secondary {
        return;
    }

    let Ok((transform, layer)) = query.get(trigger.entity) else {
        return;
    };

    let object_center = transform.translation;
    let hit_point = event.hit.position.unwrap_or(object_center);

    // Transform the world hit position into the object's local coordinate space

    let local_hit = transform.to_matrix().inverse().transform_point3(hit_point);

    let object_size = Vec2::ONE;

    if is_bottom_right_corner(local_hit, object_size) {
        let top_left_world_anchor = transform.translation
            + transform.scale * Vec3::new(-0.5, 0.5, 0.0) * object_size.extend(1.);
        let initial_anchor_to_hit_distance = top_left_world_anchor.distance(hit_point);

        // Initialize Resizing State
        commands.entity(trigger.entity).insert(RightResizing {
            pointer_id: event.pointer_id,
            camera_entity: event.hit.camera,
            initial_scale: transform.scale,
            initial_hit_local: local_hit,
            object_size,
            initial_translation: transform.translation,
            initial_anchor_to_hit_distance,
            top_left_world_anchor,
        });
    } else {
        // Initialize Normal Dragging State
        let initial_offset = object_center - hit_point;
        commands.entity(trigger.entity).insert((
            RightDragging {
                pointer_id: event.pointer_id,
                camera_entity: event.hit.camera,
                initial_offset,
                drag_speed: Vec2::ZERO,
            },
            get_new_topmost_id(*layer),
        ));
    }
}

/// Handles continuous cursor tracking for both dragging and resizing behaviors.
fn on_right_dragging(
    trigger: On<Pointer<Drag>>,
    mut dragged_entities: Query<(
        &mut Transform,
        Option<&mut RightDragging>,
        Option<&RightResizing>,
    )>,
    time: Res<Time>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mut input_recalc: ResMut<RequestInputRecalc>,
) {
    let event = trigger.event();
    let Ok((mut transform, dragging, resizing)) = dragged_entities.get_mut(trigger.entity) else {
        return;
    };

    // BRANCH 1: Resize Behavior
    if let Some(resize) = resizing {
        if event.pointer_id != resize.pointer_id {
            return;
        }
        let Ok((camera, camera_transform)) = camera_query.get(resize.camera_entity) else {
            return;
        };

        if let Ok(ray) = camera.viewport_to_world(camera_transform, event.pointer_location.position)
        {
            let camera_forward = camera_transform.forward();
            let denominator = ray.direction.dot(*camera_forward);

            if denominator.abs() > f32::EPSILON {
                // Find where the ray intersects the virtual flat plane matching our anchor depth
                let distance =
                    (resize.top_left_world_anchor - ray.origin).dot(*camera_forward) / denominator;
                let current_ray_intersection = ray.origin + ray.direction * distance;

                // Calculate current distance from the immutable top-left anchor to the cursor
                let current_distance = resize
                    .top_left_world_anchor
                    .distance(current_ray_intersection);

                // Scale multiplier is directly proportional to how much further away the cursor is
                let scale_factor = current_distance / resize.initial_anchor_to_hit_distance;

                // Clamping threshold to prevent inversion loops
                if scale_factor > 0.05 {
                    // 1. Uniformly scale based on the distance ratio
                    let new_scale =
                        resize.initial_scale * Vec3::new(scale_factor, scale_factor, 1.0);

                    // 2. Calculate the original offset vector from the anchor to the initial center
                    let initial_anchor_to_center =
                        resize.initial_translation - resize.top_left_world_anchor;

                    // 3. Scale that offset vector by our scale factor
                    let current_anchor_to_center = initial_anchor_to_center * scale_factor;

                    // 4. Update both variables simultaneously
                    // The new center moves dynamically relative to the completely frozen top-left corner
                    transform.scale = new_scale;
                    transform.translation = resize.top_left_world_anchor + current_anchor_to_center;
                    input_recalc.request();
                }
            }
        }
        return;
    }

    // BRANCH 2: Drag Behavior
    if let Some(mut drag) = dragging {
        if event.pointer_id != drag.pointer_id {
            return;
        }
        let Ok((camera, camera_transform)) = camera_query.get(drag.camera_entity) else {
            return;
        };

        let original_object_center = transform.translation;
        let original_hit_point = original_object_center - drag.initial_offset;

        if let Ok(ray) = camera.viewport_to_world(camera_transform, event.pointer_location.position)
        {
            let camera_forward = camera_transform.forward();
            let denominator = ray.direction.dot(*camera_forward);

            if denominator.abs() > f32::EPSILON {
                let distance = (original_hit_point - ray.origin).dot(*camera_forward) / denominator;
                let current_ray_intersection = ray.origin + ray.direction * distance;
                let last = transform.translation;
                transform.translation = current_ray_intersection + drag.initial_offset;
                drag.drag_speed = (transform.translation - last).xy() / time.delta_secs();
                input_recalc.request();
            }
        }
    }
}

/// Cleans up any active interaction states.
fn on_right_drag_end(trigger: On<Pointer<DragEnd>>, mut commands: Commands) {
    commands
        .entity(trigger.entity)
        .remove::<RightDragging>()
        .remove::<RightResizing>();
}

fn on_left_click(
    trigger: On<Pointer<Click>>,
    mut commands: Commands,
    query: Query<Has<CloseOnClick>>,
) {
    let event = trigger.event();

    if trigger.duration > Duration::from_millis(200) {
        return;
    }

    if event.button != PointerButton::Primary || !query.get(event.entity).unwrap_or_default() {
        return;
    }

    commands.entity(event.entity).insert(Closing);
}

fn on_left_drag_start(
    trigger: On<Pointer<DragStart>>,
    mut commands: Commands,
    query: Query<&MediaSource>,
) {
    if trigger.button != PointerButton::Primary {
        return;
    }
    let Ok(source) = query.get(trigger.entity) else {
        return;
    };

    commands
        .entity(trigger.entity)
        .trigger(|entity| StartMediaDrag {
            entity,
            source: source.clone(),
        });
}

fn populate_ray_map_manually(
    pointer_query: Query<(Entity, &PointerLocation)>,
    camera_query: Query<(Entity, &Camera, &GlobalTransform, &RenderTarget)>,
    // Fetch the mutable RayMap framework resource
    mut ray_map: ResMut<RayMap>,
) {
    for (_pointer_entity, pointer_location) in &pointer_query {
        // Confirm the pointer carries active window coordinates

        let Some(location) = pointer_location.location() else {
            continue;
        };

        let NormalizedRenderTarget::Image(pointer_target) = &location.target else {
            continue;
        };

        for (cam_entity, camera, camera_transform, target) in &camera_query {
            let RenderTarget::Image(target) = target else {
                continue;
            };

            if !camera.is_active || target.handle != pointer_target.handle {
                continue;
            }

            // Project the 2D cursor into a valid 3D Ray3d space through the camera matrix
            if let Ok(ray) = camera.viewport_to_world(camera_transform, location.position) {
                // Construct a unique identifier pairing this specific cursor entity to this camera viewport
                let ray_id = RayId::new(cam_entity, PointerId::Mouse);

                // Insert the generated ray directly into Bevy's core mesh targeting map
                ray_map.map.insert(ray_id, ray);
            }
        }
    }
}

/// A fallback system that bypasses Bevy's default mesh picking logic to explicitly
/// calculate pointer hits against items in the scene.
fn evaluate_hits_manually_fallback(
    ray_map: Res<RayMap>,
    mut ray_cast: MeshRayCast,
    mut hits_writer: MessageWriter<PointerHits>,
    render_order: Query<&DrawOrder, With<Clickable>>,
) {
    let mut max = i64::MIN;
    let mut closest = None;
    let mut fresh_hits = Vec::new();

    // Iterate through all custom rays currently registered in the map
    for (ray_id, ray) in ray_map.iter() {
        if ray_id.pointer == PointerId::Mouse {
            // Execute the structural geometric raycast against the world scene elements
            let hits = ray_cast.cast_ray(*ray, &MeshRayCastSettings::default().never_early_exit());

            for (entity, hit) in hits {
                // Map the hit data back into a format Bevy's interaction observers understand
                if let Ok(order) = render_order.get(*entity).map(|d| d.0)
                    && order > max
                {
                    closest = Some((
                        *entity,
                        HitData {
                            depth: hit.distance,
                            position: Some(hit.point),
                            normal: Some(hit.normal),
                            camera: ray_id.camera,
                            extra: None,
                        },
                    ));
                    max = order;
                }
            }
        }
    }

    if let Some(closest) = closest {
        fresh_hits.push(closest);
    }

    hits_writer.write(PointerHits {
        pointer: PointerId::Mouse,
        picks: fresh_hits,
        order: 10.0,
    });
}
