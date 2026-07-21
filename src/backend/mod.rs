//! Bevy Live Wallpaper
//!
//! A Bevy plugin that renders your scene as the desktop wallpaper on Wayland,
//! X11, and Windows. Pick the matching backend feature (`wayland` or `x11`) on
//! Linux/BSD; Windows works with defaults.

pub mod input;
pub mod plugin;
pub mod surface_info;
pub mod target_monitor;

mod wayland;

pub use plugin::WindowOverlayPlugin;

pub use input::{PointerButton, PointerSample, WallpaperPointerState};
pub use surface_info::WallpaperSurfaceInfo;
pub use target_monitor::WallpaperTargetMonitor;

pub use wayland::surface::WaylandSurfaceHandles;
