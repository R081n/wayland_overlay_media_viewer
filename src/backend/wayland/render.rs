use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    ecs::{component::Component, entity::Entity, system::Query},
    log::{debug, error, warn},
    prelude::{Assets, Handle, Image, Res, ResMut, Resource},
    render::{
        extract_component::ExtractComponent,
        extract_resource::ExtractResource,
        render_asset::RenderAssets,
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        renderer::{RenderAdapter, RenderDevice, RenderInstance, RenderQueue},
        sync_world::MainEntity,
        texture::GpuImage,
    },
};
use wgpu::{
    BindGroupEntry, BindingResource, CommandEncoderDescriptor, CompositeAlphaMode,
    CurrentSurfaceTexture, Origin3d, PresentMode, SurfaceConfiguration, SurfaceTargetUnsafe,
    TextureAspect,
};

use super::surface::WaylandSurfaceHandles;

pub(crate) const WAYLAND_SURFACE_FORMAT: TextureFormat = TextureFormat::Bgra8UnormSrgb;

pub(crate) fn create_wayland_image(
    images: &mut Assets<Image>,
    width: u32,
    height: u32,
) -> Handle<Image> {
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        WAYLAND_SURFACE_FORMAT,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage =
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC;
    images.add(image)
}

#[derive(Resource, ExtractResource, Clone, Debug, Default)]
pub(crate) struct WaylandSurfaceDescriptor {
    pub surfaces: Vec<SurfaceDescriptorEntry>,
    pub generation: u64,
}

impl WaylandSurfaceDescriptor {
    pub(crate) fn new() -> Self {
        Self {
            surfaces: Vec::new(),
            generation: 0,
        }
    }

    pub(crate) fn upsert_surface(&mut self, config: super::WaylandSurfaceConfig, camera: Entity) {
        if let Some(entry) = self
            .surfaces
            .iter_mut()
            .find(|entry| entry.output == config.output)
        {
            entry.handles = Some(config.handles);
            entry.width = config.width;
            entry.height = config.height;
            entry.offset_x = config.offset_x;
            entry.offset_y = config.offset_y;
        } else {
            self.surfaces.push(SurfaceDescriptorEntry {
                output: config.output,
                handles: Some(config.handles),
                width: config.width,
                height: config.height,
                offset_x: config.offset_x,
                offset_y: config.offset_y,
                camera,
            });
        }
    }

    pub(crate) fn overall_bounds(&self) -> Option<(i32, i32, u32, u32)> {
        let mut iter_all = self.surfaces.iter().filter(|s| s.handles.is_some());
        let first = iter_all.next()?;

        let mut min_x = first.offset_x;
        let mut min_y = first.offset_y;
        let mut max_x = first.offset_x + first.width as i32;
        let mut max_y = first.offset_y + first.height as i32;

        for s in iter_all {
            min_x = min_x.min(s.offset_x);
            min_y = min_y.min(s.offset_y);
            max_x = max_x.max(s.offset_x + s.width as i32);
            max_y = max_y.max(s.offset_y + s.height as i32);
        }

        let width = (max_x - min_x).max(1) as u32;
        let height = (max_y - min_y).max(1) as u32;

        Some((min_x, min_y, width, height))
    }

    pub(crate) fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SurfaceDescriptorEntry {
    pub output: u32,
    pub handles: Option<WaylandSurfaceHandles>,
    pub width: u32,
    pub height: u32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub camera: Entity,
}

#[derive(Component, ExtractComponent, Clone, Debug)]
pub(crate) struct WaylandRenderTarget {
    pub image: Handle<Image>,
}

impl WaylandRenderTarget {
    pub(crate) fn new(image: Handle<Image>) -> Self {
        Self { image }
    }
}

#[derive(Resource, Default)]
pub(crate) struct WaylandGpuSurfaceState {
    pub surfaces: HashMap<u32, WaylandGpuPerSurface>,
}

#[derive(Default)]
pub(crate) struct WaylandGpuPerSurface {
    pub surface: Option<wgpu::Surface<'static>>,
    pub config: Option<SurfaceConfiguration>,
    pub last_applied_generation: u64,
}

pub(crate) fn prepare_wayland_surface(
    descriptor: Res<WaylandSurfaceDescriptor>,
    mut state: ResMut<WaylandGpuSurfaceState>,
    render_instance: Res<RenderInstance>,
    render_adapter: Res<RenderAdapter>,
    render_device: Res<RenderDevice>,
) {
    let valid_outputs: Vec<u32> = descriptor.surfaces.iter().map(|s| s.output).collect();
    state
        .surfaces
        .retain(|output, _| valid_outputs.contains(output));

    for surf_desc in descriptor.surfaces.iter().filter(|s| s.handles.is_some()) {
        let entry = state.surfaces.entry(surf_desc.output).or_default();

        let needs_recreate =
            entry.surface.is_none() || entry.last_applied_generation != descriptor.generation;

        if needs_recreate {
            let handles = surf_desc.handles.expect("handles exist");
            let raw_display_handle = handles.raw_display_handle();
            let raw_window_handle = handles.raw_window_handle();
            let instance = render_instance.0.as_ref();
            let surface = unsafe {
                instance
                    .create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: Some(raw_display_handle),
                        raw_window_handle,
                    })
                    .expect("failed to create Wayland wgpu surface")
            };
            entry.surface = Some(surface);
        }

