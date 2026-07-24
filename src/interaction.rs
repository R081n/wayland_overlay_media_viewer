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
            .add_observer(on_right_drag_start)
            .add_observer(on_right_dragging)
            .add_observer(on_right_drag_end);
    }
}

// --- Main World Components ---

/// Tracks state parameters while an item is actively manipulated by a specific pointer.
#[derive(Component)]
#[component(storage = "SparseSet")] // Highly performant optimization for frequent add/remove cycles
struct RightDragging {
    pub pointer_id: PointerId,
    pub camera_entity: Entity,
}

// --- Observers (Bevy 0.19 Native Interaction Architecture) ---

/// Listens globally for drag actions to filter for Right Mouse clicks.
fn on_right_drag_start(trigger: On<Pointer<DragStart>>, mut commands: Commands) {
    let event = trigger.event();

    // Explicitly enforce isolation so Left Click dragging can handle selection/other logic
    if event.button == PointerButton::Secondary {
        commands.entity(trigger.entity).insert(RightDragging {
            pointer_id: event.pointer_id,
            camera_entity: event.hit.camera,
        });
    }
}

/// Listens globally for continuous cursor tracking movements.
fn on_right_dragging(
    trigger: On<Pointer<Drag>>,
    mut dragged_entities: Query<(&mut Transform, &RightDragging)>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
) {
    let event = trigger.event();

    // Extract targets matching the currently observed entity

    let Ok((mut transform, dragging)) = dragged_entities.get_mut(trigger.entity) else {
        return;
    };

    // Safety checks: ensure the exact pointer matching the initialization layout controls it
    if event.pointer_id != dragging.pointer_id {
        return;
    }

    let Ok((camera, camera_transform)) = camera_query.get(dragging.camera_entity) else {
        return;
    };

    dbg!("teststTt");

    // Project screen pixels directly through the correct camera matrix
    let current_world_pos = transform.translation;

    if let Ok(ray) = camera.viewport_to_world(camera_transform, event.pointer_location.position) {
        // Construct a movement surface parallel to the camera view plane at the mesh depth
        let camera_forward = camera_transform.forward();
        let denominator = ray.direction.dot(*camera_forward);

        if denominator.abs() > f32::EPSILON {
            // Find out exactly where the ray crosses the object's parallel plane depth
            let distance = (current_world_pos - ray.origin).dot(*camera_forward) / denominator;
            let target_world_pos = ray.origin + ray.direction * distance;

            // Re-assign translations instantly
            transform.translation = target_world_pos;
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

fn populate_ray_map_manually(
    pointer_query: Query<(Entity, &PointerLocation)>,
    camera_query: Query<(&Camera, &GlobalTransform, &RenderTarget)>,
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

        for (camera, camera_transform, target) in &camera_query {
            let RenderTarget::Image(target) = target else {
                continue;
            };

            if !camera.is_active || target.handle != pointer_target.handle {
                continue;
            }

            // Project the 2D cursor into a valid 3D Ray3d space through the camera matrix
            if let Ok(ray) = camera.viewport_to_world(camera_transform, location.position) {
                // Construct a unique identifier pairing this specific cursor entity to this camera viewport
                let ray_id = RayId::new(pointer_entity, PointerId::Mouse);

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
    // 💡 High-utility structural parameter that executes geometric ray intersects directly
    mut ray_cast: MeshRayCast,
    mut hits: MessageWriter<PointerHits>,
) {
    // Keep track of our hits for this frame loop
    let mut fresh_hits = Vec::new();

    // Iterate through all custom rays currently registered in the map
    for (ray_id, ray) in ray_map.iter() {
        if ray_id.pointer == PointerId::Mouse {
            // Execute the structural geometric raycast against the world scene elements
            let hits = ray_cast.cast_ray(*ray, &MeshRayCastSettings::default());

            for (entity, hit) in hits {
                // Map the hit data back into a format Bevy's interaction observers understand
                fresh_hits.push((
                    *entity,
                    HitData {
                        depth: hit.distance,
                        position: Some(hit.point),
                        normal: Some(hit.normal),
                        camera: ray_id.camera,
                        extra: None,
                    },
                ));
            }
        }
    }

    hits.write(PointerHits {
        pointer: PointerId::Mouse,
        picks: fresh_hits,
        order: 0.0,
    });
}
