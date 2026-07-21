use bevy::{
    ecs::component::Component,
    math::{primitives::Rectangle, Vec2, Vec3},
};

#[derive(Debug, Clone)]
pub enum PopupPosition {
    Global(Vec3),
    SceenSpace(u32 /*or some id */, Vec2),
    Random,
}

pub const PIXELS_PER_METER: f32 = 100.0;

#[derive(Component)]
pub struct ScreenPosition {
    pub bottom_right: Vec2,
    pub size: Rectangle,
}
