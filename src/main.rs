use std::time::Duration;

use bevy::{
    color::{Color, Srgba},
    math::{Vec2, VectorSpace},
};
use rand::seq::SliceRandom;
use wayland_overlay_media_viewer::{
    position::{FullScreenMode, PopupPosition},
    spawner::{
        CloseClickSettings, Movement, ObjectCloseCondition, ObjectMessage, ObjectType, PopupImage,
        PopupInAnimation, PopupInteraction, PopupLayer, PopupOutAnimation, PopupVideo, TextPopup,
    },
};
use wgpu::PredefinedColorSpace::Srgb;

fn main() {
    let sender = wayland_overlay_media_viewer::startup();

    // sender.send(ObjectMessage {
    //     kind: ObjectType::Shader(CustomShaderSource {
    //         code: MY_WGSL_STRING.to_owned(),
    //         duraton: 10.0,
    //     }),
    //     position: PopupPosition::FullScreen {
    //         screen: 1,
    //         relative_center: bevy::math::Vec2::ZERO,
    //         mode: FullScreenMode::All,
    //     },
    //     layer: PopupLayer::Above,
    //     popup_animation: PopupInAnimation::None,
    //     behaviour: PopupInteraction::ClickThough,
    //     opacity: 0.01,
    //     close_animation: PopupOutAnimation::FadeOut { decay_rate: 1.0 },
    //     close_condition: ObjectCloseCondition {
    //         duration: None,
    //         click: None,
    //     },
    // });

    _ = sender.send(ObjectMessage {
        kind: ObjectType::Text(TextPopup {
            text: "this is my very fun text".to_owned(),
            color: Color::Srgba(Srgba::new(1.0, 1.0, 1.0, 1.0)),
            font: bevy::text::TextFont {
                font_size: bevy::text::FontSize::Px(30.0),
                ..Default::default()
            },
        }),
        position: PopupPosition::Random,
        layer: PopupLayer::Above,
        popup_animation: PopupInAnimation::None,
        behaviour: PopupInteraction::Clickable,
        opacity: 1.0,
        close_animation: PopupOutAnimation::FadeOut { decay_rate: 1.0 },
        close_condition: ObjectCloseCondition {
            duration: None,
            click: Some(CloseClickSettings::default()),
        },
        movement: Movement::Linear(Vec2::new(1., 0.0)),
    });

    let clone = sender.clone();

    std::thread::spawn(move || loop {
        _ = clone.send(ObjectMessage {
            kind: ObjectType::Text(TextPopup {
                text: "this is my very fun text".to_owned(),
                color: Color::Srgba(Srgba::new(1.0, 1.0, 1.0, 1.0)),
                font: bevy::text::TextFont {
                    font_size: bevy::text::FontSize::Px(30.0),
                    ..Default::default()
                },
            }),
            position: PopupPosition::Random,
            layer: PopupLayer::Above,
            popup_animation: PopupInAnimation::None,
            behaviour: PopupInteraction::Clickable,
            opacity: 1.0,
            close_animation: PopupOutAnimation::FadeOut { decay_rate: 1.0 },
            close_condition: ObjectCloseCondition {
                duration: None,
                click: Some(CloseClickSettings::default()),
            },
            movement: Movement::Linear(Vec2::new(1., 0.0)),
        });
    });

    for line in std::io::stdin().lines() {
        let line = line.unwrap();

        for link in line
            .split(['\'', '"'])
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
        {
            if let Ok(dir) = std::fs::read_dir(link) {
                let sender = sender.clone();
                std::thread::spawn(move || {
                    let mut list: Vec<_> = dir.flatten().collect();
                    list.shuffle(&mut rand::rng());

                    for entry in list {
                        _ = sender.send(ObjectMessage {
                            kind: ObjectType::Image(PopupImage {
                                uri: entry.path().to_string_lossy().into_owned(),
                            }),

                            position: PopupPosition::Random,
                            popup_animation: PopupInAnimation::SlideFadeIn,
                            behaviour: PopupInteraction::Clickable,
                            opacity: 0.9,
                            layer: PopupLayer::Normal,
                            close_animation: PopupOutAnimation::FadeOut { decay_rate: 5.0 },
                            close_condition: ObjectCloseCondition {
                                duration: None,
                                click: Some(CloseClickSettings::default()),
                            },
                            movement: Movement::None,
                        });
                        std::thread::sleep(Duration::from_millis(1000));
                    }
                });
            }

            _ = sender.send(ObjectMessage {
                kind: to_type(link.to_owned()),
                position: PopupPosition::FullScreen {
                    screen: 1,
                    relative_center: Vec2::ZERO,
                    mode: FullScreenMode::One,
                },
                popup_animation: PopupInAnimation::SlideFadeIn,
                behaviour: PopupInteraction::Clickable,
                opacity: 0.9,
                layer: PopupLayer::Normal,
                close_animation: PopupOutAnimation::FadeOut { decay_rate: 5.0 },
                close_condition: ObjectCloseCondition {
                    duration: None,
                    click: Some(CloseClickSettings::default()),
                },
                movement: Movement::None,
            });
        }
    }
}

fn to_type(string: String) -> ObjectType {
    if string.ends_with("mp4") || string.ends_with("webm") {
        ObjectType::Video(PopupVideo { uri: string })
    } else {
        ObjectType::Image(PopupImage { uri: string })
    }
}

const MY_WGSL_STRING: &str = r#"
// Remap UV from [0, 1] to [-0.5, 0.5] to center the origin
let uv_centered = (in.uv - vec2<f32>(0.5, 0.5)) * p_scale;

// Calculate distance from the center
let radius = length(uv_centered);

// Calculate angle in radians (-PI to PI)
let angle = atan2(uv_centered.y, uv_centered.x);


let speed = 3.0;
let tightness = 3.0;
let arms =  1.0;

let spiral_factor =  angle * arms + log2(radius) * tightness - p_progress * speed;
        
// Use sine to create an oscillating value between -1.0 and 1.0
let wave = sin(spiral_factor);

// Manual Anti-Aliasing using screen-space derivatives (fwidth)
// This calculates exactly how fast the wave changes between neighboring pixels
let delta = fwidthFine(wave);

// Smoothstep creates a razor-sharp edge with a 1.5-pixel blending zone
// to prevent aliasing (jagged edges) without causing blurriness
let mask = smoothstep(-delta * 1.5, delta * 1.5, wave);

let center_fade = mix(0.5, mask, clamp(radius * 100 - 0.7, 0.0, 1.0));

let final_mask = center_fade;

let color = vec3<f32>(final_mask);

return vec4<f32>(color, p_opacity * final_mask); 
"#;
