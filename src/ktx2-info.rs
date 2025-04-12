use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opts {
    filename: PathBuf,
    #[structopt(long)]
    print_compression_percentage: bool,
}

fn main() {
    let opts = Opts::from_args();

    let file = std::fs::read(&opts.filename).unwrap();
    let ktx2 = ktx2::Reader::new(&file).unwrap();

    let header = ktx2.header();

    if opts.print_compression_percentage {
        let mut uncompressed_size = file.len() as i64;

        for level in ktx2.levels() {
            uncompressed_size += level.uncompressed_byte_length as i64 - level.data.len() as i64;
        }

        println!(
            "{}: {:.2}%",
            &opts.filename.display(),
            file.len() as f32 / uncompressed_size as f32 * 100.0
        );

        return;
    }

    let mut width = header.pixel_width;
    let mut height = header.pixel_height;

    println!("[File {}]", &opts.filename.display());
    println!("Width: {}", width);
    println!("Height: {}", height);
    println!("Depth: {}", header.pixel_depth);
    println!("Level Count: {}", header.level_count);
    println!(
        "Supercompression Scheme: {:?}",
        header.supercompression_scheme
    );
    println!("Format: {:?}", header.format);
    println!();

    for (i, dfd) in ktx2.dfd_blocks().enumerate() {
        println!("[Data format descriptor {}]", i);
        println!("Vendor ID: {}", dfd.header.vendor_id);
        println!("Version number: {}", dfd.header.version_number);

        /*if let Ok(basis_dfd) = ktx2::DfdBlockHeaderBasic::from_bytes(dfd.data) {
            println!("[[Basis Universal data format descriptor]]");
            println!("Color model: {:?}", basis_dfd.header.color_model);
            println!("Color primaries: {:?}", basis_dfd.header.color_primaries);
            println!(
                "Transfer function: {:?}",
                basis_dfd.header.transfer_function
            );
            println!("Flags: {:?}", basis_dfd.header.flags);
        }*/

        println!();
    }

    println!("[Key Value Pairs]");

    for (key, value) in ktx2.key_value_data() {
        println!("{}: {}", key, String::from_utf8_lossy(value));
    }

    println!();

    for (i, level) in ktx2.levels().enumerate() {
        println!("[Level {} (width: {}, height: {})]", i, width, height);
        println!("Byte length: {}", level.data.len());
        println!(
            "Uncompressed byte length: {}",
            level.uncompressed_byte_length
        );
        println!(
            "Compressed size: {:.2}%",
            level.data.len() as f32 / level.uncompressed_byte_length as f32 * 100.0
        );
        println!();

        width >>= 1;
        height >>= 1;
    }
}
