use super::*;

#[test]
fn snapshot_groups_instances_by_mesh_and_material() {
    for count in [1024_usize, 10_000_usize] {
        let mut snapshot = RenderSnapshot::default();
        for entity in 0..count {
            snapshot.instances.push(RenderInstance {
                entity: entity as u64,
                transform: TransformState::identity(),
                bounds: [0.0, 0.0, 0.0, 1.0],
                mesh: Handle::new(AssetId::new((entity % 4) as u32, 0)),
                material: Handle::new(AssetId::new((entity % 4) as u32, 0)),
            });
        }
        assert_eq!(snapshot.instances.len(), count);
        assert_eq!(snapshot.batch_count(), 4);
    }
}

#[test]
fn sphere_culling_keeps_camera_facing_instances_and_rejects_far_instances() {
    let camera = Mat4::perspective_rh(60.0_f32.to_radians(), 1.0, 0.1, 10.0);
    let visible = TransformState {
        translation: Vec3::new(0.0, 0.0, -2.0),
        ..TransformState::identity()
    };
    let outside = TransformState {
        translation: Vec3::new(100.0, 0.0, -2.0),
        ..TransformState::identity()
    };
    assert!(sphere_visible(camera, visible, [0.0, 0.0, 0.0, 0.5]));
    assert!(!sphere_visible(camera, outside, [0.0, 0.0, 0.0, 0.5]));
}

#[test]
fn cpu_skinning_blends_joint_matrices() {
    let vertices = [SkinnedVertex {
        position: [0.0, 0.0, 0.0],
        joints: [0, 1, 0, 0],
        weights: [0.5, 0.5, 0.0, 0.0],
    }];
    let result = skin_vertices(
        &vertices,
        &[
            Mat4::from_translation(Vec3::X),
            Mat4::from_translation(Vec3::Y),
        ],
    );
    assert_eq!(result[0], [0.5, 0.5, 0.0]);
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
                bounds: [0.0, 0.0, 0.0, 1.0],
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
