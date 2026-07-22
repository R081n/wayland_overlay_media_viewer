// https://github.com/bevyengine/bevy/blob/v0.17.2/examples/3d/3d_shapes.rs

use std::{f32::consts::PI, time::Duration};

use bevy::{
    app::TerminalCtrlCHandlerPlugin,
    asset::{RenderAssetUsages, io::web::WebAssetPlugin},
    prelude::*,
    render::{
        pipelined_rendering::PipelinedRenderingPlugin,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use bevy_framepace::{FramepacePlugin, FramepaceSettings};
use wayland_overlay_media_viewer::{
    WallpaperTargetMonitor, WindowOverlayPlugin,
    gif::GifPlugin,
    lifecycle::LiveCyclePlugin,
    spawner::{ObjectMessage, PopupImage, PopupPlugin},
    videos::plugin::VideoPlugin,
};

fn main() {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::srgba_u32(0)));
    app.add_systems(
        Update,
        wayland_overlay_media_viewer::videos::plugin::render_video_frame,
    );

    let mut window_plugin = WindowPlugin::default();

    window_plugin.primary_window = None;
    window_plugin.exit_condition = bevy::window::ExitCondition::DontExit;

    app.add_plugins((
        DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(WebAssetPlugin {
                silence_startup_warning: true,
            })
            .set(window_plugin)
            .disable::<PipelinedRenderingPlugin>()
            .disable::<TerminalCtrlCHandlerPlugin>()
            .set(AssetPlugin {
                unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                ..Default::default()
            }),
        PopupPlugin,
        VideoPlugin,
        FramepacePlugin,
        LiveCyclePlugin,
        GifPlugin,
    ));

    app.add_plugins(WindowOverlayPlugin {
        target_monitor: WallpaperTargetMonitor::All,
    });

    app.add_systems(Startup, setup)
        .add_systems(Update, (rotate,))
        .run();
}

/// A marker component for our shapes so we can query them separately from the ground plane
#[derive(Component)]
struct Shape;

