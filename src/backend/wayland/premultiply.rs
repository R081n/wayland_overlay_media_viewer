use bevy::{
    prelude::*,
    render::{RenderApp, render_resource::*, renderer::RenderDevice},
};

/// Resource stored in the Render App world containing the persistent pipeline data
#[derive(Resource)]
pub struct WaylandBlitPipeline {
    pub pipeline: RenderPipeline,
    pub bind_group_layout: BindGroupLayout,
    pub sampler: Sampler,
}

pub struct WaylandPresentPlugin;

impl Plugin for WaylandPresentPlugin {
    fn finish(&self, app: &mut App) {
        // We get the RenderApp sub-world to initialize our GPU resources
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                // Step 1: Initialize the pipeline once the RenderDevice is ready
                .init_resource::<WaylandPipelineCache>();
        }
    }

    fn build(&self, _app: &mut App) {}
}

/// Helper to handle FromWorld initialization using RenderDevice
#[derive(Resource)]
struct WaylandPipelineCache;

impl FromWorld for WaylandBlitPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        // 1. Create Bind Group Layout
        let bind_group_layout = render_device.create_bind_group_layout(
            "wayland_blit_layout",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        );

        // 2. Load the embedded WGSL shader code directly
        let shader_module = unsafe {
            render_device.create_shader_module(ShaderModuleDescriptor {
                label: Some("wayland_premultiply_shader"),
                source: ShaderSource::Wgsl(include_str!("premultiply_blit.wgsl").into()),
            })
        };

        // 3. Create Pipeline Layout
        let pipeline_layout = render_device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("wayland_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        // 4. Create Reusable Sampler
        let sampler = render_device.create_sampler(&SamplerDescriptor {
            label: Some("wayland_blit_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        // 5. Create Render Pipeline
        // Note: Change TextureFormat to match your surface format if it differs from Bgra8UnormSrgb
        let pipeline = render_device.create_render_pipeline(&RawRenderPipelineDescriptor {
            label: Some("wayland_blit_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: RawVertexState {
                module: &shader_module,
                compilation_options: Default::default(),
                entry_point: Some("vs_main"),
                buffers: &[],
            },
            fragment: Some(RawFragmentState {
                module: &shader_module,
                compilation_options: Default::default(),
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Bgra8UnormSrgb,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        WaylandBlitPipeline {
            pipeline,
            bind_group_layout,
            sampler,
        }
    }
}

// Automatically initializes the resource in the Render World on startup
impl FromWorld for WaylandPipelineCache {
    fn from_world(world: &mut World) -> Self {
        let pipeline = WaylandBlitPipeline::from_world(world);
        world.insert_resource(pipeline);
        WaylandPipelineCache
    }
}
