#![allow(unused)]

use schemars::JsonSchema;

#[derive(JsonSchema)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(JsonSchema)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(JsonSchema)]
pub enum Color {
    /// A color in the sRGB color space with alpha.
    Srgba(Srgba),
}

#[derive(JsonSchema)]
pub struct Srgba {
    /// The red channel. [0.0, 1.0]
    pub red: f32,
    /// The green channel. [0.0, 1.0]
    pub green: f32,
    /// The blue channel. [0.0, 1.0]
    pub blue: f32,
    /// The alpha channel. [0.0, 1.0]
    pub alpha: f32,
}
