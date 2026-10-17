//! Drawing the 3D world on the GPU.
//!
//! The world view hands egui a paint callback holding a [`Mesh`]. Before
//! egui draws, the callback renders the mesh into its own texture, with a
//! depth buffer and 4× multisampling; while egui draws, the texture is
//! copied into the view's rectangle. Rendering through wgpu keeps the same
//! backends as the rest of the app (Vulkan, DirectX 12, Metal or OpenGL).

use std::sync::Arc;

use eframe::egui_wgpu::{self, wgpu, CallbackResources, CallbackTrait, ScreenDescriptor};
use egui::{Color32, Id, Rect};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

const SAMPLES: u32 = 4;
const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// A corner of a triangle: position (relative to the camera's
/// origin, Y up) and colour (sRGB, unmultiplied alpha).
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub color: [u8; 4],
}

/// Everything to draw in one frame.
#[derive(Default)]
pub struct Mesh {
    /// Opaque triangles.
    pub solid: Vec<Vertex>,
    /// See-through triangles, drawn after the solid ones.
    pub ghost: Vec<Vertex>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    /// Background colour (rgb) and the distance where fog is full (w).
    fog: [f32; 4],
}

/// The paint callback for one frame of the world view.
pub struct Scene {
    pub mesh: Arc<Mesh>,
    pub view_proj: Mat4,
    pub eye: Vec3,
    pub fog: f32,
    pub background: Color32,
    /// Size of the view in physical pixels.
    pub size: [u32; 2],
    /// Format egui draws into.
    pub target: wgpu::TextureFormat,
}

/// Remember the format egui renders into (the window's), so the world view
/// can draw into it. Without it, `Rgba8Unorm` is assumed (headless tests).
pub fn remember_target_format(ctx: &egui::Context, format: wgpu::TextureFormat) {
    ctx.data_mut(|d| d.insert_temp(Id::unique("makine-wgpu-format"), format));
}

pub fn target_format(ctx: &egui::Context) -> wgpu::TextureFormat {
    ctx.data(|d| d.get_temp(Id::unique("makine-wgpu-format")))
        .unwrap_or(wgpu::TextureFormat::Rgba8Unorm)
}

impl Scene {
    pub fn paint(self, painter: &egui::Painter, rect: Rect) {
        painter.add(egui_wgpu::Callback::new_paint_callback(rect, self));
    }
}

/// Pipelines and the off-screen textures, kept between frames.
struct Gpu {
    target: wgpu::TextureFormat,
    solid: wgpu::RenderPipeline,
    ghost: wgpu::RenderPipeline,
    blit: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    uniform_group: wgpu::BindGroup,
    blit_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    targets: Option<Targets>,
}

/// Textures of one size.
struct Targets {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    resolved: wgpu::TextureView,
    blit_group: wgpu::BindGroup,
}

const SHADER: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    fog: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) at: vec3<f32>,
};

@vertex
fn vs(@location(0) pos: vec3<f32>, @location(1) color: vec4<f32>) -> VOut {
    var o: VOut;
    o.pos = u.view_proj * vec4<f32>(pos, 1.0);
    o.color = color;
    o.at = pos;
    return o;
}

@fragment
fn fs(i: VOut) -> @location(0) vec4<f32> {
    // Far things fade into the background, which helps to see depth.
    let d = distance(i.at, u.eye.xyz);
    let f = clamp((d - u.fog.w * 0.45) / (u.fog.w * 0.55), 0.0, 0.85);
    return vec4<f32>(mix(i.color.rgb, u.fog.rgb, f), i.color.a);
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

struct BOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn blit_vs(@builtin(vertex_index) i: u32) -> BOut {
    // One triangle covering the viewport.
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: BOut;
    o.pos = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    o.uv = uv;
    return o;
}

@fragment
fn blit_fs(i: BOut) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(tex, samp, i.uv).rgb, 1.0);
}

// For windows whose format converts to sRGB itself: the scene is drawn in
// sRGB values like egui, so undo that conversion.
@fragment
fn blit_fs_srgb(i: BOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, i.uv).rgb;
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return vec4<f32>(select(high, low, c < vec3<f32>(0.04045)), 1.0);
}
"#;

