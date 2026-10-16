use super::*;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_projection: [[f32; 4]; 4],
    light_direction: [f32; 4],
    light_color: [f32; 4],
    ambient: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstanceData {
    model: [[f32; 4]; 4],
    color: [f32; 4],
    skin_base: u32,
    skin_count: u32,
    _padding: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CullInstanceData {
    model: [[f32; 4]; 4],
    color: [f32; 4],
    bounds: [f32; 4],
    skin_base: u32,
    skin_count: u32,
    _padding: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaterialUniform {
    base_color: [f32; 4],
    metallic_roughness: [f32; 4],
    texture_flags: [f32; 4],
}

struct GpuMaterial {
    _buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

struct GpuMesh {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    index_count: u32,
}

struct GpuTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct GpuBatch {
    mesh: AssetId<MeshAsset>,
    material: AssetId<MaterialAsset>,
    buffer: wgpu::Buffer,
    indirect: wgpu::Buffer,
    capacity: usize,
    count: u32,
}

struct GpuDrivenBatch {
    mesh: AssetId<MeshAsset>,
    material: AssetId<MaterialAsset>,
    input: wgpu::Buffer,
    output: wgpu::Buffer,
    counter: wgpu::Buffer,
    indirect: wgpu::Buffer,
    params: wgpu::Buffer,
    count: u32,
    cull_bind_group: wgpu::BindGroup,
    finalize_bind_group: wgpu::BindGroup,
    capacity: usize,
}

pub struct ForwardFeature {
    pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    globals_layout: wgpu::BindGroupLayout,
    globals_bind_group: wgpu::BindGroup,
    joint_buffer: wgpu::Buffer,
    material_layout: wgpu::BindGroupLayout,
    default_texture: GpuTexture,
    default_sampler: wgpu::Sampler,
    default_material: GpuMaterial,
    meshes: HashMap<AssetId<MeshAsset>, GpuMesh>,
    textures: HashMap<AssetId<TextureAsset>, GpuTexture>,
    materials: HashMap<AssetId<MaterialAsset>, MaterialAsset>,
    pbr_materials: HashMap<AssetId<MaterialAsset>, GpuMaterial>,
    batches: Vec<GpuBatch>,
    gpu_batches: Vec<GpuDrivenBatch>,
    scratch_groups: Vec<(
        AssetId<MeshAsset>,
        AssetId<MaterialAsset>,
        Vec<InstanceData>,
    )>,
    scratch_bounds: Vec<Vec<[f32; 4]>>,
    instances: Vec<RenderInstance>,
    skins: Vec<RenderSkin>,
    snapshot_skin_fallbacks: usize,
    camera: Option<RenderCamera>,
    light: Option<RenderLight>,
    viewport: [u32; 2],
    stats: RenderStats,
    preference: RenderPreference,
    path: RenderPath,
    cull_pipeline: Option<wgpu::ComputePipeline>,
    finalize_pipeline: Option<wgpu::ComputePipeline>,
    cull_layout: Option<wgpu::BindGroupLayout>,
    finalize_layout: Option<wgpu::BindGroupLayout>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStats {
    pub instances: usize,
    pub batches: usize,
    pub draw_calls: usize,
    pub instance_upload_bytes: u64,
    pub skipped_instances: usize,
    pub fallback: bool,
    pub culled_instances: usize,
    pub indirect_draws: usize,
    pub path: RenderPath,
    pub gpu_cull_dispatches: usize,
    pub gpu_skinned_instances: usize,
    pub skin_fallbacks: usize,
    /// Visibility counters remain on the device for this frame.
    pub gpu_visibility_pending: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GpuCullingConfig {
    pub enabled: bool,
}

impl ForwardFeature {
    pub(crate) fn new(
        device: &Device,
        format: wgpu::TextureFormat,
        viewport: [u32; 2],
        preference: RenderPreference,
        capabilities: RenderCapabilities,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quasar-forward-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("forward.wgsl").into()),
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globals"),
            contents: bytemuck::bytes_of(&Globals::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let joint_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("joint-matrices"),
            contents: bytemuck::bytes_of(&Mat4::IDENTITY.to_cols_array_2d()),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals-bind-group"),
            layout: &globals_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: joint_buffer.as_entire_binding(),
                },
            ],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let default_texture_gpu = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("default-material-texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let default_texture = GpuTexture {
            view: default_texture_gpu.create_view(&wgpu::TextureViewDescriptor::default()),
            _texture: default_texture_gpu,
        };
        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let default_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("default-material"),
            contents: bytemuck::bytes_of(&MaterialUniform {
                base_color: [1.0; 4],
                metallic_roughness: [0.0, 0.5, 0.0, 0.0],
                texture_flags: [0.0; 4],
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let default_material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("default-material-bind-group"),
            layout: &material_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: default_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&default_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&default_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&default_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&default_texture.view),
                },
            ],
        });
        let (cull_pipeline, finalize_pipeline, cull_layout, finalize_layout) = if capabilities
            .supports_gpu_driven()
        {
            let cull_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("gpu-cull-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    buffer_layout_entry(4, wgpu::BufferBindingType::Uniform),
                ],
            });
            let finalize_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("gpu-finalize-layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });
            let cull_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("quasar-gpu-cull-shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("cull.wgsl").into()),
            });
            let finalize_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("quasar-gpu-finalize-shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("finalize.wgsl").into()),
            });
            let cull_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("gpu-cull-pipeline-layout"),
                    bind_group_layouts: &[&cull_layout],
                    push_constant_ranges: &[],
                });
            let finalize_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("gpu-finalize-pipeline-layout"),
                    bind_group_layouts: &[&finalize_layout],
                    push_constant_ranges: &[],
                });
            let cull_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("gpu-cull-pipeline"),
                layout: Some(&cull_pipeline_layout),
                module: &cull_shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
            let finalize_pipeline =
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("gpu-finalize-pipeline"),
                    layout: Some(&finalize_pipeline_layout),
                    module: &finalize_shader,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                });
            (
                Some(cull_pipeline),
                Some(finalize_pipeline),
                Some(cull_layout),
                Some(finalize_layout),
            )
        } else {
            (None, None, None, None)
        };
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("forward-layout"),
            bind_group_layouts: &[&globals_layout, &material_layout],
            push_constant_ranges: &[],
        });
        let attributes = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x4,
            3 => Float32x2,
            4 => Uint16x4,
            5 => Float32x4
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("forward-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<MeshVertexPbr>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attributes,
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<InstanceData>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            6 => Float32x4,
                            7 => Float32x4,
                            8 => Float32x4,
                            9 => Float32x4,
                            10 => Float32x4,
                            11 => Uint32,
                            12 => Uint32,
                        ],
                    },
                ],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });
        Self {
            pipeline,
            globals_buffer,
            globals_layout,
            globals_bind_group,
            joint_buffer,
            material_layout,
            default_texture,
            default_sampler,
            default_material: GpuMaterial {
                _buffer: default_buffer,
                bind_group: default_material_bind_group,
            },
            meshes: HashMap::new(),
            textures: HashMap::new(),
            materials: HashMap::new(),
            pbr_materials: HashMap::new(),
            batches: Vec::new(),
            gpu_batches: Vec::new(),
            scratch_groups: Vec::new(),
            scratch_bounds: Vec::new(),
            instances: Vec::new(),
            skins: Vec::new(),
            snapshot_skin_fallbacks: 0,
            camera: None,
            light: None,
            viewport,
            stats: RenderStats::default(),
            preference,
            path: preference
                .select(capabilities)
                .expect("validated render preference"),
            cull_pipeline,
            finalize_pipeline,
            cull_layout,
            finalize_layout,
        }
    }

    pub(crate) fn upload_mesh(
        &mut self,
        device: &Device,
        handle: Handle<MeshAsset>,
        mesh: &MeshAsset,
    ) {
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return;
        }
        let vertices = mesh
            .vertices
            .iter()
            .map(|vertex| MeshVertexPbr {
                position: vertex.position,
                normal: vertex.normal,
                tangent: [1.0, 0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
                joints: [0; 4],
                weights: [1.0, 0.0, 0.0, 0.0],
            })
            .collect::<Vec<_>>();
        let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-indices"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.meshes.insert(
            handle.id,
            GpuMesh {
                vertex,
                index,
                index_count: mesh.indices.len() as u32,
            },
        );
    }

    pub(crate) fn upload_textured_mesh(
        &mut self,
        device: &Device,
        handle: Handle<MeshAsset>,
        mesh: &TexturedMeshAsset,
    ) {
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return;
        }
        let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-pbr-vertices"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-indices"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.meshes.insert(
            handle.id,
            GpuMesh {
                vertex,
                index,
                index_count: mesh.indices.len() as u32,
            },
        );
    }

    pub(crate) fn upload_texture(
        &mut self,
        device: &Device,
        queue: &Queue,
        handle: Handle<TextureAsset>,
        texture: &TextureAsset,
    ) {
        if texture.width == 0 || texture.height == 0 || texture.rgba8.is_empty() {
            return;
        }
        let gpu = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("quasar-texture"),
            size: wgpu::Extent3d {
                width: texture.width,
                height: texture.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &gpu,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &texture.rgba8,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * texture.width),
                rows_per_image: Some(texture.height),
            },
            wgpu::Extent3d {
                width: texture.width,
                height: texture.height,
                depth_or_array_layers: 1,
            },
        );
        let view = gpu.create_view(&wgpu::TextureViewDescriptor::default());
        self.textures.insert(
            handle.id,
            GpuTexture {
                _texture: gpu,
                view,
            },
        );
    }

    pub(crate) fn gpu_resource_count(&self) -> usize {
        self.meshes.len()
            + self.materials.len()
            + self.batches.len()
            + self.gpu_batches.len()
            + self.textures.len()
    }

    pub(crate) fn set_viewport(&mut self, width: u32, height: u32) {
        self.viewport = [width, height];
    }

    pub(crate) fn upload_material(
        &mut self,
        handle: Handle<MaterialAsset>,
        material: MaterialAsset,
    ) {
        self.materials.insert(handle.id, material);
    }

    pub(crate) fn upload_pbr_material(
        &mut self,
        device: &Device,
        handle: Handle<MaterialAsset>,
        material: PbrMaterial,
    ) {
        let uniform = MaterialUniform {
            base_color: material.base_color,
            metallic_roughness: [material.metallic, material.roughness, 0.0, 0.0],
            texture_flags: [
                material.base_color_texture.is_some() as u32 as f32,
                material.metallic_roughness_texture.is_some() as u32 as f32,
                material.normal_texture.is_some() as u32 as f32,
                0.0,
            ],
        };
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("material-uniform"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material-bind-group"),
            layout: &self.material_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        material
                            .base_color_texture
                            .and_then(|texture| self.textures.get(&texture.id))
                            .map(|texture| &texture.view)
                            .unwrap_or(&self.default_texture.view),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.default_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        material
                            .metallic_roughness_texture
                            .and_then(|texture| self.textures.get(&texture.id))
                            .map(|texture| &texture.view)
                            .unwrap_or(&self.default_texture.view),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        material
                            .normal_texture
                            .and_then(|texture| self.textures.get(&texture.id))
                            .map(|texture| &texture.view)
                            .unwrap_or(&self.default_texture.view),
                    ),
                },
            ],
        });
        self.pbr_materials.insert(
            handle.id,
            GpuMaterial {
                _buffer: buffer,
                bind_group,
            },
        );
    }

    pub(crate) fn remove_mesh(&mut self, handle: Handle<MeshAsset>) {
        self.meshes.remove(&handle.id);
    }

    pub(crate) fn remove_material(&mut self, handle: Handle<MaterialAsset>) {
        self.materials.remove(&handle.id);
        self.pbr_materials.remove(&handle.id);
    }

    pub(crate) fn remove_texture(&mut self, handle: Handle<TextureAsset>) {
        self.textures.remove(&handle.id);
    }

    pub(crate) fn stats(&self) -> RenderStats {
        RenderStats {
            path: self.path,
            ..self.stats
        }
    }
}

