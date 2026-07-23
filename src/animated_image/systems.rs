use std::time::Duration;

use bevy::prelude::*;

use crate::animated_image::{
    Gif3d, GifNode,
    {Gif, GifAsset, GifDespawn, GifPlayer, messages::GifDespawnMessage},
};

#[derive(Component)]
pub(crate) struct GifInitialized;
/// Initialize the [Gif]'s [Sprite] / [GifNode]'s [ImageNode] / [Gif3d]'s [MeshMaterial3d] with the first image of the sequence.
pub(crate) fn initialize_gifs(
    mut commands: Commands,
    mut gifs_q: Query<
        (
            Entity,
            Option<(&Gif, &mut Sprite)>,
            Option<(&GifNode, &mut ImageNode)>,
            Option<(&Gif3d, &mut MeshMaterial3d<StandardMaterial>)>,
            &mut GifPlayer,
        ),
        Without<GifInitialized>,
    >,
    mut gifs: ResMut<Assets<GifAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (id, gif_option, gifnode_option, gif3d_option, mut player) in gifs_q.iter_mut() {
        let handle = if let Some((gif, _)) = gif_option {
            gif.handle.clone()
        } else if let Some((gif_node, _)) = gifnode_option {
            gif_node.handle.clone()
        } else if let Some((gif3d, _)) = gif3d_option {
            gif3d.handle.clone()
        } else {
            panic!("Unexpected error: a GifPlayer was inserted in an unknown entity");
        };

        if let Some(asset) = gifs.get_mut(&handle) {
            let GifAsset {
                frames: _,
                handles,
                times,
                frame_end,
            } = asset.into_inner();

            if handles.is_empty() {
                continue;
            }
            let handle = handles.first().unwrap();

            // unwrap()-ing is fine here, because this is called after `asset_server.load()`,
            // which would panic if there is an issue with the GIF file.
            if let Some((_, mut sprite)) = gif_option {
                // just replacing the image allow to not overwrite previously given members (see [brothers example](examples/brothers.rs#spawn_flipped_larger_gif).)
                // same principle for other kinds of gif
                sprite.image = handle.clone();
            }
            if let Some((_, mut image_node)) = gifnode_option {
                image_node.image = handle.clone();
            }
            if let Some((_, mm)) = gif3d_option
                && let Some(asset) = materials.get_mut(&mm.0)
            {
                let mat = asset.into_inner();
                mat.base_color_texture = Some(handle.clone());
                mat.alpha_mode = AlphaMode::Blend;
            }

            // initialize timer
            player.current = 0; // first frame
            player.timer = Timer::new(
                Duration::from_secs_f64(frame_end.last().copied().unwrap_or(0.0)),
                TimerMode::Repeating,
            );
            player.remaining = *times;

            commands.entity(id).insert(GifInitialized);
            if handles.len() == 1 {
                commands.entity(id).remove::<GifPlayer>();
            }
        }
    }
}

/// Update the [GifPlayer] of all [Gif]s / [GifNode]s / [Gif3d] entities.
/// If the timer expires, we update the player and the [Sprite] / [ImageNode] image, accordingly to the known config.
/// It updates the [MeshMaterial3d] for 3d objects.
pub(crate) fn animate_gifs(
    gifs_q: Query<(
        Option<(&Gif, &mut Sprite)>,
        Option<(&GifNode, &mut ImageNode)>,
        Option<(&Gif3d, &mut MeshMaterial3d<StandardMaterial>)>,
        &mut GifPlayer,
    )>,
    gifs: Res<Assets<GifAsset>>,
    time: Res<Time>,
    mut writer: MessageWriter<GifDespawnMessage>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (gif_option, gifnode_option, gif3d_option, mut player) in gifs_q {
        let handle = if let Some((gif, _)) = gif_option {
            gif.handle.clone()
        } else if let Some((gif_node, _)) = gifnode_option {
            gif_node.handle.clone()
        } else if let Some((gif3d, _)) = gif3d_option {
            gif3d.handle.clone()
        } else {
            panic!("Unexpected error: a GifPlayer was inserted in an unknown entity");
        };

        if let Some(gif_asset) = gifs.get(&handle) {
            player.timer.tick(time.delta());
            let time_now = player.timer.elapsed_secs_f64();
            let current = gif_asset
                .frame_end
                .iter()
                .position(|t| *t > time_now)
                .unwrap_or(0);

            if player.current != current {
                player.current = current;

                if player.current == 0 {
                    // That means we just ended a loop !
                    if let Some(remaining) = player.remaining {
                        if remaining == 0 {
                            player.timer.pause();
                            writer.write(GifDespawnMessage(handle.clone()));
                        } else {
                            player.remaining = Some(remaining - 1);
                        }
                    }
                    // no else because it means it is an infinite-looping GIF.
                }

                // Update sprite
                let handle = gif_asset.handles[player.current].clone();
                if let Some((_, mut sprite)) = gif_option {
                    sprite.image = handle.clone();
                }
                if let Some((_, mut image_node)) = gifnode_option {
                    image_node.image = handle.clone();
                }
                if let Some((_, mm)) = gif3d_option
                    && let Some(asset) = materials.get_mut(&mm.0)
                {
                    let mat = asset.into_inner();
                    mat.base_color_texture = Some(handle.clone());
                    mat.alpha_mode = AlphaMode::Blend;
                }
            }
        }
    }
}

/// Triggered when a GIF with a finite number of loops reaches its end.
/// Despawn the relevant entity.
pub(crate) fn despawn_gifs(
    mut commands: Commands,
    mut reader: MessageReader<GifDespawnMessage>,
    gif_q: Query<(Option<&Gif>, Option<&GifNode>, Option<&Gif3d>, Entity), With<GifDespawn>>,
) {
    for GifDespawnMessage(handle) in reader.read() {
        for (gif_option, gifnode_option, gif3d_option, entity) in gif_q {
            let gif_handle = if let Some(gif) = gif_option {
                gif.handle.clone()
            } else if let Some(gif_node) = gifnode_option {
                gif_node.handle.clone()
            } else if let Some(gif3d) = gif3d_option {
                gif3d.handle.clone()
            } else {
                panic!("Unexpected error: a GifPlayer was inserted in an unknown entity");
            };
            if gif_handle.id() == handle.id() {
                commands.entity(entity).despawn();
            }
        }
    }
}
