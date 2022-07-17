use ktx2_tools::{Writer, WriterHeader, WriterLevel};
use std::collections::BTreeMap;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opts {
    input: PathBuf,
    output: PathBuf,
    #[structopt(long, default_value = "")]
    key_value_pairs: KeyValuePairs,
}

#[derive(Debug)]
struct KeyValuePairs(BTreeMap<String, String>);

impl std::str::FromStr for KeyValuePairs {
    type Err = String;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let key_value_pairs: Result<BTreeMap<String, String>, Self::Err> = string
            .split(';')
            .filter_map(|substr| {
                if substr.is_empty() {
                    None
                } else {
                    let result = substr
                        .split_once('=')
                        .map(|(key, value)| (String::from(key), String::from(value)))
                        .ok_or_else(|| format!("Could not find '=' in '{}'", substr));
                    Some(result)
                }
            })
            .collect();

        Ok(Self(key_value_pairs?))
    }
}

fn main() {
    let opts = Opts::from_args();

    let bytes = std::fs::read(&opts.input).unwrap();
    let ktx2 = ktx2::Reader::new(&bytes).unwrap();

    let header = ktx2.header();

    let num_levels = header
        .level_count
        .min((header.pixel_width.min(header.pixel_height) as f32).log2() as u32 - 1);

    println!("{:#?} {}", header, num_levels);

    let convert_to_half = match header.format {
        Some(ktx2::Format::R16G16B16A16_SFLOAT) => false,
        Some(ktx2::Format::R32G32B32A32_SFLOAT) => true,
        _ => {
            eprintln!("Unsupported frormat: {:?}", header.format);
            return;
        }
    };

    let writer = Writer {
        header: WriterHeader {
            format: Some(ktx2::Format::BC6H_UFLOAT_BLOCK),
            type_size: header.type_size,
            pixel_width: header.pixel_width,
            pixel_height: header.pixel_height,
            pixel_depth: header.pixel_depth,
            layer_count: header.layer_count,
            face_count: header.face_count,
            supercompression_scheme: Some(ktx2::SupercompressionScheme::Zstandard),
        },
        dfd_bytes: &bytes[header.index.dfd_byte_offset as usize
            ..(header.index.dfd_byte_offset + header.index.dfd_byte_length) as usize],
        key_value_pairs: &opts
            .key_value_pairs
            .0
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_bytes()))
            .collect(),
        sgd_bytes: &[],
        levels_descending: ktx2
            .levels()
            .take(num_levels as usize)
            .enumerate()
            .map(|(i, level)| {
                let mut level_bytes = match header.supercompression_scheme {
                    Some(ktx2::SupercompressionScheme::Zstandard) => std::borrow::Cow::Owned(
                        zstd::bulk::decompress(level.data, level.uncompressed_byte_length as usize)
                            .unwrap(),
                    ),
                    Some(other) => todo!("{:?}", other),
                    None => std::borrow::Cow::Borrowed(level.data),
                };

                if convert_to_half {
                    level_bytes = level_bytes
                        .chunks(4)
                        .flat_map(|bytes| {
                            half::f16::from_f32(f32::from_le_bytes(
                                <[u8; 4]>::try_from(bytes).unwrap(),
                            ))
                            .to_le_bytes()
                        })
                        .collect();
                }

                let width = header.pixel_width >> i;
                let height = header.pixel_height >> i;

                let mut compressed = Vec::new();

                for chunk in level_bytes.chunks(level_bytes.len() / 6) {
                    let compressed_chunk = intel_tex_2::bc6h::compress_blocks(
                        &intel_tex_2::bc6h::very_slow_settings(),
                        &intel_tex_2::RgbaSurface {
                            width,
                            height,
                            stride: width * 8,
                            data: chunk,
                        },
                    );

                    compressed.extend_from_slice(&compressed_chunk);
                }

                WriterLevel {
                    uncompressed_length: compressed.len(),
                    bytes: zstd::bulk::compress(&compressed, 0).unwrap(),
                }
            })
            .collect(),
    };

    writer
        .write(&mut std::fs::File::create(&opts.output).unwrap())
        .unwrap();
}
