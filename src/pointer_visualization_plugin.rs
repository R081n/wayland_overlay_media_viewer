use bevy::{
    picking::{
        backend::{ray::RayMap, PointerHits},
        pointer::{PointerInteraction, PointerLocation, PointerPress},
    },
    prelude::*,
};

pub struct PointerVisualizerPlugin;

impl Plugin for PointerVisualizerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (draw_pointer_gizmos, log_internal_pointer_state));
    }
}

fn draw_pointer_gizmos(
    mut gizmos: Gizmos,
    // 1. Pointers are entities now; query their PointerLocation component directly
    pointer_query: Query<&PointerLocation>,
    // Query all cameras to map coordinates into their viewports
    camera_query: Query<(&Camera, &GlobalTransform)>,
) {
    // Modern color allocations via explicitly declared color spaces
    let cyan = Color::Srgba(Srgba::new(0.0, 1.0, 1.0, 1.0));
    let red = Color::Srgba(Srgba::new(1.0, 0.0, 0.0, 1.0));
    let green = Color::Srgba(Srgba::new(0.0, 1.0, 0.0, 1.0));
    let blue = Color::Srgba(Srgba::new(0.0, 0.0, 1.0, 1.0));
    let yellow_alpha = Color::Srgba(Srgba::new(1.0, 1.0, 0.0, 0.2));

    // 2. Iterate through all active pointer entities
    for pointer_location in &pointer_query {
        // Some pointers may have undefined positions on the first frame
        let Some(position) = pointer_location.location() else {
            continue;
        };

        // 3. Loop through every active camera viewport to project the ray
        for (camera, camera_transform) in &camera_query {
            if !camera.is_active {
                continue;
            }

            // 4. Project the ray using the pointer's logical screen position
            if let Ok(ray) = camera.viewport_to_world(camera_transform, position.position) {
                // Static distance fallback in front of the camera viewport
                let target_depth = 5.0;
                let gizmo_position = ray.origin + ray.direction * target_depth;

                // 5. Paint a sphere indicator where Bevy projects the pointer
                gizmos.sphere(gizmo_position, 0.15, cyan);

                // 6. Draw 3D crosshairs matching the modern Srgba color design
                gizmos.ray(gizmo_position, Vec3::X * 0.5, red);
                gizmos.ray(gizmo_position, Vec3::Y * 0.5, green);
                gizmos.ray(gizmo_position, Vec3::Z * 0.5, blue);

                // 7. Draw a tracer line pointing back to the viewing camera
                gizmos.line(
                    ray.origin + ray.direction * 0.2,
                    gizmo_position,
                    yellow_alpha,
                );
            }
        }
    }
}

fn log_internal_pointer_state(
    // Wir fragen das Entity selbst ab, um es im Log identifizieren zu können
    pointer_query: Query<(
        Entity,
        &PointerLocation,
        Option<&PointerPress>,
        Option<&PointerInteraction>,
    )>,

    mut hits: MessageReader<PointerHits>,
) {
    for (entity, location, press_state, interaction) in &pointer_query {
        let pos_str = match location.location() {
            Some(pos) => format!("X: {:.1}, Y: {:.1}", pos.position.x, pos.position.y),
            None => "No Position (Outside Window/Not Initialized)".to_string(),
        };

        let press_str = match press_state {
            Some(press) => {
                let mut pressed_buttons = Vec::new();

                if press.is_primary_pressed() {
                    pressed_buttons.push("Primary");
                }
                if press.is_secondary_pressed() {
                    pressed_buttons.push("Secondary");
                }
                if press.is_middle_pressed() {
                    pressed_buttons.push("Middle");
                }
                if pressed_buttons.is_empty() {
                    "None".to_string()
                } else {
                    format!("Pressing: [{}]", pressed_buttons.join(", "))
                }
            }
            None => "No PointerPress Component Attached".to_string(),
        };

        let interaction_str = match interaction {
            Some(interact) => {
                let hovered: Vec<String> = interact
                    .iter()
                    .map(|(entity, _info)| format!("{:?}", entity))
                    .collect();
                if hovered.is_empty() {
                    "Hovering: Nothing".to_string()
                } else {
                    format!("Hovering Entities: [{}]", hovered.join(", "))
                }
            }
            None => "No PointerInteraction Component Attached".to_string(),
        };

        info!(
            target: "bevy_picking_debug",
            "Pointer {:?} -> Position: [{}], State: {}, {}",
            entity, pos_str, press_str, interaction_str
        );
    }
}
