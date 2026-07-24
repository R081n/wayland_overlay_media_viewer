use bevy::{
    pbr::MaterialPlugin,
    prelude::*,
    reflect::TypePath,
    render::render_resource::AsBindGroup,
};

pub struct DynamicShaderPlugin;

impl Plugin for DynamicShaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<DynamicMaterial>::default())
            .add_systems(Update, tick_and_update_shader_timers);
    }
}

// --- Main World Components ---

#[derive(Component, Debug, Clone)]
pub struct ShaderRepeatPeriod {
    pub timer: Timer,
}

impl ShaderRepeatPeriod {
    pub fn new(seconds: f32) -> Self {
        Self {
            timer: Timer::from_seconds(seconds, TimerMode::Repeating),
        }
    }
}

// --- Custom Material Extension Definition ---

#[derive(Asset, AsBindGroup, TypePath, Clone, Debug)]
#[bind_group_data(DynamicMateiralKey)]
pub struct DynamicMaterial {
    // Shared loop progress parameter mapped to @group(2) @binding(100)
    #[uniform(100)]
    pub p_progress: f32,
    #[uniform(101)]
    pub p_opacity: f32,
    #[uniform(102)]
    pub p_scale: Vec2,

    pub shader: Handle<Shader>,
}

impl DynamicMaterial {
    pub fn new(opacity: f32, shader: Handle<Shader>) -> Self {
        Self {
            p_progress: 0.0,
            p_opacity: opacity,
            p_scale: Vec2::ONE,
            shader,
        }
    }
}

impl From<&DynamicMaterial> for DynamicMateiralKey {
    fn from(value: &DynamicMaterial) -> Self {
        Self {
            shader: value.shader.clone(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct DynamicMateiralKey {
    shader: Handle<Shader>,
}

// #[derive(thiserror::Error, Debug)]
// pub enum DynamicMaterialError {
//     #[error("Unknown error")]
//     Unknown,
// }

// #[derive(Default, TypePath)]
// struct DynamicMaterialLoader;

// impl AssetLoader for DynamicMaterialLoader {
//     type Asset = DynamicMaterial;

//     type Settings = ();

//     type Error = DynamicMaterialError;

//     fn load(
//         &self,
//         reader: &mut dyn bevy::asset::io::Reader,
//         settings: &Self::Settings,
//         load_context: &mut bevy::asset::LoadContext,
//     ) -> impl bevy::tasks::ConditionalSendFuture<
//         Output = std::prelude::v1::Result<Self::Asset, Self::Error>,
//     > {
//         todo!()
//     }
// }

// Implement MaterialExtension on our clean, cloneable structural layout
impl Material for DynamicMaterial {
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::material::descriptor::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::material::specialize::SpecializedMeshPipelineError> {
        let b = key.bind_group_data;

        if let Some(fragment) = &mut descriptor.fragment {
            fragment.shader = b.shader;
        };

        Ok(())
    }
}

// --- System Architectures ---

fn tick_and_update_shader_timers(
    time: Res<Time>,
    mut materials: ResMut<Assets<DynamicMaterial>>,
    mut query: Query<(
        &mut ShaderRepeatPeriod,
        &Transform,
        &MeshMaterial3d<DynamicMaterial>,
    )>,
) {
    for (mut repeat_period, transform, material_handle) in &mut query {
        repeat_period.timer.tick(time.delta());
        if let Some(mut extended_material) = materials.get_mut(material_handle) {
            extended_material.p_progress = repeat_period.timer.fraction() * std::f32::consts::TAU;
            extended_material.p_scale = transform.scale.xy();
        }
    }
}

// --- Dynamic Runtime Setup ---

pub const SHADER_TEMPLATE: &str = r#"
    #import bevy_pbr::forward_io::VertexOutput

    @group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> p_progress: f32;
    @group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> p_opacity: f32;
    @group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> p_scale: vec2<f32>;

    @fragment
    fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
       ###CodeHere###
    }
"#;