impl Gpu {
    fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("makine-world"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("makine-world-uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("makine-world-uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("makine-world-uniforms"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("makine-world"),
            bind_group_layouts: &[Some(&uniform_layout)],
            immediate_size: 0,
        });
        let vertex = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4],
        };
        let blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };
        let pipeline = |write_depth: bool, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("makine-world"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    buffers: &[Some(vertex.clone())],
                    compilation_options: Default::default(),
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(write_depth),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let solid = pipeline(true, None);
        let ghost = pipeline(false, Some(blend));

        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("makine-world-blit"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("makine-world-blit"),
            bind_group_layouts: &[Some(&blit_layout)],
            immediate_size: 0,
        });
        let blit = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("makine-world-blit"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("blit_vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(if target.is_srgb() {
                    "blit_fs_srgb"
                } else {
                    "blit_fs"
                }),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("makine-world-blit"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            target,
            solid,
            ghost,
            blit,
            uniforms,
            uniform_group,
            blit_layout,
            sampler,
            targets: None,
        }
    }

    fn targets(&mut self, device: &wgpu::Device, size: [u32; 2]) -> &Targets {
        if self.targets.as_ref().is_none_or(|t| t.size != size) {
            let texture = |format, samples, usage| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("makine-world"),
                        size: wgpu::Extent3d {
                            width: size[0],
                            height: size[1],
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: samples,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
            let resolved = texture(COLOR, 1, attach | wgpu::TextureUsages::TEXTURE_BINDING);
            let blit_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("makine-world-blit"),
                layout: &self.blit_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&resolved),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.targets = Some(Targets {
                size,
                msaa: texture(COLOR, SAMPLES, attach),
                depth: texture(DEPTH, SAMPLES, attach),
                resolved,
                blit_group,
            });
        }
        self.targets.as_ref().unwrap()
    }
}

fn linear(c: u8) -> f64 {
    c as f64 / 255.0
}

impl CallbackTrait for Scene {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if self.size[0] == 0 || self.size[1] == 0 {
            return Vec::new();
        }
        if resources
            .get::<Gpu>()
            .is_none_or(|g| g.target != self.target)
        {
            resources.insert(Gpu::new(device, self.target));
        }
        let gpu: &mut Gpu = resources.get_mut().unwrap();
        let bg = self.background;
        queue.write_buffer(
            &gpu.uniforms,
            0,
            bytemuck::bytes_of(&Uniforms {
                view_proj: self.view_proj.to_cols_array_2d(),
                eye: self.eye.extend(1.0).to_array(),
                fog: [
                    bg.r() as f32 / 255.0,
                    bg.g() as f32 / 255.0,
                    bg.b() as f32 / 255.0,
                    self.fog,
                ],
            }),
        );
        let buffer = |v: &[Vertex]| {
            (!v.is_empty()).then(|| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("makine-world"),
                    contents: bytemuck::cast_slice(v),
                    usage: wgpu::BufferUsages::VERTEX,
                })
            })
        };
        let draws = [
            (&gpu.solid, buffer(&self.mesh.solid), self.mesh.solid.len()),
            (&gpu.ghost, buffer(&self.mesh.ghost), self.mesh.ghost.len()),
        ];
        let draws: Vec<_> = draws
            .into_iter()
            .map(|(p, b, n)| (p.clone(), b, n))
            .collect();
        let (uniform_group, size) = (gpu.uniform_group.clone(), self.size);
        let t = gpu.targets(device, size);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("makine-world"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &t.msaa,
                resolve_target: Some(&t.resolved),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: linear(bg.r()),
                        g: linear(bg.g()),
                        b: linear(bg.b()),
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Discard,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &t.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &uniform_group, &[]);
        for (pipeline, buffer, n) in &draws {
            if let Some(b) = buffer {
                pass.set_pipeline(pipeline);
                pass.set_vertex_buffer(0, b.slice(..));
                pass.draw(0..*n as u32, 0..1);
            }
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let Some(gpu) = resources.get::<Gpu>() else {
            return;
        };
        let Some(t) = &gpu.targets else { return };
        pass.set_pipeline(&gpu.blit);
        pass.set_bind_group(0, &t.blit_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