        let Some(surface) = entry.surface.as_ref() else {
            continue;
        };

        let width = surf_desc.width.max(1);
        let height = surf_desc.height.max(1);

        let needs_reconfigure = entry
            .config
            .as_ref()
            .map(|config| config.width != width || config.height != height)
            .unwrap_or(true);

        if needs_reconfigure || needs_recreate {
            let capabilities = surface.get_capabilities(render_adapter.0.as_ref());
            if capabilities.formats.is_empty() {
                warn!("Wayland surface reported no supported formats; retrying later");
                entry.surface = None;
                entry.config = None;
                entry.last_applied_generation = 0;
                continue;
            }

            let format = capabilities
                .formats
                .iter()
                .copied()
                .find(|fmt| *fmt == WAYLAND_SURFACE_FORMAT)
                .or_else(|| capabilities.formats.first().copied())
                .expect("Wayland surface has no supported formats");

            let present_mode = capabilities
                .present_modes
                .iter()
                .copied()
                .find(|mode| matches!(mode, PresentMode::Fifo))
                .or_else(|| capabilities.present_modes.first().copied())
                .expect("Wayland surface has no supported present mode");

            let alpha_mode = capabilities
                .alpha_modes
                .iter()
                .copied()
                .find(|mode| matches!(mode, CompositeAlphaMode::PreMultiplied))
                .unwrap_or(capabilities.alpha_modes[0]);

            let mut usage = TextureUsages::RENDER_ATTACHMENT;
            if capabilities.usages.contains(TextureUsages::COPY_DST) {
                usage |= TextureUsages::COPY_DST;
            }

            let config = SurfaceConfiguration {
                usage,
                format,
                width,
                height,
                present_mode,
                alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: 1,
            };

            render_device.configure_surface(surface, &config);

            entry.config = Some(config);
        }

        entry.last_applied_generation = descriptor.generation;
    }
}

pub(crate) fn present_wayland_surface(
    mut state: ResMut<WaylandGpuSurfaceState>,
    target: Query<(&MainEntity, &WaylandRenderTarget)>,
    images: Res<RenderAssets<GpuImage>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    descriptor: Res<WaylandSurfaceDescriptor>,
    pipeline: Res<super::premultiply::WaylandBlitPipeline>,
) {
    for (output, entry) in state.surfaces.iter_mut() {
        let Some(surface) = entry.surface.as_ref() else {
            continue;
        };
        let Some(config) = entry.config.as_ref() else {
            continue;
        };

        let Some(desc_entry) = descriptor
            .surfaces
            .iter()
            .find(|s| s.output == *output && s.handles.is_some())
        else {
            continue;
        };

        let Some((_, target)) = target.iter().find(|(e, _)| e.id() == desc_entry.camera) else {
            continue;
        };

        let Some(gpu_image) = images.get(&target.image) else {
            return;
        };

        let extent = Extent3d {
            width: config.width.min(gpu_image.texture_descriptor.size.width),
            height: config.height.min(gpu_image.texture_descriptor.size.height),
            depth_or_array_layers: 1,
        };

        let surface_texture = match surface.get_current_texture() {
            CurrentSurfaceTexture::Success(texture)
            | CurrentSurfaceTexture::Suboptimal(texture) => texture,
            CurrentSurfaceTexture::Outdated => {
                debug!(
                    "Wayland surface for output {} outdated; scheduling reconfigure",
                    output
                );
                entry.config = None;
                entry.last_applied_generation = 0;
                continue;
            }
            CurrentSurfaceTexture::Lost => {
                warn!(
                    "Wayland surface for output {} lost; scheduling recreate",
                    output
                );
                entry.surface = None;
                entry.config = None;
                entry.last_applied_generation = 0;
                continue;
            }
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => {
                debug!("Wayland surface acquire timeout (output {})", output);
                continue;
            }
            CurrentSurfaceTexture::Validation => {
                error!("Wayland surface validation failed (output {})", output);
                continue;
            }
        };

        let mut encoder = render_device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("wayland-surface-present"),
        });
        // Create a view of your surface texture to serve as the render target
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let src_view = gpu_image
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Setup a render pass replacing the encoder.copy_texture_to_texture instruction
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("premultiply_blit_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            let source_texture_bind_group = render_device.create_bind_group(
                "premultiply_blit_bind_group",
                &pipeline.bind_group_layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&src_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&pipeline.sampler), // Use the sampler from Step 2
                    },
                ],
            );

            // Configure your blit pipeline context
            render_pass.set_pipeline(&pipeline.pipeline);
            render_pass.set_bind_group(0, Some(&*source_texture_bind_group), &[]);

            // Draw 3 vertices to execute the full-screen blit shader
            render_pass.draw(0..3, 0..1);
        }

        // Submit and present normally
        render_queue.submit(Some(encoder.finish()));
        surface_texture.present();
    }
}
