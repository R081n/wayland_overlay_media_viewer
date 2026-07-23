use bevy::{
    ecs::component::Component,
    math::{I64Vec2, Rect, U64Vec2, Vec2, Vec3},
};

#[derive(Debug, Clone)]
pub enum PopupPosition {
    Global(Vec3),
    SceenSpace(u32 /*or some id */, Vec2),
    Random,
}

pub const PIXELS_PER_METER: f32 = 100.0;

#[derive(Component, Debug)]
pub struct ScreenPosition {
    /// Rectangle in world space coordinates
    pub rect: Rect,
    /// Rectangle in world space coordinates
    pub pixel_min: I64Vec2,
    pub pixel_size: U64Vec2,
    pub output: u32,
}
