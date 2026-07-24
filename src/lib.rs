#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

pub mod animated_image;
mod backend;
pub mod cleanup;
mod draw_order;
pub mod images;
pub mod interaction;
pub mod lifecycle;
pub mod position;
pub mod shader;
pub mod spawner;
pub mod texts;
pub mod videos;

use std::time::Duration;

pub use backend::*;
use bevy::{
    DefaultPlugins,
    app::{App, PluginGroup as _, PreUpdate, Startup, TerminalCtrlCHandlerPlugin, Update},
    asset::{AssetPlugin, io::web::WebAssetPlugin},
    camera::ClearColor,
    color::Color,
    dev_tools::picking_debug::DebugPickingMode,
    ecs::{
        error::ErrorContext, message::MessageWriter, resource::Resource,
        schedule::IntoScheduleConfigs, system::ResMut,
    },
    image::ImagePlugin,
    pbr::PbrPlugin,
    picking::PickingSettings,
    prelude::*,
    render::pipelined_rendering::PipelinedRenderingPlugin,
    window::WindowPlugin,
    winit::WinitPlugin,
};
use bevy_framepace::{FramepacePlugin, FramepaceSettings};
use bevy_rand::{plugin::EntropyPlugin, prelude::WyRand};
use crossbeam_channel::{Receiver, Sender};

use crate::{
    animated_image::AnimatedImagePlugin,
    draw_order::CustomDrawOrderPlugin,
    interaction::PopupInteractionPlugin,
    lifecycle::LiveCyclePlugin,
    position::ScreenPosition,
    shader::DynamicShaderPlugin,
    spawner::{ObjectMessage, PopupPlugin, preload_asset},
    texts::TextOverlayPlugin,
    videos::plugin::VideoPlugin,
};

#[derive(Clone)]
pub struct AppHandle {
    new_objects: Sender<ObjectMessage>,
}

#[derive(Resource)]
pub struct DataReceiver {
    new_objcets: Receiver<ObjectMessage>,
}

impl AppHandle {
    pub fn send(&self, msg: ObjectMessage) {
        _ = self.new_objects.send(msg);
    }
}

pub fn startup() -> AppHandle {
    let (tx, rx) = crossbeam_channel::bounded(100);

    std::thread::spawn(move || startup_inner(DataReceiver { new_objcets: rx }));
    AppHandle { new_objects: tx }
}

fn startup_inner(rx: DataReceiver) {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::srgba_u32(0)));
    app.insert_resource(rx);

    app.add_systems(Update, videos::plugin::render_video_frame);
    app.add_observer(videos::plugin::on_video_removed);

    let window_plugin = WindowPlugin {
        primary_window: None,
        exit_condition: bevy::window::ExitCondition::DontExit,
        ..Default::default()
    };

    app.add_plugins((
        DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(WebAssetPlugin {
                silence_startup_warning: true,
            })
            .set(window_plugin)
            .disable::<PipelinedRenderingPlugin>()
            .disable::<TerminalCtrlCHandlerPlugin>()
            .set(WinitPlugin {
                run_on_any_thread: true,
            })
            .set(AssetPlugin {
                unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                ..Default::default()
            })
            .set(PbrPlugin {
                add_default_deferred_lighting_plugin: false,
                prepass_enabled: false,
                use_gpu_instance_buffer_builder: false,
                ..Default::default()
            }),
        PopupPlugin,
        VideoPlugin,
        FramepacePlugin,
        LiveCyclePlugin,
        AnimatedImagePlugin,
        CustomDrawOrderPlugin,
        PopupInteractionPlugin,
        DynamicShaderPlugin,
        TextOverlayPlugin,
        EntropyPlugin::<WyRand>::default(),
    ));

    app.insert_resource(PickingSettings {
        is_enabled: true,
        is_input_enabled: true,
        is_hover_enabled: true,
        is_window_picking_enabled: false,
        multi_click_interval: Duration::from_millis(500),
    });
    app.insert_resource(MeshPickingSettings {
        require_markers: true,
        ray_cast_visibility: RayCastVisibility::Any,
    });

    app.insert_resource(DebugPickingMode::Noisy);

    app.add_plugins(WindowOverlayPlugin {
        target_monitor: WallpaperTargetMonitor::All,
    });
    app.set_error_handler(my_error_handler);
    app.add_systems(Startup, setup)
        .add_systems(PreUpdate, forward_messages.before(preload_asset))
        .run();
}

fn my_error_handler(error: BevyError, ctx: ErrorContext) {
    if ctx.name().ends_with("plz_ignore") {
        trace!("Nothing to see here, move along.");
        return;
    }
    bevy::ecs::error::error(error, ctx);
}

fn setup(mut framepace: ResMut<FramepaceSettings>) {
    framepace.limiter = bevy_framepace::Limiter::Manual(Duration::from_secs_f64(1.0 / 60.));
}

fn forward_messages(
    mut writer: MessageWriter<ObjectMessage>,
    new_objs: ResMut<DataReceiver>,
    screens: Query<&ScreenPosition>,
) {
    if screens.is_empty() {
        return;
    }
    while let Ok(msg) = new_objs.new_objcets.try_recv() {
        writer.write(msg);
    }
}
