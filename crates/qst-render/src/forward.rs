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

struct GpuMesh {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    index_count: u32,
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
    meshes: HashMap<AssetId<MeshAsset>, GpuMesh>,
    materials: HashMap<AssetId<MaterialAsset>, MaterialAsset>,
    batches: Vec<GpuBatch>,
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("forward-layout"),
            bind_group_layouts: &[&globals_layout],
            push_constant_ranges: &[],
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
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
                            2 => Float32x4,
                            3 => Float32x4,
                            4 => Float32x4,
                            5 => Float32x4,
                            6 => Float32x4,
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
            meshes: HashMap::new(),
            materials: HashMap::new(),
            batches: Vec::new(),
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
        let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-vertices"),
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

    pub(crate) fn gpu_resource_count(&self) -> usize {
        self.meshes.len() + self.materials.len() + self.batches.len()
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

    pub(crate) fn remove_mesh(&mut self, handle: Handle<MeshAsset>) {
        self.meshes.remove(&handle.id);
    }

    pub(crate) fn remove_material(&mut self, handle: Handle<MaterialAsset>) {
        self.materials.remove(&handle.id);
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
        let mut groups: Vec<(
            AssetId<MeshAsset>,
            AssetId<MaterialAsset>,
            Vec<InstanceData>,
        )> = Vec::new();
        for instance in &self.instances {
            let key = (instance.mesh.id, instance.material.id);
            if let Some((mesh, material, values)) = groups.last_mut()
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
                groups.push((
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
        self.batches.truncate(groups.len());
        for (index, (mesh, material, values)) in groups.iter().enumerate() {
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
            batches: groups.len(),
            draw_calls: groups.len(),
            instance_upload_bytes: groups
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
