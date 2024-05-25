use std::{iter, num::NonZeroU32};

use image::{DynamicImage, GenericImageView};
use wgpu::{
    util::DeviceExt, BindGroup, BindGroupLayout, Buffer, Sampler, TextureView,
};
use winit::window::Window;

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);

pub struct Context<'a> {
    pub surface: wgpu::Surface<'a>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub render_pipeline: wgpu::RenderPipeline,
    pub window: &'a Window,

    pub projection_matrix_bytes: [u8; 64],
    pub projection_buffer: Buffer,

    pub rectangles_to_render: Vec<RectangleDrawData>,
    pub rectangles_buffer: Buffer,

    pub uniform_bind_group: BindGroup,

    // this bind group is recreated each time a texture is added, so it's
    // easier to also store the layout here
    pub textures_bind_group_layout: BindGroupLayout,
    pub textures_bind_group: BindGroup,

    pub sampler: Sampler,
    pub empty_texture: Texture, /* used to fill in the empty entries in
                                 * textures_bind_group */
    pub textures: Vec<Texture>,
}

pub type TextureHandle = usize;

pub struct Texture {
    pub wgpu_texture: wgpu::Texture,
    pub wgpu_texture_view: wgpu::TextureView,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::NoUninit)]
pub struct RectangleDrawData {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 3],
    pub _padding: [u8; 4],
}

impl<'a> Context<'a> {
    pub fn new(window: &'a Window) -> Context<'a> {
        let size = window.inner_size();

        // BORING BOILERPLATE
        // ==================

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ))
        .unwrap();

