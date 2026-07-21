use bevy::prelude::*;

use crate::{WallpaperPointerState, WallpaperSurfaceInfo, WallpaperTargetMonitor};

/// Main plugin to run the live wallpaper.
#[derive(Default)]
pub struct WindowOverlayPlugin {
    /// Selects which monitor(s) to render to (primary, index, or all).
    pub target_monitor: WallpaperTargetMonitor,
}

impl Plugin for WindowOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.target_monitor)
            .init_resource::<WallpaperPointerState>()
            .init_resource::<WallpaperSurfaceInfo>();

        app.add_plugins(super::wayland::backend::WaylandBackendPlugin);
    }
}
