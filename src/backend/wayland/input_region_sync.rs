use bevy::{camera::primitives::Aabb, prelude::*, window::RawHandleWrapper};
use raw_window_handle::{RawWindowHandle, WaylandWindowHandle};
use wayland_client::{
    Proxy,
    protocol::{
        wl_compositor, wl_region,
        wl_surface::{self, WlSurface},
    },
};

use crate::{
    backend::wayland::{
        WaylandAppState,
        backend::{WaylandEventQueue, wayland_event_system},
    },
    position::ScreenPosition,
};

pub struct LayerShellInputPlugin;

impl Plugin for LayerShellInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            update_layer_shell_input_regions.after(wayland_event_system),
        );
    }
}

#[derive(Component, Debug, Clone, Default)]
pub struct RequestInputRecalc(bool);

impl RequestInputRecalc {
    pub fn request(&mut self) {
        self.0 = true;
    }
}

#[derive(Component, Debug, Clone, Default)]
pub struct Clickable;

fn update_layer_shell_input_regions(
    event_queue: NonSendMut<WaylandEventQueue>,
    mut cameras: Query<(
        &Camera,
        &GlobalTransform,
        &ScreenPosition,
        &mut RequestInputRecalc,
    )>,
    // Query all objects that should catch pointer/mouse inputs
    clickable_objects: Query<(&Aabb, &GlobalTransform), With<Clickable>>,
    app_state: NonSendMut<WaylandAppState>,
) -> Result<(), BevyError> {
    if !app_state.is_running() {
        return Ok(());
    }
    // Iterate through every active camera output layer
    for (camera, camera_transform, position, mut request) in &mut cameras {
        // Skip inactive or uninitialized cameras
        if !camera.is_active {
            //|| !request.0 {
            continue;
        }

        request.0 = false;

        let surface_info = app_state
            .surfaces
            .get(&position.output)
            .ok_or("Surface does not exist")?;

        let surface: &WlSurface = &surface_info.surface;
        let compositor = &app_state.compositor.as_ref().ok_or("No compositor")?.0;

        // Create a fresh Wayland region for this surface layer
        let qh = event_queue.handle();
        let region = compositor.create_region(&qh, ());

        // Project every visible clickable object into this camera's view space
        for (aabb, mesh_transform) in &clickable_objects {
            // Find the screen-space bounding box for the 3D AABB
            if let Some(rect) =
                calculate_screen_rect(camera, camera_transform, aabb, mesh_transform)
            {
                // Wayland coordinates use logical pixels, matching Bevy's screen space values.
                // region.add takes: x, y, width, height
                region.add(
                    rect.min.x as i32,
                    rect.min.y as i32,
                    rect.width() as i32,
                    rect.height() as i32,
                );
            }
        }

        // Apply the composited click-region layout to the surface
        surface.set_input_region(Some(&region));

        // Wayland surfaces require a state commit to flush changes down to the server
        surface.commit();

        // Clean up the client-side region proxy allocation
        region.destroy();
    }

    Ok(())
}

fn calculate_screen_rect(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    aabb: &Aabb,
    mesh_transform: &GlobalTransform,
) -> Option<Rect> {
    // 1. Get the 8 corners of the 3D bounding box in local mesh space
    let center = Vec3::from(aabb.center);
    let half = Vec3::from(aabb.half_extents);

    let corners = [
        center + Vec3::new(half.x, half.y, half.z),
        center + Vec3::new(half.x, half.y, -half.z),
        center + Vec3::new(half.x, -half.y, half.z),
        center + Vec3::new(half.x, -half.y, -half.z),
        center + Vec3::new(-half.x, half.y, half.z),
        center + Vec3::new(-half.x, half.y, -half.z),
        center + Vec3::new(-half.x, -half.y, half.z),
        center + Vec3::new(-half.x, -half.y, -half.z),
    ];

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    let mut any_visible = false;

    // 2. Project each corner to world space, then convert to 2D Screen Space
    for corner in corners {
        let world_pos = mesh_transform.transform_point(corner);

        // Projects 3D coordinate cleanly into logical viewport positions (x, y)
        if let Ok(viewport_pos) = camera.world_to_viewport(camera_transform, world_pos) {
            min_x = min_x.min(viewport_pos.x);
            max_x = max_x.max(viewport_pos.x);
            min_y = min_y.min(viewport_pos.y);
            max_y = max_y.max(viewport_pos.y);
            any_visible = true;
        }
    }

    if any_visible {
        Some(Rect::new(min_x, min_y, max_x, max_y))
    } else {
        None
    }
}
