fn main() {
    let filename = std::env::args().nth(1).unwrap();
    let bytes = std::fs::read(filename).unwrap();

    let ktx2 = ktx2::Reader::new(&bytes).unwrap();

    let header = ktx2.header();

    println!("{:#?}", header);

    let uastc_transfer_function = Some(ktx2::TransferFunction::SRGB); /*ktx2
                                                                      .dfd_blocks()
                                                                      .filter_map(|dfd| {
                                                                          let basic_dfd = ktx2::DfdBlockHeaderBasic::parse(dfd.data);
                                                                          basic_dfd.ok()
                                                                      })
                                                                      .filter(|basic_dfd| basic_dfd.header.color_model == Some(ktx2::ColorModel::UASTC))
                                                                      .filter_map(|basic_dfd| basic_dfd.header.transfer_function)
                                                                      .next()*/

    let mut dds = ddsfile::Dds::new_dxgi(ddsfile::NewDxgiParams {
        width: header.pixel_width,
        height: header.pixel_height,
        depth: Some(header.pixel_depth).filter(|&depth| depth != 0),
        format: match (header.format, uastc_transfer_function) {
            (Some(ktx2::Format::R8G8B8A8_UNORM), _) => ddsfile::DxgiFormat::R8G8B8A8_UNorm,
            (Some(ktx2::Format::R8G8B8A8_SRGB), _) => ddsfile::DxgiFormat::R8G8B8A8_UNorm_sRGB,
            (Some(ktx2::Format::R32G32B32A32_SFLOAT), _) => ddsfile::DxgiFormat::R32G32B32A32_Float,
            (Some(ktx2::Format::R16G16B16A16_SFLOAT), _) => ddsfile::DxgiFormat::R16G16B16A16_Float,
            (Some(ktx2::Format::BC6H_UFLOAT_BLOCK), _) => ddsfile::DxgiFormat::BC6H_UF16,
            (Some(ktx2::Format::BC7_UNORM_BLOCK), _) => ddsfile::DxgiFormat::BC7_UNorm,
            (Some(ktx2::Format::BC7_SRGB_BLOCK), _) => ddsfile::DxgiFormat::BC7_UNorm_sRGB,
            (Some(ktx2::Format::E5B9G9R9_UFLOAT_PACK32), _) => {
                ddsfile::DxgiFormat::R9G9B9E5_SharedExp
            }
            (Some(ktx2::Format::R8_UNORM), _) => ddsfile::DxgiFormat::R8_UNorm,
            (Some(ktx2::Format::ASTC_4x4_SFLOAT_BLOCK), _) => {
                ddsfile::DxgiFormat::R32G32B32A32_Float
            }

            (None, Some(ktx2::TransferFunction::SRGB)) => ddsfile::DxgiFormat::BC7_UNorm_sRGB,
            (None, _) => ddsfile::DxgiFormat::BC7_UNorm,
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

    let transcoder = basis_universal::LowLevelUastcTranscoder::new();

    for (level_index, level) in ktx2.levels().enumerate() {
        let level_bytes = match header.supercompression_scheme {
            Some(ktx2::SupercompressionScheme::Zstandard) => std::borrow::Cow::Owned(
                zstd::bulk::decompress(level.data, level.uncompressed_byte_length as usize)
                    .unwrap(),
            ),
            Some(other) => todo!("{:?}", other),
            None => std::borrow::Cow::Borrowed(level.data),
        };

        let slice_width = header.pixel_width >> level_index;
        let slice_height = header.pixel_height >> level_index;

        let level_bytes = if uastc_transfer_function.is_some() {
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
        } else if header.format == Some(ktx2::Format::ASTC_4x4_SFLOAT_BLOCK) {
            /*let mut context = astcenc_rs::Context::new(
                astcenc_rs::ConfigBuilder::new()
                    .with_profile(astcenc_rs::Profile::HdrRgba)
                    .with_preset(astcenc_rs::Preset::Exhaustive)
                    .with_block_size(astcenc_rs::Extents::default_block_size())
                    .build()
                    .unwrap(),
            )
            .unwrap();

            dbg!(&header.pixel_depth, header.face_count);

            let image: Vec<f32> = level_bytes
                .chunks(level_bytes.len() / header.pixel_depth.max(1) as usize)
                .map(|chunk| {
                    context
                        .decompress(
                            chunk,
                            astcenc_rs::Extents::new(slice_width, slice_height),
                            astcenc_rs::Swizzle::rgba(),
                        )
                        .unwrap()
                })
                .flatten()
                .collect();

            Cow::Owned(image.iter().flat_map(|float| float.to_le_bytes()).collect())*/
            panic!("astc disabled for now")
        } else {
            level_bytes
        };

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
