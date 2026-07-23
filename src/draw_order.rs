use bevy::{
    core_pipeline::core_3d::Transparent3d,
    platform::collections::HashMap,
    prelude::*,
    render::{
        Render, RenderApp,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_phase::ViewSortedRenderPhases,
        sync_world::MainEntity,
    },
};

/// A plugin that overrides Bevy 0.19's distance-based transparency sorting fallback
/// with a deterministic, user-defined `DrawOrder` integer index.
pub struct CustomDrawOrderPlugin;

impl Plugin for CustomDrawOrderPlugin {
    fn build(&self, app: &mut App) {
        // 1. Synchronize the component from the App World into the Render Sub-App every frame
        app.add_plugins(ExtractComponentPlugin::<DrawOrder>::default());

        // 2. Safely grab the Render Sub-App and append our customized phase sorting
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                sort_transparent_by_custom_order
                    // Explicitly schedule immediately AFTER Bevy's built-in transparency sorting
                    .after(bevy::render::render_phase::sort_phase_system::<Transparent3d>),
            );
        }
    }
}

// --- Main World Components ---

/// Lower values are drawn first (background). Higher values are drawn last (on top).
/// The `#[extract_component]` macro attribute is mandatory in modern Bevy versions.
#[derive(Component, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ExtractComponent)]
pub struct DrawOrder(pub i64);

// --- Render World Systems ---

/// Intercepts the visible `Transparent3d` render phases and mutates the sorting criteria.
fn sort_transparent_by_custom_order(
    //  Modern Bevy exposes the active rendering lists via a mutable Resource map instead of a Query
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    draw_orders: Query<(Entity, &MainEntity, &DrawOrder)>,
    mut local: Local<HashMap<MainEntity, DrawOrder>>,
) {
    let map = &mut *local;
    map.clear();
    map.extend(draw_orders.iter().map(|(_, e, order)| (e, order)));

    // Iterate through every active camera view's transparent item list
    for (_, transparent_phase) in transparent_render_phases.iter_mut() {
        // Mutate the underlying active GPU draw queue slice in place
        transparent_phase
            .items
            .sort_by(|(_id_a, main_id_a), t_a, (_id_b, main_id_b), t_b| {
                // Fetch the custom DrawOrder component via the render entity associated with the draw token

                let order_a = map.get(main_id_a).copied().unwrap_or(DrawOrder(0));
                let order_b = map.get(main_id_b).copied().unwrap_or(DrawOrder(0));

                // Primary Check: Compare explicit draw layer values
                if order_a != order_b {
                    return order_a.cmp(&order_b);
                }

                // Secondary Fallback: Maintain standard back-to-front depth calculation if layers match
                // Reversed comparison because transparent phases evaluate further items first
                t_b.distance
                    .partial_cmp(&t_a.distance)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
    }
}