        let mut required_limits = wgpu::Limits::default();
        required_limits.max_sampled_textures_per_shader_stage = 1000;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::TEXTURE_BINDING_ARRAY,
                required_limits,
            },
            None,
        ))
        .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);

        // srgb surface format (or fall back to the first one)
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        // TEXTURES
        // ========

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // start with 0 textures
        let textures: Vec<Texture> = vec![];

        let empty_texture = create_texture_from_raw_data(
            &device,
            &queue,
            &DynamicImage::new(1, 1, image::ColorType::Rgba8),
        );

        // BUFFERS
        // =======

        let projection_matrix_bytes = Self::calculate_projection_matrix(
            size.width as f32,
            size.height as f32,
        );

        let projection_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Projection Buffer"),
                contents: &projection_matrix_bytes,
                usage: wgpu::BufferUsages::UNIFORM
                    | wgpu::BufferUsages::COPY_DST,
            });

        let rectangles_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Rectangles Buffer"),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            size: 10000 * std::mem::size_of::<RectangleDrawData>() as u64,
            mapped_at_creation: false,
        });

        // UNIFORM BIND GROUP
        // ==================

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage {
                                read_only: true,
                            },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(
                            wgpu::SamplerBindingType::Filtering,
                        ),
                        count: None,
                    },
                ],
                label: Some("Uniform bind group layout"),
            });

        let uniform_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &uniform_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: projection_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: rectangles_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
                label: Some("Uniform bind group"),
            });

        // TEXTURES BIND GROUP
        // ===================

        let textures_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float {
                            filterable: true,
                        },
                    },
                    count: NonZeroU32::new(1000),
                }],
                label: Some("Textures bind group layout"),
            });

        let textures_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &textures_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureViewArray(
                        &[&empty_texture.wgpu_texture_view; 1000],
                    ),
                }],
                label: Some("Textures bind group"),
            });

        // PIPELINE
        // ========

        let shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shader.wgsl").into(),
                ),
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &uniform_bind_group_layout,
                    &textures_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent::REPLACE,
                            alpha: wgpu::BlendComponent::REPLACE,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    // no culling since I'm only drawing rectangles!!!!!
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            });

        Self {
            surface,
            device,
            queue,
            size,
            config,
            render_pipeline,
            window,
            projection_matrix_bytes,
            projection_buffer,
            rectangles_to_render: vec![
                RectangleDrawData {
                    pos: [10.0, 10.0],
                    size: [100.0, 100.0],
                    color: [1.0, 1.0, 1.0],
                    _padding: [0, 0, 0, 0],
                },
                RectangleDrawData {
                    pos: [120.0, 20.0],
                    size: [100.0, 100.0],
                    color: [1.0, 0.5, 1.0],
                    _padding: [0, 0, 0, 0],
                },
                RectangleDrawData {
                    pos: [230.0, 50.0],
                    size: [100.0, 150.0],
                    color: [0.4, 0.3, 0.3],
                    _padding: [0, 0, 0, 0],
                },
            ],
            rectangles_buffer,
            uniform_bind_group,
            textures_bind_group_layout,
            textures_bind_group,
            sampler,
            empty_texture,
            textures,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;

            // RECONFIGURE SURFACE
            // ===================

            self.config.width = new_size.width;
            self.config.height = new_size.height;

            self.surface.configure(&self.device, &self.config);

            // UPDATE PROJECTION MATRIX
            // ========================

            self.projection_matrix_bytes = Self::calculate_projection_matrix(
                new_size.width as f32,
                new_size.height as f32,
            );

            self.queue.write_buffer(
                &self.projection_buffer,
                0,
                &self.projection_matrix_bytes,
            );
        }
    }

    pub fn update(&mut self) {}

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            },
        );

        self.queue.write_buffer(
            &self.rectangles_buffer,
            0,
            bytemuck::cast_slice(self.rectangles_to_render.as_slice()),
        );

        {
            let mut render_pass =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(
                        wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.2,
                                    b: 0.3,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        },
                    )],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.textures_bind_group, &[]);

            let vertex_count = 6 * self.rectangles_to_render.len() as u32;
            render_pass.draw(0..vertex_count, 0..1);
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    pub fn create_texture_from_raw_data(
        &mut self,
        data: &DynamicImage,
    ) -> Result<TextureHandle, &str> {
        let texture =
            create_texture_from_raw_data(&self.device, &self.queue, data);

        self.textures.push(texture);

        // UPDATE BIND GROUP
        // =================

        let mut texture_views: Vec<&wgpu::TextureView> = vec![];
        for texture in self.textures.iter() {
            texture_views.push(&texture.wgpu_texture_view);
        }

        // fill the rest with an empty texture view
        for i in texture_views.len()..1000 {
            texture_views.push(&self.empty_texture.wgpu_texture_view)
        }

        self.textures_bind_group =
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.textures_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureViewArray(
                        &texture_views,
                    ),
                }],
                label: Some("Textures bind group"),
            });

        // return index of the added texture
        Ok(self.textures.len() - 1)
    }

    pub fn create_texture_from_path(
        &mut self,
        path: &str,
    ) -> Result<TextureHandle, &str> {
        // LOAD IMAGE DATA
        // ===============

        let img = image::io::Reader::open(path);
        if img.is_err() {
            return Err("Could not open file.");
        }
        let img = img.unwrap();

        let decoded_img = img.decode();
        if decoded_img.is_err() {
            return Err("Could not decode image data.");
        }
        let decoded_img = decoded_img.unwrap();

        return self.create_texture_from_raw_data(&decoded_img);
    }

    fn calculate_projection_matrix(
        window_width: f32,
        window_height: f32,
    ) -> [u8; 64] {
        let matrix = OPENGL_TO_WGPU_MATRIX
            * cgmath::ortho(0.0, window_width, window_height, 0.0, -1.0, 1.0);

        let matrix_transformed: [[f32; 4]; 4] = matrix.into();

        // lol unsafe I don't care
        unsafe {
            std::mem::transmute::<[[f32; 4]; 4], [u8; 64]>(matrix_transformed)
        }
    }
}

pub fn create_texture_from_raw_data(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &DynamicImage,
) -> Texture {
    let rgba = data.to_rgba8();
    let dimensions = data.dimensions();

    // CREATE WGPU TEXTURE
    // ===================

    let texture_size = wgpu::Extent3d {
        width: dimensions.0,
        height: dimensions.1,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST,
        label: Some("Texture created from raw data"),
        view_formats: &[],
    });

    // WRITE TO WGPU TEXTURE
    // =====================

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * dimensions.0),
            rows_per_image: Some(dimensions.1),
        },
        texture_size,
    );

    let texture_view =
        texture.create_view(&wgpu::TextureViewDescriptor::default());

    return Texture {
        wgpu_texture: texture,
        wgpu_texture_view: texture_view,
    };
}