const SHAPES_X_EXTENT: f32 = 50.0;
const EXTRUSION_X_EXTENT: f32 = 30.0;
const Z_EXTENT: f32 = 0.0;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut framepace: ResMut<FramepaceSettings>,
) {
    framepace.limiter = bevy_framepace::Limiter::Manual(Duration::from_secs_f64(1.0 / 60.));
    let debug_material = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(uv_debug_texture())),
        ..default()
    });

    let shapes = [
        meshes.add(Cuboid::default()),
        meshes.add(Tetrahedron::default()),
        meshes.add(Capsule3d::default()),
        meshes.add(Torus::default()),
        meshes.add(Cylinder::default()),
        meshes.add(Cone::default()),
        meshes.add(ConicalFrustum::default()),
        meshes.add(Sphere::default().mesh().ico(5).unwrap()),
        meshes.add(Sphere::default().mesh().uv(32, 18)),
        meshes.add(Segment3d::default()),
        meshes.add(Polyline3d::new(vec![
            Vec3::new(-0.5, 0.0, 0.0),
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::new(0.0, 0.5, 0.0),
        ])),
    ];

    let _rectangle = meshes.add(Rectangle::from_size(Vec2::ONE));

    let extrusions = [
        meshes.add(Extrusion::new(Rectangle::default(), 1.)),
        meshes.add(Extrusion::new(Capsule2d::default(), 1.)),
        meshes.add(Extrusion::new(Annulus::default(), 1.)),
        meshes.add(Extrusion::new(Circle::default(), 1.)),
        meshes.add(Extrusion::new(Ellipse::default(), 1.)),
        meshes.add(Extrusion::new(RegularPolygon::default(), 1.)),
        meshes.add(Extrusion::new(Triangle2d::default(), 1.)),
    ];

    let num_shapes = shapes.len();

    for (i, shape) in shapes.into_iter().enumerate() {
        commands.spawn((
            Mesh3d(shape),
            MeshMaterial3d(debug_material.clone()),
            Transform::from_xyz(
                i as f32 / (num_shapes - 1) as f32 * SHAPES_X_EXTENT,
                2.0,
                Z_EXTENT / 2.,
            )
            .with_rotation(Quat::from_rotation_x(-PI / 4.)),
            Shape,
        ));
    }

    let num_extrusions = extrusions.len();

    for (i, shape) in extrusions.into_iter().enumerate() {
        commands.spawn((
            Mesh3d(shape),
            MeshMaterial3d(debug_material.clone()),
            Transform::from_xyz(
                i as f32 / (num_extrusions - 1) as f32 * EXTRUSION_X_EXTENT,
                4.0,
                -Z_EXTENT / 2.,
            )
            .with_rotation(Quat::from_rotation_x(-PI / 4.)),
            Shape,
        ));
    }

    commands.spawn((
        Text2d("Hello".to_owned()),
        Transform::from_translation(Vec3::new(20., 60., 0.)),
    ));

    commands.spawn((
        Text2d("Hello".to_owned()),
        Transform::from_translation(Vec3::new(20., 60., 0.)),
    ));

    for (id, l) in [
        "/home/robink/Downloads/916af9c0bd4af6c8131a19e5d841d7e4.webp",
        "/home/robink/Downloads/9c92ab83276d3ef3c16518f2b779abc4.webp",
        "/home/robink/Downloads/4e943391859cdcab9ed52c1b032c2a0e.webp",
        "/home/robink/Downloads/f1c2c4e7bfd196fb7dc1a51042e7710d.webp",
        "/home/robink/Downloads/c16e0d702b3136151fef60e9b20038ad.webp",
        "/home/robink/Downloads/2acec51f58722b13ebc278b9923a1906.webp",
    ]
    .iter()
    .copied()
    .enumerate()
    {
        for i in 0..2 {
            commands
                .delayed()
                .secs(id as f32 + i as f32 * 3.)
                .write_message(ObjectMessage {
                    kind: wayland_overlay_media_viewer::spawner::ObjectType::Image(PopupImage {
                        uri: l.to_owned(),
                    }),
                    position: wayland_overlay_media_viewer::position::PopupPosition::Global(
                        Vec3::new(
                            10.0 + id as f32 * 4.,
                            i as f32 * 8. + 1.,
                            0.001 * id as f32 + 0.01 * i as f32,
                        ),
                    ),
                    popup_animation:
                        wayland_overlay_media_viewer::spawner::PopupAnimation::SlideFadeIn,
                    behaviour: wayland_overlay_media_viewer::spawner::PopupInteraction::ClickThough,
                    opacity: 0.4,
                });
        }
    }

    // commands.delayed().secs(3.0).write_message(ObjectMessage {
    //     kind: wayland_overlay_media_viewer::spawner::ObjectType::Video(PopupVideo {
    //         uri: "".to_owned(),
    //     }),
    //     position: wayland_overlay_media_viewer::position::PopupPosition::Global(Vec3::new(
    //         37., 7., 0.0,
    //     )),
    //     popup_animation: wayland_overlay_media_viewer::spawner::PopupAnimation::SlideFadeIn,
    //     behaviour: wayland_overlay_media_viewer::spawner::PopupInteraction::ClickThough,
    //     opacity: 0.9,
    // });

    commands.spawn((
        PointLight {
            shadow_maps_enabled: true,
            intensity: 10_000_000.,
            range: 100.0,
            shadow_depth_bias: 0.2,
            ..default()
        },
        Transform::from_xyz(8.0, 16.0, 8.0),
    ));
}

fn rotate(mut query: Query<&mut Transform, With<Shape>>, time: Res<Time>) {
    for mut transform in &mut query {
        transform.rotate_y(time.delta_secs() / 2.);
    }
}

/// Creates a colorful test pattern
fn uv_debug_texture() -> Image {
    const TEXTURE_SIZE: usize = 8;

    let mut palette: [u8; 32] = [
        255, 102, 159, 255, 255, 159, 102, 255, 236, 255, 102, 255, 121, 255, 102, 255, 102, 255,
        198, 255, 102, 198, 255, 255, 121, 102, 255, 255, 236, 102, 255, 255,
    ];

    let mut texture_data = [0; TEXTURE_SIZE * TEXTURE_SIZE * 4];
    for y in 0..TEXTURE_SIZE {
        let offset = TEXTURE_SIZE * y * 4;
        texture_data[offset..(offset + TEXTURE_SIZE * 4)].copy_from_slice(&palette);
        palette.rotate_right(4);
    }

    Image::new_fill(
        Extent3d {
            width: TEXTURE_SIZE as u32,
            height: TEXTURE_SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &texture_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}
