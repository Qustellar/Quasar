use super::*;

async fn headless() -> Option<(Device, Queue, RenderCapabilities)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    let Ok(adapter) = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
    else {
        assert_eq!(
            RenderPreference::Auto
                .select(RenderCapabilities::default())
                .unwrap(),
            RenderPath::Indirect
        );
        eprintln!("SKIP: no headless adapter; adapter-free selection verified");
        return None;
    };
    eprintln!("adapter: {:?}", adapter.get_info());
    let caps = RenderCapabilities::from_adapter(&adapter);
    assert!(
        caps.supports_gpu_driven(),
        "local GPU validation requires compute and vertex storage"
    );
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .unwrap();
    Some((device, queue, caps))
}

fn feature(
    device: &Device,
    caps: RenderCapabilities,
    preference: RenderPreference,
) -> ForwardFeature {
    let mut forward = ForwardFeature::new(
        device,
        wgpu::TextureFormat::Rgba8Unorm,
        [128, 128],
        preference,
        caps,
    );
    let vertices =
        [[-2.4, -0.4, 0.0], [-1.6, -0.4, 0.0], [-2.0, 0.4, 0.0]].map(|position| MeshVertexPbr {
            position,
            normal: [0.0, 0.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            uv: [0.0; 2],
            joints: [0; 4],
            weights: [1.0, 0.0, 0.0, 0.0],
        });
    forward.upload_textured_mesh(
        device,
        Handle::new(AssetId::new(1, 0)),
        &TexturedMeshAsset {
            vertices: vertices.to_vec(),
            indices: vec![0, 1, 2],
        },
    );
    forward.upload_pbr_material(
        device,
        Handle::new(AssetId::new(2, 0)),
        PbrMaterial {
            base_color: [1.0, 0.01, 0.01, 1.0],
            roughness: 1.0,
            ..PbrMaterial::default()
        },
    );
    forward
}

fn snapshot(count: usize, skin: bool) -> RenderSnapshot {
    RenderSnapshot {
        skin_fallbacks: 0,
        instances: (0..count)
            .map(|index| RenderInstance {
                entity: index as u64,
                transform: TransformState {
                    translation: if index % 2 == 0 {
                        Vec3::ZERO
                    } else {
                        Vec3::new(100.0, 0.0, 0.0)
                    },
                    ..TransformState::identity()
                },
                bounds: [-2.0, 0.0, 0.0, 0.6],
                mesh: Handle::new(AssetId::new(1, 0)),
                material: Handle::new(AssetId::new(2, 0)),
                skin: skin.then_some(0),
            })
            .collect(),
        skins: if skin {
            vec![RenderSkin {
                matrices: Arc::from([
                    Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)),
                    Mat4::IDENTITY,
                ]),
            }]
        } else {
            Vec::new()
        },
        camera: Some(RenderCamera {
            transform: TransformState {
                translation: Vec3::new(0.0, 0.0, 3.0),
                ..TransformState::identity()
            },
            fov_y_radians: 60.0_f32.to_radians(),
            near: 0.1,
            far: 10.0,
        }),
        lights: vec![RenderLight {
            direction: [0.0, 0.0, -1.0],
            color: [1.0; 3],
            intensity: 1.0,
        }],
    }
}

fn readback(
    device: &Device,
    encoder: &mut wgpu::CommandEncoder,
    source: &wgpu::Buffer,
    size: u64,
) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    encoder.copy_buffer_to_buffer(source, 0, &buffer, 0, size);
    buffer
}

fn mapped(device: &Device, buffer: &wgpu::Buffer) -> Vec<u8> {
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let bytes = buffer.slice(..).get_mapped_range().to_vec();
    buffer.unmap();
    bytes
}

