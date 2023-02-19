pub use ktx2;
use std::borrow::Cow;
use std::collections::BTreeMap;

pub struct Writer<'a> {
    pub header: WriterHeader,
    pub dfd_bytes: &'a [u8],
    pub key_value_pairs: &'a BTreeMap<String, Vec<u8>>,
    pub sgd_bytes: &'a [u8],
    pub uncompressed_levels_descending: &'a [Cow<'a, [u8]>],
}

impl<'a> Writer<'a> {
    pub fn write<T: std::io::Write>(&self, writer: &mut T) -> std::io::Result<()> {
        let dfd_offset = ktx2::Header::LENGTH
            + self.uncompressed_levels_descending.len() * ktx2::LevelIndex::LENGTH;

        let mut key_value_pairs = self.key_value_pairs.clone();

        key_value_pairs.insert(
            "KTXwriter".to_string(),
            concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"), "\0")
                .as_bytes()
                .to_vec(),
        );

        let mut kvd_bytes = Vec::new();

        for (key, value) in key_value_pairs.iter() {
            let length = (key.len() + 1 + value.len()) as u32;

            kvd_bytes.extend_from_slice(&length.to_le_bytes());

            kvd_bytes.extend_from_slice(key.as_bytes());

            kvd_bytes.push(b'\0');

            kvd_bytes.extend_from_slice(value);

            while kvd_bytes.len() % 4 != 0 {
                kvd_bytes.push(0);
            }
        }

        writer.write_all(
            &ktx2::Header {
                format: self.header.format,
                type_size: self.header.type_size,
                pixel_width: self.header.pixel_width,
                pixel_height: self.header.pixel_height,
                pixel_depth: self.header.pixel_depth,
                layer_count: self.header.layer_count,
                face_count: self.header.face_count,
                supercompression_scheme: self.header.supercompression_scheme,
                level_count: self.uncompressed_levels_descending.len() as u32,
                index: ktx2::Index {
                    dfd_byte_length: self.dfd_bytes.len() as u32,
                    kvd_byte_length: kvd_bytes.len() as u32,
                    sgd_byte_length: self.sgd_bytes.len() as u64,
                    dfd_byte_offset: dfd_offset as u32,
                    kvd_byte_offset: if kvd_bytes.is_empty() {
                        0
                    } else {
                        dfd_offset + self.dfd_bytes.len()
                    } as u32,
                    sgd_byte_offset: if self.sgd_bytes.is_empty() {
                        0
                    } else {
                        dfd_offset + self.dfd_bytes.len() + kvd_bytes.len()
                    } as u64,
                },
            }
            .as_bytes()[..],
        )?;

        let mut offset = dfd_offset + self.dfd_bytes.len() + kvd_bytes.len() + self.sgd_bytes.len();

        let compressed_levels: Vec<Cow<[u8]>> = self
            .uncompressed_levels_descending
            .iter()
            .map(|level| match self.header.supercompression_scheme {
                Some(ktx2::SupercompressionScheme::Zstandard) => {
                    Cow::Owned(zstd::bulk::compress(level, 0).unwrap())
                }
                Some(other) => panic!("{:?}", other),
                None => {
                    let level: &[u8] = level;
                    Cow::Borrowed(level)
                }
            })
            .collect();

        let mut levels = self
            .uncompressed_levels_descending
            .iter()
            .zip(&compressed_levels)
            .rev()
            .map(|(uncompressed_level, level)| {
                let index = ktx2::LevelIndex {
                    byte_offset: offset as u64,
                    byte_length: level.len() as u64,
                    uncompressed_byte_length: uncompressed_level.len() as u64,
                };

                offset += level.len();

                index
            })
            .collect::<Vec<_>>();

        levels.reverse();

        for level in levels {
            writer.write_all(&level.as_bytes())?;
        }

        writer.write_all(self.dfd_bytes)?;
        writer.write_all(&kvd_bytes)?;
        writer.write_all(self.sgd_bytes)?;

        for level in compressed_levels.iter().rev() {
            writer.write_all(level)?;
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WriterHeader {
    pub format: Option<ktx2::Format>,
    pub type_size: u32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub pixel_depth: u32,
    pub layer_count: u32,
    pub face_count: u32,
    pub supercompression_scheme: Option<ktx2::SupercompressionScheme>,
}
