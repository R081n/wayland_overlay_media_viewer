use std::io::ErrorKind;
use std::{collections::HashSet, f32::consts::FRAC_PI_4};

use bevy::asset::uuid::Uuid;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::cluster::ClusterConfig;
use bevy::math::{I64Vec2, U64Vec2};
use bevy::picking::PickingSystems;
use bevy::picking::pointer::{
    Location, PointerAction, PointerId, PointerInput, PointerLocation, update_pointer_map,
};
use bevy::render::view::NoIndirectDrawing;
use bevy::{
    camera::{ImageRenderTarget, RenderTarget},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems, extract_component::ExtractComponentPlugin,
        extract_resource::ExtractResourcePlugin,
    },
};
use wayland_client::{Connection, EventQueue, Proxy, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::RequestInputRecalc;
use crate::backend::wayland::input_region_sync::LayerShellInputPlugin;
use crate::position::{PIXELS_PER_METER, ScreenPosition};
use crate::{
    PointerSample, WallpaperPointerState, WallpaperSurfaceInfo, WallpaperTargetMonitor,
    backend::wayland::render::SurfaceDescriptorEntry,
};

use super::{
    PendingPointerEvent, WaylandAppState,
    render::{
        WaylandGpuSurfaceState, WaylandRenderTarget, WaylandSurfaceDescriptor,
        create_wayland_image, prepare_wayland_surface, present_wayland_surface,
    },
};

pub(crate) struct WaylandBackendPlugin;

impl Plugin for WaylandBackendPlugin {
    fn build(&self, app: &mut App) {
        let conn = Connection::connect_to_env().unwrap();
        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();

        let display = conn.display();
        display.get_registry(&qh, ());

        let mut app_state = WaylandAppState::new(display.clone());

        info!("Waiting for globals...");
        event_queue.roundtrip(&mut app_state).unwrap();
        info!("Globals received.");

        // At startup, create surfaces for the currently requested target monitor if available.
        let initial_target = app
            .world()
            .get_resource::<WallpaperTargetMonitor>()
            .copied()
            .unwrap_or_default();
        ensure_surfaces_for_outputs(&mut app_state, &qh, &initial_target);
        info!("Initial commit done. Waiting for configure event...");

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<WaylandGpuSurfaceState>()
            .add_systems(
                Render,
                prepare_wayland_surface.in_set(RenderSystems::PrepareResources),
            )
            .add_systems(
                Render,
                present_wayland_surface.in_set(RenderSystems::Cleanup),
            );

        app.insert_resource(WaylandSurfaceDescriptor::new())
            .add_plugins((
                ExtractResourcePlugin::<WaylandSurfaceDescriptor>::default(),
                ExtractComponentPlugin::<WaylandRenderTarget>::default(),
            ))
            .add_systems(PostUpdate, wayland_event_system)
            .add_systems(First, pointer_input_system.in_set(PickingSystems::Input))
            .insert_non_send(WaylandEventQueue(event_queue))
            .insert_non_send(app_state)
            .add_plugins(LayerShellInputPlugin);
    }
}

#[derive(Resource, Deref, DerefMut)]
pub(crate) struct WaylandEventQueue(EventQueue<WaylandAppState>);

pub fn wayland_event_system(
    mut commands: Commands,
    mut event_queue: NonSendMut<WaylandEventQueue>,
    mut app_state: NonSendMut<WaylandAppState>,
    mut surface_descriptor: ResMut<WaylandSurfaceDescriptor>,
    target_monitor: Res<WallpaperTargetMonitor>,
    mut surface_info: ResMut<WallpaperSurfaceInfo>,
    mut cameras: Query<(&WaylandRenderTarget, &mut Transform, &mut ScreenPosition)>,
    mut images: ResMut<Assets<Image>>,
) {
    if app_state.is_running() {
        if let Err(err) = pump_wayland_events(&mut event_queue, &mut app_state) {
            warn!("Wayland event dispatch failed: {err:?}; closing background surface");
            app_state.closed = true;
            surface_descriptor.surfaces.clear();
            surface_descriptor.bump_generation();
            return;
        }

        let qh = event_queue.handle();
        let (mut touched, removed) =
            ensure_surfaces_for_outputs(&mut app_state, &qh, &target_monitor);

        if !removed.is_empty() {
            for removed in surface_descriptor
                .surfaces
                .extract_if(.., |s| !removed.contains(&s.output))
            {
                commands.entity(removed.camera).despawn();
            }
            touched = true;
        }

        for surface_config in app_state.take_surface_config() {
            info!(
                "Wayland surface configured (output {}): {}x{}",
                surface_config.output, surface_config.width, surface_config.height
            );
            surface_descriptor.upsert_surface(
                surface_config,
                spawn_camera(&mut commands, &mut images, surface_config),
            );
            touched = true;
        }

        // Integrate fresh logical positions/sizes from xdg-output / wl_output.
        if apply_output_info_updates(&mut surface_descriptor, &mut app_state) {
            touched = true;

            for desc in &surface_descriptor.surfaces {
                commands
                    .entity(desc.camera)
                    .insert(create_transform(desc, &app_state.output_order));
            }
        }

        if touched {
            surface_descriptor.bump_generation();
        }

        if let Some((min_x, min_y, w, h)) =
            ready_bounds(&surface_descriptor, &app_state, &target_monitor)
        {
            surface_info.set(min_x, min_y, w, h);
        }
    }
}

pub fn pointer_input_system(
    mut app_state: NonSendMut<WaylandAppState>,
    mut pointer_state: ResMut<WallpaperPointerState>,
    mut cameras: Query<(&WaylandRenderTarget, &mut Transform, &mut ScreenPosition)>,
    mut pointer_writer: MessageWriter<PointerInput>,
) {
    if app_state.is_running() {
        let had_pointer_events = !app_state.pending_pointer_events.is_empty();
        apply_pointer_events(
            &mut pointer_state,
            app_state.pending_pointer_events.drain(..),
            &mut pointer_writer,
            &mut cameras,
        );

        if !had_pointer_events && let Some(sample) = pointer_state.last.as_mut() {
            sample.delta = Vec2::ZERO;
            sample.last_button = None;
        }
    }
}

fn create_transform(
    entry: &SurfaceDescriptorEntry,
    screen_order: &[u32],
) -> (Transform, ScreenPosition) {
    let w_target = entry.width as f32 / PIXELS_PER_METER;
    let h_target = entry.height as f32 / PIXELS_PER_METER;

    let center_x = entry.offset_x as f32 / PIXELS_PER_METER + (w_target / 2.0);
    let center_y = entry.offset_y as f32 / PIXELS_PER_METER + (h_target / 2.0);

    (
        Transform::from_translation(Vec3::new(center_x, center_y, 10.0))
            .looking_at(Vec3::new(center_x, center_y, 0.0), Vec3::Y),
        ScreenPosition {
            rect: Rect::from_center_size(
                Vec2::new(center_x, center_y),
                Vec2::new(w_target, h_target),
            ),
            pixel_min: I64Vec2::new(entry.offset_x as i64, entry.offset_y as i64),
            pixel_size: U64Vec2::new(entry.width as u64, entry.height as u64),
            output: entry.output,
            index: screen_order
                .iter()
                .position(|id| entry.output == *id)
                .unwrap() as u32,
        },
    )
}

fn spawn_camera(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    config: super::WaylandSurfaceConfig,
) -> Entity {
    let image = create_wayland_image(images, config.width, config.height);

    commands
        .spawn((
            WaylandRenderTarget::new(image.clone()),
            Camera3d::default(),
            Projection::Orthographic(OrthographicProjection {
                scale: 1.0 / PIXELS_PER_METER,
                ..OrthographicProjection::default_3d()
            }),
            RenderTarget::Image(ImageRenderTarget {
                handle: image,
                scale_factor: 1.0,
            }),
            RequestInputRecalc::default(),
            // No lights: No clusters
            ClusterConfig::Single,
            // No batchable meshes
            NoIndirectDrawing,
            // We can pick
            MeshPickingCamera,
            // Don't need frustum culling, meshes are simple
            // and always on at least one screen
            NoFrustumCulling,
        ))
        .id()
}

fn pump_wayland_events(
    event_queue: &mut WaylandEventQueue,
    app_state: &mut WaylandAppState,
) -> Result<(), wayland_client::DispatchError> {
    while event_queue.dispatch_pending(app_state)? > 0 {}

    event_queue.flush()?;

    if let Some(read_guard) = event_queue.prepare_read() {
        match read_guard.read() {
            Ok(_) => while event_queue.dispatch_pending(app_state)? > 0 {},
            Err(wayland_client::backend::WaylandError::Io(err))
                if err.kind() == ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }
    } else {
        while event_queue.dispatch_pending(app_state)? > 0 {}
    }

    Ok(())
}

fn ready_bounds(
    descriptor: &WaylandSurfaceDescriptor,
    app_state: &WaylandAppState,
    target: &WallpaperTargetMonitor,
) -> Option<(i32, i32, u32, u32)> {
    let selected = selected_outputs(app_state, target)?;

    let have_all_selected = selected.iter().all(|output| {
        descriptor
            .surfaces
            .iter()
            .find(|s| s.output == *output)
            .map(|entry| entry.handles.is_some() && entry.width > 0 && entry.height > 0)
            .unwrap_or(false)
    });
    if !have_all_selected {
        return None;
    }

    let missing_output_for_all = matches!(target, WallpaperTargetMonitor::All)
        && descriptor
            .surfaces
            .iter()
            .filter(|s| s.handles.is_some())
            .count()
            < app_state.outputs.len();
    if missing_output_for_all {
        return None;
    }

    descriptor.overall_bounds()
}

fn apply_pointer_events(
    state: &mut WallpaperPointerState,
    pending: impl IntoIterator<Item = PendingPointerEvent>,
    writer: &mut MessageWriter<PointerInput>,
    cameras: &mut Query<(&WaylandRenderTarget, &mut Transform, &mut ScreenPosition)>,
) {
    for evt in pending {
        let prev_position = state
            .last
            .as_ref()
            .map(|s| s.position)
            .unwrap_or(evt.position + evt.offset);
        let new_position = evt.position + evt.offset;

        let mut sample = PointerSample {
            output: Some(evt.output),
            position: new_position,
            delta: new_position - prev_position,
            ..state.last.clone().unwrap_or_default()
        };

        let Some((target, pos)) = cameras
            .iter()
            .find(|(_, _, p)| p.output == evt.output)
            .map(|(t, _, pos)| (t.image.clone(), pos))
        else {
            continue;
        };

        let mut write = |action: PointerAction| {
            writer.write(PointerInput {
                pointer_id: bevy::picking::pointer::PointerId::Mouse,
                location: Location {
                    position: new_position - pos.pixel_min.as_vec2(),
                    target: bevy::camera::NormalizedRenderTarget::Image(ImageRenderTarget {
                        handle: target.clone(),
                        scale_factor: 1.0,
                    }),
                },
                action,
            });
        };
        if new_position != prev_position {
            write(PointerAction::Move {
                delta: (new_position - prev_position),
            });
        }

        sample.last_button = evt
            .kind
            .button_change()
            .map(|(button, pressed)| crate::backend::PointerButton { button, pressed });

        if let Some(btn) = sample.last_button
            && let Some(button) = btn.button
        {
            if btn.pressed {
                sample.pressed.insert(button);
                write(PointerAction::Press(match button {
                    MouseButton::Left => PointerButton::Primary,
                    MouseButton::Right => PointerButton::Secondary,
                    MouseButton::Middle => PointerButton::Middle,
                    _ => continue,
                }));
            } else {
                sample.pressed.remove(&button);
                write(PointerAction::Release(match button {
                    MouseButton::Left => PointerButton::Primary,
                    MouseButton::Right => PointerButton::Secondary,
                    MouseButton::Middle => PointerButton::Middle,
                    _ => continue,
                }));
            }
        }

        state.last = Some(sample);
    }
}

/// Apply the latest logical position/size info to existing surface descriptors.
/// Returns true if any descriptor changed.
fn apply_output_info_updates(
    descriptor: &mut WaylandSurfaceDescriptor,
    app_state: &mut WaylandAppState,
) -> bool {
    if app_state.dirty_outputs.is_empty() {
        return false;
    }

    let mut changed_any = false;

    #[inline]
    fn update_if<T: PartialEq + Copy>(dst: &mut T, src: T, changed: &mut bool) {
        if *dst != src {
            *dst = src;
            *changed = true;
        }
    }

    for surface in &mut descriptor.surfaces {
        if !app_state.dirty_outputs.contains(&surface.output) {
            continue;
        }

        if let Some(info) = app_state.output_info.get(&surface.output) {
            let mut changed = false;

            update_if(&mut surface.offset_x, info.x, &mut changed);
            update_if(&mut surface.offset_y, info.y, &mut changed);

            if info.width > 0 {
                update_if(&mut surface.width, info.width as u32, &mut changed);
            }
            if info.height > 0 {
                update_if(&mut surface.height, info.height as u32, &mut changed);
            }

            changed_any |= changed;
        }
    }

    app_state.dirty_outputs.clear();
    changed_any
}

/// Ensure we have a layer-surface for every known output.
/// Returns (touched, removed_outputs).
fn ensure_surfaces_for_outputs(
    app_state: &mut WaylandAppState,
    qh: &QueueHandle<WaylandAppState>,
    target: &WallpaperTargetMonitor,
) -> (bool, Vec<u32>) {
    let mut touched = false;
    let mut removed: Vec<u32> = Vec::new();

    let Some(compositor) = app_state.compositor.as_ref() else {
        return (touched, removed);
    };
    let Some(layer_shell) = app_state.layer_shell.as_ref() else {
        return (touched, removed);
    };

    let Some(selected) = selected_outputs(app_state, target) else {
        // Invalid selection (e.g., Index out of range); keep current surfaces as-is.
        return (touched, removed);
    };

    // create missing surfaces
    for output_name in &selected {
        let Some(output) = app_state.outputs.get(output_name) else {
            continue;
        };
        if app_state.surfaces.contains_key(output_name) {
            continue;
        }
        let surface = compositor.0.create_surface(qh, ());
        let surface_id = surface.id().protocol_id();
        let layer_surface = layer_shell.0.get_layer_surface(
            &surface,
            Some(output),
            // TODO TOP or Overlay. Overlay makes sure were the top most
            // But top allows other thinks above?
            zwlr_layer_shell_v1::Layer::Overlay,
            format!("egl_background_{output_name}"),
            qh,
            (),
        );

        let region = compositor.0.create_region(qh, ());

        surface.set_input_region(Some(&region));

        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_anchor(
            zwlr_layer_surface_v1::Anchor::Top
                | zwlr_layer_surface_v1::Anchor::Bottom
                | zwlr_layer_surface_v1::Anchor::Left
                | zwlr_layer_surface_v1::Anchor::Right,
        );
        layer_surface.set_size(0, 0);
        surface.commit();
        app_state.surfaces.insert(
            *output_name,
            super::OutputSurface {
                surface: surface.clone(),
                layer_surface,
            },
        );
        app_state.surface_to_output.insert(surface_id, *output_name);
        touched = true;
    }

    // remove surfaces whose outputs vanished
    let outputs: HashSet<u32> = selected.into_iter().collect();
    let to_remove: Vec<u32> = app_state
        .surfaces
        .keys()
        .filter(|k| !outputs.contains(k))
        .copied()
        .collect();
    for key in to_remove {
        if let Some(surface) = app_state.surfaces.remove(&key) {
            // Explicitly destroy to stop showing on that output.
            surface.layer_surface.destroy();
            surface.surface.destroy();
            app_state
                .surface_to_output
                .remove(&surface.surface.id().protocol_id());
        }
        touched = true;
        removed.push(key);
    }

    (touched, removed)
}

/// Choose outputs according to target monitor selection.
fn selected_outputs(
    app_state: &WaylandAppState,
    target: &WallpaperTargetMonitor,
) -> Option<Vec<u32>> {
    let mut outputs: Vec<u32> = app_state.output_order.clone();
    outputs.retain(|id| app_state.outputs.contains_key(id));

    match target {
        WallpaperTargetMonitor::All => Some(outputs),
        WallpaperTargetMonitor::Primary => {
            let v: Vec<u32> = outputs.into_iter().take(1).collect();
            if v.is_empty() { None } else { Some(v) }
        }
        WallpaperTargetMonitor::Index(n) => {
            let v: Vec<u32> = outputs.into_iter().skip(*n).take(1).collect();
            if v.is_empty() { None } else { Some(v) }
        }
    }
}