impl RenderFeature for ForwardFeature {
    fn extract(&mut self, snapshot: &RenderSnapshot) {
        self.instances.clone_from(&snapshot.instances);
        self.skins.clone_from(&snapshot.skins);
        self.snapshot_skin_fallbacks = snapshot.skin_fallbacks;
        self.camera = snapshot.camera;
        self.light = snapshot.lights.first().copied();
    }

    fn prepare(&mut self, device: &Device, queue: &Queue) -> EngineResult<()> {
        self.instances.sort_by_key(|instance| {
            (
                instance.mesh.id.index,
                instance.mesh.id.generation,
                instance.material.id.index,
                instance.material.id.generation,
            )
        });
        let camera = self.camera.unwrap_or(RenderCamera {
            transform: TransformState {
                translation: Vec3::new(0.0, 2.0, 8.0),
                ..TransformState::identity()
            },
            fov_y_radians: 60.0_f32.to_radians(),
            near: 0.1,
            far: 1000.0,
        });
        let view_projection = Mat4::perspective_rh(
            camera.fov_y_radians.max(0.1),
            self.viewport[0] as f32 / self.viewport[1].max(1) as f32,
            camera.near.max(0.001),
            camera.far.max(camera.near + 1.0),
        ) * transform_matrix(camera.transform).inverse();
        let light = self.light.unwrap_or(RenderLight {
            direction: [-0.4, -1.0, -0.5],
            color: [1.0; 3],
            intensity: 1.0,
        });
        let globals = Globals {
            view_projection: view_projection.to_cols_array_2d(),
            light_direction: [
                light.direction[0],
                light.direction[1],
                light.direction[2],
                0.0,
            ],
            light_color: [
                light.color[0],
                light.color[1],
                light.color[2],
                light.intensity,
            ],
            ambient: [0.2, 0.22, 0.25, 0.0],
        };
        queue.write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
        let mut matrices = Vec::new();
        let mut skin_ranges = Vec::with_capacity(self.skins.len());
        for skin in &self.skins {
            if skin.matrices.is_empty() || skin.matrices.iter().any(|matrix| !matrix.is_finite()) {
                skin_ranges.push(None);
            } else {
                skin_ranges.push(Some((matrices.len() as u32, skin.matrices.len() as u32)));
                matrices.extend(skin.matrices.iter().map(Mat4::to_cols_array_2d));
            }
        }
        if matrices.is_empty() {
            matrices.push(Mat4::IDENTITY.to_cols_array_2d());
        }
        let matrix_bytes = bytemuck::cast_slice(&matrices);
        if matrix_bytes.len() as u64 > device.limits().max_storage_buffer_binding_size as u64 {
            return Err(EngineError::Unsupported(
                "skin palettes exceed storage buffer limit".into(),
            ));
        }
        if self.joint_buffer.size() < matrix_bytes.len() as u64 {
            self.joint_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("joint-matrices"),
                size: (matrix_bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("globals-bind-group"),
                layout: &self.globals_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.globals_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.joint_buffer.as_entire_binding(),
                    },
                ],
            });
        }
        queue.write_buffer(&self.joint_buffer, 0, matrix_bytes);
        for (_, _, values) in &mut self.scratch_groups {
            values.clear();
        }
        for bounds in &mut self.scratch_bounds {
            bounds.clear();
        }
        let mut skipped = 0;
        let mut culled = 0;
        let mut skin_fallbacks = self.snapshot_skin_fallbacks;
        let mut skinned = 0;
        let mut group_count = 0;
        for instance in &self.instances {
            if !self.meshes.contains_key(&instance.mesh.id) {
                skipped += 1;
                continue;
            }
            let range = instance
                .skin
                .and_then(|index| skin_ranges.get(index as usize).copied().flatten());
            if instance.skin.is_some() && range.is_none() {
                skin_fallbacks += 1;
            }
            let bounds = instance
                .skin
                .and_then(|index| self.skins.get(index as usize))
                .filter(|_| range.is_some())
                .map(|skin| skinned_bounds(instance.bounds, &skin.matrices))
                .unwrap_or(instance.bounds);
            if self.path != RenderPath::GpuDriven
                && !sphere_visible(view_projection, instance.transform, bounds)
            {
                culled += 1;
                continue;
            }
            if range.is_some() {
                skinned += 1;
            }
            let key = (instance.mesh.id, instance.material.id);
            if group_count == 0 || {
                let group = &self.scratch_groups[group_count - 1];
                (group.0, group.1) != key
            } {
                if let Some(group) = self.scratch_groups.get_mut(group_count) {
                    group.0 = key.0;
                    group.1 = key.1;
                } else {
                    self.scratch_groups.push((key.0, key.1, Vec::new()));
                }
                if self.scratch_bounds.len() == group_count {
                    self.scratch_bounds.push(Vec::new());
                }
                group_count += 1;
            }
            let (skin_base, skin_count) = range.unwrap_or((u32::MAX, 0));
            self.scratch_groups[group_count - 1].2.push(InstanceData {
                model: transform_matrix(instance.transform).to_cols_array_2d(),
                color: [1.0; 4],
                skin_base,
                skin_count,
                _padding: [0; 2],
            });
            self.scratch_bounds[group_count - 1].push(bounds);
        }
        self.scratch_groups.truncate(group_count);
        self.scratch_bounds.truncate(group_count);
        if self.path == RenderPath::GpuDriven {
            self.batches.clear();
            self.gpu_batches.truncate(group_count);
            for (index, (mesh, material, values)) in self.scratch_groups.iter().enumerate() {
                let count = values.len();
                let bytes = count * std::mem::size_of::<CullInstanceData>();
                if bytes as u64 > device.limits().max_storage_buffer_binding_size as u64
                    || count.div_ceil(64)
                        > device.limits().max_compute_workgroups_per_dimension as usize
                {
                    return Err(EngineError::Unsupported(
                        "instance batch exceeds GPU culling limits".into(),
                    ));
                }
                if self
                    .gpu_batches
                    .get(index)
                    .is_none_or(|batch| batch.capacity < count)
                {
                    let batch = GpuDrivenBatch::new(
                        device,
                        &self.globals_buffer,
                        self.cull_layout.as_ref().expect("compute supported"),
                        self.finalize_layout.as_ref().expect("compute supported"),
                        *mesh,
                        *material,
                        count,
                    );
                    if index == self.gpu_batches.len() {
                        self.gpu_batches.push(batch);
                    } else {
                        self.gpu_batches[index] = batch;
                    }
                }
                let batch = &mut self.gpu_batches[index];
                batch.mesh = *mesh;
                batch.material = *material;
                batch.count = count as u32;
                let input = values
                    .iter()
                    .zip(&self.scratch_bounds[index])
                    .map(|(value, bounds)| CullInstanceData {
                        model: value.model,
                        color: value.color,
                        bounds: *bounds,
                        skin_base: value.skin_base,
                        skin_count: value.skin_count,
                        _padding: [0; 2],
                    })
                    .collect::<Vec<_>>();
                queue.write_buffer(&batch.input, 0, bytemuck::cast_slice(&input));
                queue.write_buffer(&batch.counter, 0, bytemuck::bytes_of(&0_u32));
                queue.write_buffer(
                    &batch.params,
                    0,
                    bytemuck::cast_slice(&[count as u32, self.meshes[mesh].index_count, 0, 0]),
                );
            }
        } else {
            self.gpu_batches.clear();
            self.batches.truncate(group_count);
            for (index, (mesh, material, values)) in self.scratch_groups.iter().enumerate() {
                let count = values.len();
                if self
                    .batches
                    .get(index)
                    .is_none_or(|batch| batch.capacity < count)
                {
                    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("cpu-instance-buffer"),
                        size: (count.max(1) * std::mem::size_of::<InstanceData>()) as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    let indirect = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("cpu-indirect"),
                        size: 20,
                        usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    let batch = GpuBatch {
                        mesh: *mesh,
                        material: *material,
                        buffer,
                        indirect,
                        capacity: count,
                        count: count as u32,
                    };
                    if index == self.batches.len() {
                        self.batches.push(batch);
                    } else {
                        self.batches[index] = batch;
                    }
                }
                let batch = &mut self.batches[index];
                batch.mesh = *mesh;
                batch.material = *material;
                batch.count = count as u32;
                queue.write_buffer(&batch.buffer, 0, bytemuck::cast_slice(values));
                queue.write_buffer(
                    &batch.indirect,
                    0,
                    bytemuck::bytes_of(&IndirectDrawIndexedArgs {
                        index_count: self.meshes[mesh].index_count,
                        instance_count: count as u32,
                        ..IndirectDrawIndexedArgs::default()
                    }),
                );
            }
        }
        self.stats = RenderStats {
            instances: self.instances.len(),
            batches: group_count,
            draw_calls: group_count,
            instance_upload_bytes: self
                .scratch_groups
                .iter()
                .map(|(_, _, v)| {
                    v.len() as u64
                        * if self.path == RenderPath::GpuDriven {
                            112
                        } else {
                            96
                        }
                })
                .sum(),
            skipped_instances: skipped,
            fallback: self.preference == RenderPreference::Auto
                && self.path != RenderPath::GpuDriven,
            culled_instances: culled,
            indirect_draws: group_count,
            path: self.path,
            gpu_cull_dispatches: if self.path == RenderPath::GpuDriven {
                group_count * 2
            } else {
                0
            },
            gpu_skinned_instances: skinned,
            skin_fallbacks,
            gpu_visibility_pending: self.path == RenderPath::GpuDriven,
        };
        Ok(())
    }

    fn compute<'a>(&'a self, node: &str, pass: &mut wgpu::ComputePass<'a>) {
        if node != "gpu_cull" || self.path != RenderPath::GpuDriven {
            return;
        }
        pass.set_pipeline(self.cull_pipeline.as_ref().expect("compute supported"));
        for batch in &self.gpu_batches {
            pass.set_bind_group(0, &batch.cull_bind_group, &[]);
            pass.dispatch_workgroups((batch.count as usize).div_ceil(64) as u32, 1, 1);
            pass.set_pipeline(self.finalize_pipeline.as_ref().expect("compute supported"));
            pass.set_bind_group(0, &batch.finalize_bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
            pass.set_pipeline(self.cull_pipeline.as_ref().expect("compute supported"));
        }
    }

    fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.globals_bind_group, &[]);
        if self.path == RenderPath::GpuDriven {
            for batch in &self.gpu_batches {
                let Some(mesh) = self.meshes.get(&batch.mesh) else {
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.vertex.slice(..));
                pass.set_vertex_buffer(1, batch.output.slice(..));
                pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
                let material = self
                    .pbr_materials
                    .get(&batch.material)
                    .unwrap_or(&self.default_material);
                pass.set_bind_group(1, &material.bind_group, &[]);
                pass.draw_indexed_indirect(&batch.indirect, 0);
            }
        } else {
            for batch in &self.batches {
                let Some(mesh) = self.meshes.get(&batch.mesh) else {
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.vertex.slice(..));
                pass.set_vertex_buffer(1, batch.buffer.slice(..));
                pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
                let material = self
                    .pbr_materials
                    .get(&batch.material)
                    .unwrap_or(&self.default_material);
                pass.set_bind_group(1, &material.bind_group, &[]);
                pass.draw_indexed_indirect(&batch.indirect, 0);
            }
        }
    }
}

