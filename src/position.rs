use bevy::{
    ecs::component::Component,
    math::{I64Vec2, Rect, U64Vec2, Vec2, Vec3},
};

#[derive(Debug, Clone, Copy)]
pub enum PopupPosition {
    Global(Vec3),
    SceenSpace(u32 /*or some id */, Vec2),
    FullScreen {
        screen: u32,
        // Relative to the center
        // screen is normalized to [-1, 1],
        relative_center: Vec2,
        mode: FullScreenMode,
    },
    Random,
}

#[derive(Debug, Clone, Copy)]
pub enum FullScreenMode {
    One,
    All,
}

pub const PIXELS_PER_METER: f32 = 100.0;

#[derive(Component, Debug)]
pub struct ScreenPosition {
    /// Rectangle in world space coordinates
    pub rect: Rect,
    /// Rectangle in world space coordinates
    pub pixel_min: I64Vec2,
    pub pixel_size: U64Vec2,
    /// Output id (some int)
    pub output: u32,
    /// 0 based index of the screen
    // TODO how to find the actual primary monitor, wrong way round for me
    pub index: u32,
}
