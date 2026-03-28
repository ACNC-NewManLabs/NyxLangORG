use wgpu::*;
use wgpu::util::{DeviceExt, BufferInitDescriptor};
use crate::graphics::renderer::display_list::{DisplayList, DrawOp};
use crate::graphics::renderer::atlas::TextureAtlas;

pub struct WgpuRenderer {
    device: Device,
    queue: Queue,
    surface: Surface,
    config: SurfaceConfiguration,
    pipeline: RenderPipeline,
    bind_group: BindGroup,
    uniform_buffer: Buffer,
    _texture_atlas: TextureAtlas,
    
    // Performance tracking
    frame_count: u64,
    draw_calls: usize,
    last_frame_time: std::time::Instant,
}

impl WgpuRenderer {
    pub async fn new(window: &'static winit::window::Window) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        let surface = unsafe { instance.create_surface(window) }.unwrap();
        
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or("Failed to find an appropriate adapter")?;

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("Device"),
                    features: Features::empty(),
                    limits: Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| format!("Failed to create device: {}", e))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: window.inner_size().width,
            height: window.inner_size().height,
            present_mode: PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        // Create shader
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shader"),
            source: ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: 64, // 4x4 matrix + padding
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create texture atlas
        let texture_atlas = TextureAtlas::new(&device, &queue, 2048, 2048)?;

        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&texture_atlas.view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&texture_atlas.sampler),
                },
            ],
        });

        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: config.format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Ok(Self {
            device,
            queue,
            surface,
            config,
            pipeline,
            bind_group,
            uniform_buffer,
            _texture_atlas: texture_atlas,
            frame_count: 0,
            draw_calls: 0,
            last_frame_time: std::time::Instant::now(),
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn render(&mut self, display_list: &DisplayList) -> Result<(), Box<dyn std::error::Error>> {
        let frame_start = std::time::Instant::now();
        
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&TextureViewDescriptor::default());

        // Update uniforms
        let transform = [
            2.0 / self.config.width as f32, 0.0, 0.0, 0.0,
            0.0, -2.0 / self.config.height as f32, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            -1.0, 1.0, 0.0, 1.0,
        ];

        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&transform));

        // Create vertex buffer from display list
        let vertices = self.display_list_to_vertices(display_list);
        let vertex_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: BufferUsages::VERTEX,
        });

        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.draw(0..vertices.len() as u32, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Update performance metrics
        self.frame_count += 1;
        self.draw_calls = display_list.ops.len();
        self.last_frame_time = frame_start;

        Ok(())
    }

    fn display_list_to_vertices(&self, display_list: &DisplayList) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        
        for op in &display_list.ops {
            match op {
                DrawOp::SolidQuad { x, y, width, height, color } => {
                    let x0 = *x;
                    let y0 = *y;
                    let x1 = x0 + width;
                    let y1 = y0 + height;
                    
                    vertices.push(Vertex { position: [x0, y0], color: *color, uv: [0.0, 0.0] });
                    vertices.push(Vertex { position: [x1, y0], color: *color, uv: [1.0, 0.0] });
                    vertices.push(Vertex { position: [x0, y1], color: *color, uv: [0.0, 1.0] });
                    vertices.push(Vertex { position: [x1, y0], color: *color, uv: [1.0, 0.0] });
                    vertices.push(Vertex { position: [x1, y1], color: *color, uv: [1.0, 1.0] });
                    vertices.push(Vertex { position: [x0, y1], color: *color, uv: [0.0, 1.0] });
                }
                DrawOp::GlyphRun { x, y, glyphs: _, color } => {
                    // Simplified glyph rendering
                    let x0 = *x;
                    let y0 = *y;
                    let x1 = x0 + 10.0; // Placeholder size
                    let y1 = y0 + 12.0;
                    
                    vertices.push(Vertex { position: [x0, y0], color: *color, uv: [0.0, 0.0] });
                    vertices.push(Vertex { position: [x1, y0], color: *color, uv: [1.0, 0.0] });
                    vertices.push(Vertex { position: [x0, y1], color: *color, uv: [0.0, 1.0] });
                    vertices.push(Vertex { position: [x1, y0], color: *color, uv: [1.0, 0.0] });
                    vertices.push(Vertex { position: [x1, y1], color: *color, uv: [1.0, 1.0] });
                    vertices.push(Vertex { position: [x0, y1], color: *color, uv: [0.0, 1.0] });
                }
                DrawOp::Image { x, y, width, height, image_id: _ } => {
                    let x0 = *x;
                    let y0 = *y;
                    let x1 = x0 + width;
                    let y1 = y0 + height;
                    
                    vertices.push(Vertex { position: [x0, y0], color: [1.0, 1.0, 1.0, 1.0], uv: [0.0, 0.0] });
                    vertices.push(Vertex { position: [x1, y0], color: [1.0, 1.0, 1.0, 1.0], uv: [1.0, 0.0] });
                    vertices.push(Vertex { position: [x0, y1], color: [1.0, 1.0, 1.0, 1.0], uv: [0.0, 1.0] });
                    vertices.push(Vertex { position: [x1, y0], color: [1.0, 1.0, 1.0, 1.0], uv: [1.0, 0.0] });
                    vertices.push(Vertex { position: [x1, y1], color: [1.0, 1.0, 1.0, 1.0], uv: [1.0, 1.0] });
                    vertices.push(Vertex { position: [x0, y1], color: [1.0, 1.0, 1.0, 1.0], uv: [0.0, 1.0] });
                }
                DrawOp::ClipRect { x, y, width, height } => {
                    // Implement clipping via stencil buffer
                    // For now, just draw a border
                    let x0 = *x;
                    let y0 = *y;
                    let x1 = x0 + width;
                    let y1 = y0 + height;
                    let color = [1.0, 0.0, 0.0, 1.0];
                    
                    vertices.push(Vertex { position: [x0, y0], color, uv: [0.0, 0.0] });
                    vertices.push(Vertex { position: [x1, y0], color, uv: [1.0, 0.0] });
                    vertices.push(Vertex { position: [x0, y1], color, uv: [0.0, 1.0] });
                    vertices.push(Vertex { position: [x1, y0], color, uv: [1.0, 0.0] });
                    vertices.push(Vertex { position: [x1, y1], color, uv: [1.0, 1.0] });
                    vertices.push(Vertex { position: [x0, y1], color, uv: [0.0, 1.0] });
                }
                DrawOp::Transform(_) => {
                    // Transform handling would be more complex
                    // For now, ignore
                }
            }
        }
        
        vertices
    }

    pub fn get_performance_stats(&self) -> RendererStats {
        RendererStats {
            frame_count: self.frame_count,
            draw_calls: self.draw_calls,
            frame_time_ms: self.last_frame_time.elapsed().as_millis() as f32,
            fps: if self.last_frame_time.elapsed().as_secs_f32() > 0.0 {
                1000.0 / self.last_frame_time.elapsed().as_millis() as f32
            } else {
                0.0
            },
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
    uv: [f32; 2],
}

impl Vertex {
    fn desc<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x2,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Float32x4,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as BufferAddress,
                    shader_location: 2,
                    format: VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[derive(Debug)]
pub struct RendererStats {
    pub frame_count: u64,
    pub draw_calls: usize,
    pub frame_time_ms: f32,
    pub fps: f32,
}

// Removed custom DeviceExt trait here as wgpu::util::DeviceExt is used instead