fn transform_matrix(transform: TransformState) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        transform.scale,
        transform.rotation,
        transform.translation,
    )
}

fn buffer_layout_entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

impl GpuDrivenBatch {
    fn new(
        device: &Device,
        globals: &wgpu::Buffer,
        cull_layout: &wgpu::BindGroupLayout,
        finalize_layout: &wgpu::BindGroupLayout,
        mesh: AssetId<MeshAsset>,
        material: AssetId<MaterialAsset>,
        count: usize,
    ) -> Self {
        let capacity = count.max(1);
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let input = buffer(
            "cull-input",
            capacity as u64 * 112,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let output = buffer(
            "cull-output",
            capacity as u64 * 96,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_SRC,
        );
        let counter = buffer(
            "cull-counter",
            4,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let indirect = buffer(
            "cull-indirect",
            20,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_SRC,
        );
        let params = buffer(
            "cull-params",
            16,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        fn entry(binding: u32, b: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry {
                binding,
                resource: b.as_entire_binding(),
            }
        }
        let cull_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cull-bind-group"),
            layout: cull_layout,
            entries: &[
                entry(0, globals),
                entry(1, &input),
                entry(2, &output),
                entry(3, &counter),
                entry(4, &params),
            ],
        });
        let finalize_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("finalize-bind-group"),
            layout: finalize_layout,
            entries: &[entry(0, &counter), entry(1, &indirect), entry(2, &params)],
        });
        Self {
            mesh,
            material,
            input,
            output,
            counter,
            indirect,
            params,
            count: count as u32,
            cull_bind_group,
            finalize_bind_group,
            capacity,
        }
    }
}

/// A linear blend of positive normalized weights lies inside the union's AABB.
fn skinned_bounds(bounds: [f32; 4], matrices: &[Mat4]) -> [f32; 4] {
    let center = Vec3::new(bounds[0], bounds[1], bounds[2]);
    let mut min = center - Vec3::splat(bounds[3]);
    let mut max = center + Vec3::splat(bounds[3]);
    for matrix in matrices {
        let c = matrix.transform_point3(center);
        // Frobenius norm bounds the spectral norm even when a palette contains shear.
        let radius = bounds[3]
            * (matrix.x_axis.truncate().length_squared()
                + matrix.y_axis.truncate().length_squared()
                + matrix.z_axis.truncate().length_squared())
            .sqrt();
        min = min.min(c - Vec3::splat(radius));
        max = max.max(c + Vec3::splat(radius));
    }
    let c = (min + max) * 0.5;
    [c.x, c.y, c.z, (max - c).length()]
}

#[cfg(test)]
#[path = "forward_tests.rs"]
mod gpu_tests;
