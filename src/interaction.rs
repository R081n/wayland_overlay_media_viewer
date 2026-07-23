use bevy::{
    picking::pointer::{PointerButton, PointerId},
    prelude::*,
};

pub struct PopupInteractionPlugin;

impl Plugin for PopupInteractionPlugin {
    fn build(&self, app: &mut App) {
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
    dbg!("any");

    // Explicitly enforce isolation so Left Click dragging can handle selection/other logic
    if event.button == PointerButton::Secondary {
        dbg!("down");
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
