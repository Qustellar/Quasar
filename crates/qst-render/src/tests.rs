use super::*;

#[test]
fn snapshot_groups_instances_by_mesh_and_material() {
    let mut snapshot = RenderSnapshot::default();
    for entity in 0..1024 {
        snapshot.instances.push(RenderInstance {
            entity,
            transform: TransformState::identity(),
            mesh: Handle::new(AssetId::new((entity % 4) as u32, 0)),
            material: Handle::new(AssetId::new((entity % 4) as u32, 0)),
        });
    }
    assert_eq!(snapshot.instances.len(), 1024);
    assert_eq!(snapshot.batch_count(), 4);
}

#[test]
#[ignore = "requires a GPU adapter"]
fn forward_feature_draws_nonblank_pixels() {
    pollster::block_on(async {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();
        let width = 128;
        let height = 128;
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test-color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth = depth_view(&device, width, height);
        let mesh_handle = Handle::new(AssetId::new(1, 0));
        let material_handle = Handle::new(AssetId::new(2, 0));
        let mut forward =
            ForwardFeature::new(&device, wgpu::TextureFormat::Rgba8Unorm, [width, height]);
        forward.upload_mesh(
            &device,
            mesh_handle,
            &MeshAsset {
                vertices: vec![
                    MeshVertex {
                        position: [-0.7, -0.7, 0.0],
                        normal: [0.0, 0.0, 1.0],
                    },
                    MeshVertex {
                        position: [0.7, -0.7, 0.0],
                        normal: [0.0, 0.0, 1.0],
                    },
                    MeshVertex {
                        position: [0.0, 0.7, 0.0],
                        normal: [0.0, 0.0, 1.0],
                    },
                ],
                indices: vec![0, 1, 2],
            },
        );
        forward.upload_material(
            material_handle,
            MaterialAsset::from_color([1.0, 0.1, 0.1, 1.0]),
        );
        let snapshot = RenderSnapshot {
            instances: vec![RenderInstance {
                entity: 1,
                transform: TransformState::identity(),
                mesh: mesh_handle,
                material: material_handle,
            }],
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
        };
        forward.extract(&snapshot);
        forward.prepare(&device, &queue).unwrap();
        forward.queue(&snapshot);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test-readback"),
            size: (width * height * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("test-frame"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("test-forward"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08,
                            g: 0.1,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            forward.render(&mut pass);
        }
        encoder.copy_texture_to_buffer(
            color.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(encoder.finish()));
        let (tx, rx) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let data = readback.slice(..).get_mapped_range();
        let center = ((height / 2 * width + width / 2) * 4) as usize;
        assert!(
            data[center] > data[center + 1] * 2,
            "center pixel should be red"
        );
    });
}