#[test]
#[ignore = "requires a GPU adapter; validates counters, compaction and batch reuse"]
fn gpu_culling_indirect_readback() {
    pollster::block_on(async {
        let Some((device, queue, caps)) = headless().await else {
            return;
        };
        let mut forward = feature(&device, caps, RenderPreference::GpuDriven);
        for count in [2, 1, 7, 3, 2] {
            let source = snapshot(count, true);
            forward.extract(&source);
            forward.prepare(&device, &queue).unwrap();
            assert_eq!(forward.stats.batches, 1);
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
                forward.compute("gpu_cull", &mut pass);
            }
            let args = readback(&device, &mut encoder, &forward.gpu_batches[0].indirect, 20);
            let output = readback(&device, &mut encoder, &forward.gpu_batches[0].output, 96);
            queue.submit([encoder.finish()]);
            let data = mapped(&device, &args);
            let args = bytemuck::pod_read_unaligned::<IndirectDrawIndexedArgs>(&data);
            assert_eq!(args.index_count, 3);
            assert_eq!(args.instance_count, count.div_ceil(2) as u32);
            assert_eq!(
                args.first_instance, 0,
                "does not require INDIRECT_FIRST_INSTANCE"
            );
            let output = mapped(&device, &output);
            let value = bytemuck::pod_read_unaligned::<InstanceData>(&output);
            assert_eq!(value.skin_base, 0);
            assert_eq!(value.skin_count, 2);
            assert_eq!(value.model[3][0], 0.0);
            assert_eq!(forward.stats.gpu_cull_dispatches, 2);
        }
        // A reused slot must use the new mesh's index count, and resources must be batch-local.
        let mut mesh = MeshAsset {
            vertices: vec![
                MeshVertex {
                    position: [0.0; 3],
                    normal: [0.0, 0.0, 1.0]
                };
                3
            ],
            indices: vec![0, 1, 2, 0, 1, 2],
        };
        forward.upload_mesh(&device, Handle::new(AssetId::new(4, 0)), &mesh);
        let mut source = snapshot(2, true);
        source.instances[1].mesh = Handle::new(AssetId::new(4, 0));
        source.instances[1].transform = TransformState::identity();
        forward.extract(&source);
        forward.prepare(&device, &queue).unwrap();
        assert_eq!(forward.stats.batches, 2);
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            forward.compute("gpu_cull", &mut pass);
        }
        let buffers = forward
            .gpu_batches
            .iter()
            .map(|batch| readback(&device, &mut encoder, &batch.indirect, 20))
            .collect::<Vec<_>>();
        queue.submit([encoder.finish()]);
        for (i, buffer) in buffers.iter().enumerate() {
            let data = mapped(&device, buffer);
            let args = bytemuck::pod_read_unaligned::<IndirectDrawIndexedArgs>(&data);
            assert_eq!(args.instance_count, 1);
            assert_eq!(args.index_count, if i == 0 { 3 } else { 6 });
        }
        mesh.indices.truncate(3);
        forward.upload_mesh(&device, Handle::new(AssetId::new(4, 0)), &mesh);
    });
}

fn render_pixels(device: &Device, queue: &Queue, forward: &ForwardFeature) -> Vec<u8> {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("skin-output"),
        size: wgpu::Extent3d {
            width: 128,
            height: 128,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let depth = crate::renderer::depth_view(device, 128, 128);
    let mut encoder = device.create_command_encoder(&Default::default());
    let graph = RenderGraph::for_path(forward.path).compile().unwrap();
    let trace = Arc::new(std::sync::Mutex::new(Vec::new()));
    let features: Vec<Box<dyn RenderFeature>> = vec![Box::new(TraceFeature(trace.clone()))];
    crate::renderer::encode_graph(
        &mut encoder,
        &graph,
        forward,
        &features,
        crate::renderer::RenderTargets {
            color: &view,
            depth: &depth,
            clear: wgpu::Color::BLACK,
        },
    );
    let expected = if forward.path == RenderPath::GpuDriven {
        vec!["gpu_cull", "forward"]
    } else {
        vec!["forward"]
    };
    assert_eq!(*trace.lock().unwrap(), expected);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pixels"),
        size: 128 * 128 * 4,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(512),
                rows_per_image: Some(128),
            },
        },
        wgpu::Extent3d {
            width: 128,
            height: 128,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    mapped(device, &buffer)
}

