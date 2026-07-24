use bevy::{
    camera::{NormalizedRenderTarget, RenderTarget},
    picking::{
        backend::{
            ray::{RayId, RayMap},
            HitData, PointerHits,
        },
        pointer::{PointerButton, PointerId, PointerLocation},
        PickingSystems,
    },
    prelude::*,
};

use crate::{
    draw_order::DrawOrder,
    lifecycle::{get_new_topmost_id, Closing},
};

pub struct PopupInteractionPlugin;

impl Plugin for PopupInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
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
            .add_observer(on_right_drag_end);
    }
}

// --- Main World Components ---

/// Tracks state parameters while an item is actively manipulated by a specific pointer.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct RightDragging {
    pub pointer_id: PointerId,
    pub camera_entity: Entity,
    pub initial_offset: Vec3,
}
// --- Observers (Bevy 0.19 Native Interaction Architecture) ---

/// Listens globally for drag actions to filter for Right Mouse clicks and stores the initial offset.
fn on_right_drag_start(
    trigger: On<Pointer<DragStart>>,
    mut commands: Commands,
    query: Query<&Transform>,
) {
    let event = trigger.event();

    if event.button == PointerButton::Secondary {
        // Fetch the object's starting translation to calculate the delta
        let object_center = query
            .get(trigger.entity)
            .map(|t| t.translation)
            .unwrap_or(Vec3::ZERO);

        // Fetch where the cursor ray physically intersected the mesh surface.
        // Fallback to the object's center if the hit position is somehow undefined.
        let hit_point = event.hit.position.unwrap_or(object_center);

        // Calculate the relative vector offset
        let initial_offset = object_center - hit_point;

        commands.entity(trigger.entity).insert((
            RightDragging {
                pointer_id: event.pointer_id,
                camera_entity: event.hit.camera,
                initial_offset,
            },
            get_new_topmost_id(),
        ));
    }
}

/// Listens globally for continuous cursor tracking movements, preserving the original hit offset.
fn on_right_dragging(
    trigger: On<Pointer<Drag>>,
    // TODO render order,
    mut dragged_entities: Query<(&mut Transform, &RightDragging)>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
) {
    let event = trigger.event();

    let Ok((mut transform, dragging)) = dragged_entities.get_mut(trigger.entity) else {
        return;
    };

    if event.pointer_id != dragging.pointer_id {
        return;
    }

    let Ok((camera, camera_transform)) = camera_query.get(dragging.camera_entity) else {
        return;
    };

    // Use the stored offset to locate the virtual plane depth where the drag loop was initiated
    let original_object_center = transform.translation;
    let original_hit_point = original_object_center - dragging.initial_offset;

    if let Ok(ray) = camera.viewport_to_world(camera_transform, event.pointer_location.position) {
        let camera_forward = camera_transform.forward();
        let denominator = ray.direction.dot(*camera_forward);

        if denominator.abs() > f32::EPSILON {
            // Find where the ray intersects the virtual flat plane passing through our original hit point
            let distance = (original_hit_point - ray.origin).dot(*camera_forward) / denominator;
            let current_ray_intersection = ray.origin + ray.direction * distance;

            // 💡 APPLY THE OFFSET: Reconstruct the new object center relative to where the cursor is now
            transform.translation = current_ray_intersection + dragging.initial_offset;
        }
    }
}

/// Cleans up state assignments when the user terminates the drag event.
fn on_right_drag_end(
    trigger: On<Pointer<DragEnd>>,
    mut commands: Commands,
    dragged_entities: Query<&RightDragging>,
) {
    let event = trigger.event();

    if let Ok(dragging) = dragged_entities.get(trigger.entity) {
        if event.pointer_id == dragging.pointer_id {
            commands.entity(trigger.entity).remove::<RightDragging>();
        }
    }
}

fn on_left_click(trigger: On<Pointer<Click>>, mut commands: Commands) {
    let event = trigger.event();

    if event.button != PointerButton::Primary {
        return;
    }

    commands.entity(event.entity).insert(Closing::default());
}

fn populate_ray_map_manually(
    pointer_query: Query<(Entity, &PointerLocation)>,
    camera_query: Query<(Entity, &Camera, &GlobalTransform, &RenderTarget)>,
    // Fetch the mutable RayMap framework resource
    mut ray_map: ResMut<RayMap>,
) {
    for (pointer_entity, pointer_location) in &pointer_query {
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
    render_order: Query<&DrawOrder>,
) {
    let mut max = i64::MIN;
    let mut closest = None;
    let mut fresh_hits = Vec::new();

    // Iterate through all custom rays currently registered in the map
    for (ray_id, ray) in ray_map.iter() {
        if ray_id.pointer == PointerId::Mouse {
            // Execute the structural geometric raycast against the world scene elements
            let hits = ray_cast.cast_ray(*ray, &MeshRayCastSettings::default().never_early_exit());
            dbg!(hits.len());

            for (entity, hit) in hits {
                // Map the hit data back into a format Bevy's interaction observers understand
                let order = render_order.get(*entity).map(|d| d.0).unwrap_or_default();
                if order > max {
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
