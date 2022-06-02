use wgpu::util::DeviceExt;

fn main() {
    let filename = std::env::args().nth(1).unwrap();
    let bytes = std::fs::read(&filename).unwrap();

    let ktx2 = ktx2::Reader::new(&bytes).unwrap();

    let header = ktx2.header();

    println!("{:#?}", header);

    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);

    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::PUSH_CONSTANTS,
            limits: wgpu::Limits {
                max_push_constant_size: 8,
                ..Default::default()
            },
        },
        None,
    ))
    .unwrap();

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Uint,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::COMPUTE,
            range: 0..std::mem::size_of::<[u32; 2]>() as u32,
        }],
    });

    let shader =
        device.create_shader_module(&wgpu::include_spirv!("../granite-shaders/bc6.comp.spv"));

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: "main",
    });

    let uastc_transfer_function = ktx2
        .data_format_descriptors()
        .filter_map(|dfd| {
            let basic_dfd = ktx2::BasicDataFormatDescriptor::parse(&dfd.data);
            basic_dfd.ok()
        })
        .filter(|basic_dfd| basic_dfd.color_model == Some(ktx2::ColorModel::UASTC))
        .filter_map(|basic_dfd| basic_dfd.transfer_function)
        .next();

    let mut dds = ddsfile::Dds::new_dxgi(ddsfile::NewDxgiParams {
        width: header.pixel_width,
        height: header.pixel_height,
        depth: Some(header.pixel_depth).filter(|&depth| depth != 0),
        format: match (header.format, uastc_transfer_function) {
            (Some(ktx2::Format::R32G32B32A32_SFLOAT), _) => ddsfile::DxgiFormat::R32G32B32A32_Float,
            (Some(ktx2::Format::R16G16B16A16_SFLOAT), _) => ddsfile::DxgiFormat::R16G16B16A16_Float,
            (Some(ktx2::Format::BC6H_UFLOAT_BLOCK), _) => ddsfile::DxgiFormat::R16G16B16A16_Float,
            (None, Some(ktx2::TransferFunction::SRGB)) => ddsfile::DxgiFormat::BC7_UNorm_sRGB,
            (None, Some(_)) => ddsfile::DxgiFormat::BC7_UNorm,
            other => unimplemented!("unsupported format: {:?}", other),
        },
        mipmap_levels: Some(header.level_count).filter(|&count| count != 0),
        array_layers: Some(header.layer_count).filter(|&count| count != 0),
        is_cubemap: header.face_count == 6,
        caps2: Some(ddsfile::Caps2::CUBEMAP),
        resource_dimension: if header.pixel_depth != 0 {
            ddsfile::D3D10ResourceDimension::Texture3D
        } else {
            ddsfile::D3D10ResourceDimension::Texture2D
        },
        alpha_mode: ddsfile::AlphaMode::Opaque,
    })
    .unwrap();

    let face_count = header.face_count as usize;

    let mut faces = vec![Vec::new(); face_count];

    basis_universal::transcoding::transcoder_init();
    let transcoder = basis_universal::LowLevelUastcTranscoder::new();

    for (level_index, level) in ktx2.levels().enumerate() {
        let level_bytes = match header.supercompression_scheme {
            Some(ktx2::SupercompressionScheme::Zstandard) => std::borrow::Cow::Owned(
                zstd::bulk::decompress(level.bytes, level.uncompressed_byte_length as usize)
                    .unwrap(),
            ),
            Some(other) => todo!("{:?}", other),
            None => std::borrow::Cow::Borrowed(level.bytes),
        };

        let level_bytes = if uastc_transfer_function.is_some() {
            let slice_width = header.pixel_width >> level_index;
            let slice_height = header.pixel_height >> level_index;
            let (block_width_pixels, block_height_pixels) = (4, 4);

            std::borrow::Cow::Owned(
                transcoder
                    .transcode_slice(
                        &level_bytes,
                        basis_universal::SliceParametersUastc {
                            num_blocks_x: ((slice_width + block_width_pixels - 1)
                                / block_width_pixels)
                                .max(1),
                            num_blocks_y: ((slice_height + block_height_pixels - 1)
                                / block_height_pixels)
                                .max(1),
                            has_alpha: false,
                            original_width: slice_width,
                            original_height: slice_height,
                        },
                        basis_universal::DecodeFlags::HIGH_QUALITY,
                        basis_universal::transcoding::TranscoderBlockFormat::BC7,
                    )
                    .unwrap(),
            )
        } else {
            level_bytes
        };

        let mut olevel_bytes = Vec::new();

        for i in 0..face_count {
            let size = level_bytes.len() / face_count;

            let bytes = &level_bytes[i * size..(i + 1) * size];

            let layer_width = header.pixel_width >> (level_index as u32);
            let layer_height = header.pixel_height >> (level_index as u32);

            let pixel_size = 8;

            // Store each block as a uvec4.
            let texture = device.create_texture_with_data(
                &queue,
                &wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: layer_width >> 2,
                        height: layer_height >> 2,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba32Uint,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                },
                bytes,
            );

            let row_size = (layer_width * pixel_size).max(256);

            let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: row_size as u64 * layer_height as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            let output_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: layer_width,
                    height: layer_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Uint,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &output_texture.create_view(&Default::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &texture.create_view(&Default::default()),
                        ),
                    },
                ],
            });

            let mut command_encoder = device.create_command_encoder(&Default::default());

            let mut compute_pass = command_encoder.begin_compute_pass(&Default::default());

            compute_pass.set_pipeline(&pipeline);

            compute_pass.set_bind_group(0, &bind_group, &[]);

            let mut push_constants = [0; 8];

            push_constants[0..4].copy_from_slice(&(layer_width as i32).to_le_bytes());
            push_constants[4..8].copy_from_slice(&(layer_height as i32).to_le_bytes());

            compute_pass.set_push_constants(0, &push_constants);

            compute_pass.dispatch_workgroups(layer_width >> 2, layer_height >> 2, 1);

            drop(compute_pass);

            command_encoder.copy_texture_to_buffer(
                wgpu::ImageCopyTexture {
                    texture: &output_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::ImageCopyBuffer {
                    buffer: &output_buffer,
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(std::num::NonZeroU32::new(row_size).unwrap()),
                        rows_per_image: None,
                    },
                },
                wgpu::Extent3d {
                    width: layer_width,
                    height: layer_height,
                    depth_or_array_layers: 1,
                },
            );

            queue.submit(std::iter::once(command_encoder.finish()));

            let slice = output_buffer.slice(..);

            let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
            slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

            device.poll(wgpu::Maintain::Wait);

            pollster::block_on(receiver.receive()).unwrap().unwrap();

            let bytes = slice.get_mapped_range();

            if layer_width * pixel_size >= 256 {
                olevel_bytes.extend_from_slice(&bytes);
            } else {
                for chunk in bytes.chunks_exact(256) {
                    olevel_bytes
                        .extend_from_slice(&chunk[..(layer_width as usize * pixel_size as usize)]);
                }
            }
            /*olevel_bytes.extend_from_slice(&bcndecode::decode(
                &level_bytes[i * size..(i + 1) * size],
                header.pixel_width as usize >> level_index,
                header.pixel_height as usize >> level_index,
                bcndecode::BcnEncoding::Bc6H,
                bcndecode::BcnDecoderFormat::RGBA
            ).unwrap())*/

            //olevel_bytes.extend_from_slice(&utgh::decompress_bytes(&level_bytes[i * size..(i + 1) * size], header.pixel_width as usize >> level_index));
        }

        println!(
            "{} {} {}",
            level_bytes.len(),
            olevel_bytes.len(),
            olevel_bytes.len() / level_bytes.len()
        );

        let level_bytes = olevel_bytes;

        let size = level_bytes.len() / face_count;
        for (i, face) in faces.iter_mut().enumerate() {
            let slice = &level_bytes[i * size..(i + 1) * size];
            face.extend_from_slice(slice);
        }
    }

    dds.data = faces.concat();

    let output = std::env::args().nth(2).unwrap();

    let mut output_file = std::fs::File::create(output).unwrap();

    dds.write(&mut output_file).unwrap();
}