#[test]
#[ignore = "requires a GPU adapter; verifies deformation and CPU-cull/GPU-skin fallback"]
fn vertex_skinning_render_readback_and_fallback() {
    pollster::block_on(async {
        let Some((device, queue, caps)) = headless().await else {
            return;
        };
        for (preference, caps) in [
            (RenderPreference::GpuDriven, caps),
            (RenderPreference::CpuIndirect, caps),
            (
                RenderPreference::Auto,
                RenderCapabilities {
                    compute: false,
                    ..caps
                },
            ),
        ] {
            let mut forward = feature(&device, caps, preference);
            for skin in [false, true, false] {
                forward.extract(&snapshot(2, skin));
                forward.prepare(&device, &queue).unwrap();
                let pixels = render_pixels(&device, &queue, &forward);
                let center = (64 * 128 + 64) * 4;
                if skin {
                    assert!(
                        pixels[center] > 128 && pixels[center + 1] < 10,
                        "skinned triangle reaches center: {:?}",
                        &pixels[center..center + 4]
                    );
                    assert!(forward.stats.gpu_skinned_instances > 0);
                } else {
                    assert_eq!(&pixels[center..center + 3], &[0, 0, 0]);
                }
            }
            if !caps.compute {
                assert!(forward.cull_pipeline.is_none());
                assert_eq!(forward.stats.gpu_cull_dispatches, 0);
                assert!(forward.stats.fallback);
            }
        }
    });
}

#[test]
#[ignore = "local CPU/GPU preparation + compute benchmark at 1024 and 10000 instances"]
fn cpu_gpu_instance_benchmark() {
    pollster::block_on(async {
        let Some((device, queue, caps)) = headless().await else {
            return;
        };
        for count in [1024, 10_000] {
            for preference in [RenderPreference::CpuIndirect, RenderPreference::GpuDriven] {
                let mut forward = feature(&device, caps, preference);
                let source = snapshot(count, true);
                let mut prepare_ms = Vec::new();
                let mut complete_ms = Vec::new();
                for frame in 0..23 {
                    let start = std::time::Instant::now();
                    forward.extract(&source);
                    forward.prepare(&device, &queue).unwrap();
                    let prepared = start.elapsed().as_secs_f64() * 1000.0;
                    let mut encoder = device.create_command_encoder(&Default::default());
                    if preference == RenderPreference::GpuDriven {
                        let mut pass = encoder.begin_compute_pass(&Default::default());
                        forward.compute("gpu_cull", &mut pass);
                    }
                    queue.submit([encoder.finish()]);
                    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                    if frame >= 3 {
                        prepare_ms.push(prepared);
                        complete_ms.push(start.elapsed().as_secs_f64() * 1000.0);
                    }
                }
                let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len() as f64;
                eprintln!(
                    "BENCH {count} {preference:?} prepare_ms={:.4} complete_ms={:.4} uploads={} batches={}",
                    mean(&prepare_ms),
                    mean(&complete_ms),
                    forward.stats.instance_upload_bytes,
                    forward.stats.batches
                );
                assert_eq!(forward.stats.batches, 1);
            }
        }
    });
}

struct TraceFeature(Arc<std::sync::Mutex<Vec<String>>>);
impl RenderFeature for TraceFeature {
    fn compute<'a>(&'a self, node: &str, _pass: &mut wgpu::ComputePass<'a>) {
        self.0.lock().unwrap().push(node.to_owned());
    }
    fn render<'a>(&'a self, _pass: &mut wgpu::RenderPass<'a>) {
        self.0.lock().unwrap().push("forward".into());
    }
}
