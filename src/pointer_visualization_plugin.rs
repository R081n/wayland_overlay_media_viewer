use bevy::{picking::pointer::PointerLocation, prelude::*};

pub struct PointerVisualizerPlugin;

impl Plugin for PointerVisualizerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw_pointer_gizmos);
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
