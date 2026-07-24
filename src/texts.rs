//! A simple UI health bar which follows an object around in 3D space.
//! Using UI nodes is just one way to do this. Alternatively, you can use
//! a mesh facing the camera to set up your health bar.

use std::ops::Div;

use bevy::color::palettes::basic::{BLACK, GREEN};
use bevy::math::ops::{cos, sin};
use bevy::prelude::*;

use crate::position::PIXELS_PER_METER;
use crate::spawner::{ProxyOpacity, TargetOpacity, TextPopup};
use crate::RequestInputRecalc;

pub struct TextOverlayPlugin;

impl Plugin for TextOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_text)
            .add_observer(create_new_texts);
    }
}

#[derive(Component)]
#[relationship(relationship_target = TextAnchor)]
pub struct AnchoredText(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = AnchoredText, linked_spawn)]
pub struct TextAnchor(Vec<Entity>);

fn create_new_texts(
    trigger: On<Insert, TextPopup>,
    mut commands: Commands,
    cameras: Query<(Entity, &Camera)>,
    popup: Query<&TextPopup>,
) {
    let (popup) = popup.get(trigger.entity).unwrap();
    for (camid, camera) in cameras {
        commands.spawn((
            Text::new(popup.text.clone()),
            popup.font.clone(),
            UiTargetCamera(camid),
            AnchoredText(trigger.entity),
            TextColor(popup.color),
        ));
    }
}

fn update_text(
    mut text_query: Query<(&mut Node, &UiTargetCamera, &ComputedNode, &mut TextColor)>,
    target_query: Query<(&TextAnchor, &GlobalTransform, &mut Transform, &ProxyOpacity)>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mut recalc_size: ResMut<RequestInputRecalc>,
) -> Result<(), BevyError> {
    for (anchor, target_global_transform, mut target_transform, opacity) in target_query {
        for n_entity in anchor.iter() {
            let (mut node, cam_id, computed_node, mut text_color) =
                text_query.get_mut(n_entity).unwrap();
            let (camera, camera_transform) = camera_query.get(cam_id.entity()).unwrap();

            let target_world_position = target_global_transform.translation();
            let target_viewport_position =
                camera.world_to_viewport(camera_transform, target_world_position)?;

            node.left = Val::Px(target_viewport_position.x - computed_node.size.x / 2.0);
            node.top = Val::Px(target_viewport_position.y - computed_node.size.y / 2.0);

            if opacity.0 != text_color.0.alpha() {
                text_color.0.set_alpha(opacity.0);
            }

            if !target_transform
                .scale
                .xy()
                .abs_diff_eq(computed_node.size() / PIXELS_PER_METER, 0.1)
            {
                target_transform.scale = computed_node.size().div(PIXELS_PER_METER).extend(1.0);
                recalc_size.request();
            }
        }
    }

    Ok(())
}
