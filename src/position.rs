use bevy::{
    ecs::component::Component,
    math::{primitives::Rectangle, Vec2, Vec3},
};

pub enum PopupPosition {
    Global(Vec3),

    // TODO random screen if we know how to reference
    Random,
}

pub const PIXELS_PER_METER: f32 = 100.0;

#[derive(Component)]
pub struct ScreenPosition {
    pub bottom_right: Vec2,
    pub size: Rectangle,
}
