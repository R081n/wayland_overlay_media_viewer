#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

pub mod animated_image;
mod backend;
pub mod cleanup;
pub mod images;
pub mod lifecycle;
pub mod position;
pub mod spawner;
pub mod videos;

use std::time::Duration;

pub use backend::*;
use bevy::{
    DefaultPlugins,
    app::{App, PluginGroup as _, PreUpdate, Startup, TerminalCtrlCHandlerPlugin, Update},
    asset::{AssetPlugin, io::web::WebAssetPlugin},
    camera::ClearColor,
    color::Color,
    ecs::{
        message::MessageWriter, resource::Resource, schedule::IntoScheduleConfigs, system::ResMut,
    },
    image::ImagePlugin,
    render::pipelined_rendering::PipelinedRenderingPlugin,
    window::WindowPlugin,
    winit::WinitPlugin,
};
use bevy_framepace::{FramepacePlugin, FramepaceSettings};
use bevy_rand::{plugin::EntropyPlugin, prelude::WyRand};
use crossbeam_channel::{Receiver, Sender};

use crate::{
    animated_image::AnimatedImagePlugin,
    lifecycle::LiveCyclePlugin,
    spawner::{ObjectMessage, PopupPlugin, preload_asset},
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
            }),
        PopupPlugin,
        VideoPlugin,
        FramepacePlugin,
        LiveCyclePlugin,
        AnimatedImagePlugin,
        EntropyPlugin::<WyRand>::default(),
    ));

    app.add_plugins(WindowOverlayPlugin {
        target_monitor: WallpaperTargetMonitor::All,
    });

    app.add_systems(Startup, setup)
        .add_systems(PreUpdate, forward_messages.before(preload_asset))
        .run();
}

fn setup(mut framepace: ResMut<FramepaceSettings>) {
    framepace.limiter = bevy_framepace::Limiter::Manual(Duration::from_secs_f64(1.0 / 60.));
}

fn forward_messages(mut writer: MessageWriter<ObjectMessage>, new_objs: ResMut<DataReceiver>) {
    while let Ok(msg) = new_objs.new_objcets.try_recv() {
        writer.write(msg);
    }
}
