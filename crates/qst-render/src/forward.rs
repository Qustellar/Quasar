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
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaterialUniform {
    base_color: [f32; 4],
    metallic_roughness: [f32; 4],
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
    capacity: usize,
    count: u32,
}

pub struct ForwardFeature {
    pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    material_layout: wgpu::BindGroupLayout,
    default_texture: GpuTexture,
    default_sampler: wgpu::Sampler,
    default_material: GpuMaterial,
    meshes: HashMap<AssetId<MeshAsset>, GpuMesh>,
    textures: HashMap<AssetId<TextureAsset>, GpuTexture>,
    materials: HashMap<AssetId<MaterialAsset>, MaterialAsset>,
    pbr_materials: HashMap<AssetId<MaterialAsset>, GpuMaterial>,
    batches: Vec<GpuBatch>,
    scratch_groups: Vec<(
        AssetId<MeshAsset>,
        AssetId<MaterialAsset>,
        Vec<InstanceData>,
    )>,
    instances: Vec<RenderInstance>,
    camera: Option<RenderCamera>,
    light: Option<RenderLight>,
    viewport: [u32; 2],
    stats: RenderStats,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStats {
    pub instances: usize,
    pub batches: usize,
    pub draw_calls: usize,
    pub instance_upload_bytes: u64,
    pub skipped_instances: usize,
    pub fallback: bool,
}

impl ForwardFeature {
    pub(crate) fn new(device: &Device, format: wgpu::TextureFormat, viewport: [u32; 2]) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quasar-forward-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("forward.wgsl").into()),
        });
        let globals_layout = uniform_layout(device, "globals-layout");
        let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globals"),
            contents: bytemuck::bytes_of(&Globals::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let globals_bind_group = uniform_bind_group(
            device,
            &globals_layout,
            &globals_buffer,
            "globals-bind-group",
        );
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
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("forward-layout"),
            bind_group_layouts: &[&globals_layout, &material_layout],
            push_constant_ranges: &[],
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32x2];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("forward-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<MeshVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attributes,
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<InstanceData>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            4 => Float32x4,
                            5 => Float32x4,
                            6 => Float32x4,
                            7 => Float32x4,
                            8 => Float32x4,
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
            globals_bind_group,
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
            scratch_groups: Vec::new(),
            instances: Vec::new(),
            camera: None,
            light: None,
            viewport,
            stats: RenderStats::default(),
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
        self.meshes.len() + self.materials.len() + self.batches.len() + self.textures.len()
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
            metallic_roughness: [
                material.metallic,
                material.roughness,
                0.0,
                material.base_color_texture.is_some() as u32 as f32,
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
        self.stats
    }
}

impl RenderFeature for ForwardFeature {
    fn extract(&mut self, snapshot: &RenderSnapshot) {
        self.instances.clone_from(&snapshot.instances);
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
        let projection = Mat4::perspective_rh(
            camera.fov_y_radians.max(0.1),
            self.viewport[0] as f32 / self.viewport[1].max(1) as f32,
            camera.near.max(0.001),
            camera.far.max(camera.near + 1.0),
        );
        let light = self.light.unwrap_or(RenderLight {
            direction: [-0.4, -1.0, -0.5],
            color: [1.0; 3],
            intensity: 1.0,
        });
        let globals = Globals {
            view_projection: (projection * transform_matrix(camera.transform).inverse())
                .to_cols_array_2d(),
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
        self.scratch_groups.clear();
        for instance in &self.instances {
            let key = (instance.mesh.id, instance.material.id);
            if let Some((mesh, material, values)) = self.scratch_groups.last_mut()
                && (*mesh, *material) == key
            {
                values.push(InstanceData {
                    model: transform_matrix(instance.transform).to_cols_array_2d(),
                    color: self
                        .materials
                        .get(&instance.material.id)
                        .map(|m| m.color)
                        .unwrap_or([0.8; 4]),
                });
            } else {
                self.scratch_groups.push((
                    key.0,
                    key.1,
                    vec![InstanceData {
                        model: transform_matrix(instance.transform).to_cols_array_2d(),
                        color: self
                            .materials
                            .get(&instance.material.id)
                            .map(|m| m.color)
                            .unwrap_or([0.8; 4]),
                    }],
                ));
            }
        }
        self.batches.truncate(self.scratch_groups.len());
        for (index, (mesh, material, values)) in self.scratch_groups.iter().enumerate() {
            let required = values.len();
            let batch = if let Some(batch) = self.batches.get_mut(index) {
                batch
            } else {
                self.batches.push(GpuBatch {
                    mesh: *mesh,
                    material: *material,
                    buffer: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("instance-buffer"),
                        size: (std::mem::size_of::<InstanceData>() * required.max(1)) as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }),
                    capacity: required.max(1),
                    count: 0,
                });
                self.batches.last_mut().expect("batch inserted")
            };
            batch.mesh = *mesh;
            batch.material = *material;
            if batch.capacity < required {
                batch.capacity = required.next_power_of_two();
                batch.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("instance-buffer"),
                    size: (std::mem::size_of::<InstanceData>() * batch.capacity) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            queue.write_buffer(&batch.buffer, 0, bytemuck::cast_slice(values));
            batch.count = required as u32;
        }
        self.stats = RenderStats {
            instances: self.instances.len(),
            batches: self.scratch_groups.len(),
            draw_calls: self.scratch_groups.len(),
            instance_upload_bytes: self
                .scratch_groups
                .iter()
                .map(|(_, _, values)| (values.len() * std::mem::size_of::<InstanceData>()) as u64)
                .sum(),
            skipped_instances: self
                .instances
                .iter()
                .filter(|instance| !self.meshes.contains_key(&instance.mesh.id))
                .count(),
            fallback: false,
        };
        Ok(())
    }

    fn queue(&mut self, _snapshot: &RenderSnapshot) {
        self.instances.sort_by_key(|instance| {
            (
                instance.mesh.id.index,
                instance.mesh.id.generation,
                instance.material.id.index,
                instance.material.id.generation,
            )
        });
    }

    fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.globals_bind_group, &[]);
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
            pass.draw_indexed(0..mesh.index_count, 0, 0..batch.count);
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

fn uniform_layout(device: &Device, label: &str) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
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
    })
}

fn uniform_bind_group(
    device: &Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}
