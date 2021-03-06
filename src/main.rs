use std::{fs, path, io, time};

use image::ColorType;
use image::ImageEncoder;
use clap::Parser;
use std::time::Instant;

extern crate imagepipe;
extern crate rawloader;

/// Converts raw image files produced by cameras into image files
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// File or directory to parse
    #[clap(parse(from_os_str))]
    filename: std::path::PathBuf,

    /// Output file or directory (must not exist yet)
    #[clap(short, long, parse(from_os_str))]
    output: std::path::PathBuf,

    /// How to handle raw image files
    #[clap(short, long, value_enum, value_parser, default_value_t = ParsableAction::Parse)]
    raws: ParsableAction,

    /// How to handle parsed image files
    #[clap(short, long, value_enum, value_parser, default_value_t = UnparsableAction::Copy)]
    images: UnparsableAction,

    /// How to handle files other than raw or parsed images
    #[clap(short, long, value_enum, value_parser, default_value_t = UnparsableAction::Copy)]
    files: UnparsableAction,

    /// What to do if the output file already exists
    #[clap(short, long, value_enum, value_parser, default_value_t = ExistingAction::Ignore)]
    existing: ExistingAction,

    /// Which type to encode the images to
    #[clap(short('t'), long, value_enum, value_parser, default_value_t = EncodedType::Jpeg)]
    encode_type: EncodedType,

    /// Quality setting for jpeg encoding
    #[clap(long, default_value_t = 90)]
    jpeg_quality: u8,

}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ArgEnum)]
enum UnparsableAction {
    Copy, Move, Ignore,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ArgEnum)]
enum ParsableAction {
    Copy, Move, Ignore, Parse,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ArgEnum)]
enum ExistingAction {
    Rename, Ignore,
}



#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ArgEnum)]
enum EncodedType {
    Jpeg, Png, Tiff,
}

enum FileKind {
    Raw, Image, Other,
}

#[derive(Copy, Clone)]
enum EncoderType {
    JpegEncoder(u8),
    PngEncoder(image::codecs::png::CompressionType, image::codecs::png::FilterType),
    TiffEncoder,
}

const RAW_EXTENSIONS: [&'static str; 1] = [
    "CR2",
];

const IMG_EXTENSIONS: [&'static str; 8] = [
    "jpg", "jpeg", "png", "tiff",
    "JPG", "JPEG", "PNG", "TIFF",
];


fn recurse(dirname: &mut path::PathBuf) -> Vec<path::PathBuf> {
    let mut file_list = Vec::new();
    for entry in fs::read_dir(dirname).unwrap() {
        let entry = entry.unwrap();
        let meta = entry.metadata().unwrap();
        let path = entry.path();

        file_list.push(path);
        if meta.is_dir() {
            let mut subfiles = recurse(&mut file_list.pop().unwrap());
            file_list.append(&mut subfiles);
        }
    }
    return file_list;
}

fn print_files(files: &Vec<path::PathBuf>) {
    for file in files {
        println!("{:?}", file);
    }

}

fn raw_info(raw_path: &path::Path) {
    let from_time = Instant::now();
    let image = match rawloader::decode_file(raw_path) {
        Ok(val) => val,
        Err(e) => panic!("{:?}", e),
    };
    let duration = from_time.elapsed();

    println!();
    println!("-------------------------------------------------------");
    println!("Decoded in {} ms", duration.as_millis());
    println!("Found camera \"{}\" model \"{}\"", image.make, image.model);
    println!("Found clean named camera \"{}\" model \"{}\"", image.clean_make, image.clean_model);
    println!("Image size is {}x{}", image.width, image.height);
    println!("WB coeffs are {:?}", image.wb_coeffs);
    println!("black levels are {:?}", image.blacklevels);
    println!("white levels are {:?}", image.whitelevels);
    println!("xyz_to_cam is {:?}", image.xyz_to_cam);
    println!("CFA is {:?}", image.cfa);
    println!("crops are {:?}", image.crops);
    println!("-------------------------------------------------------");
    println!();
}

fn raw_info_short(raw_path: &path::Path) {
    let from_time = Instant::now();
    let image = match rawloader::decode_file(raw_path) {
        Ok(val) => val,
        Err(_e) => return,
    };
    let duration = from_time.elapsed();

    println!("File: {:?}", raw_path);
    println!("\tSize: {}x{}", image.width, image.height);
    println!("\tTaken with \"{}\"", image.model);
    println!("\tDecoded metadata in {} ms", duration.as_millis());
}

fn fmt_duration(duration: &time::Duration) -> String {
    let millis = duration.as_millis() % 1000;
    let secs = duration.as_secs() % 60;
    let mins = duration.as_secs() / 60;

    let mut string = String::new();

    if mins > 0 {
        string.push_str(format!("{}m ", mins).as_str());
    }
    if secs > 0 {
        string.push_str(format!("{}s ", secs).as_str());
    }
    string.push_str(format!("{}ms", millis).as_str());

    return string;
}

fn fmt_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return String::from(format!("{} B", bytes));
    } else if bytes < 1024 * 1024 {
        return String::from(format!("{:.2} KiB", (bytes as f64) / 1024.0));
    } else {
        return String::from(format!("{:.2} MiB", (bytes as f64) / (1024.0 * 1024.0)));
    }

}

fn decode_raw(path: &path::Path) -> Result<(imagepipe::SRGBImage, time::Duration), String> {
    let start_decode = Instant::now();
    let decoded = match imagepipe::simple_decode_8bit(path, 0, 0) {
        Ok(img) => img,
        Err(e) => return Err(e),
    };

    return Ok((decoded, start_decode.elapsed()));
}

fn encode_img(decoded: imagepipe::SRGBImage, path: &path::Path, encoder_type: EncoderType) -> Result<time::Duration, String> {
    let start_encode = Instant::now();

    let output_file = match fs::File::create(path) {
        Ok(val) => val,
        Err(e) => return Err(e.to_string()),
    };
    let bufwriter = io::BufWriter::new(output_file);

    let encode_result = match encoder_type {
        EncoderType::JpegEncoder(quality)
            => image::codecs::jpeg::JpegEncoder::new_with_quality(bufwriter, quality)
                .write_image(&decoded.data, decoded.width as u32, decoded.height as u32, ColorType::Rgb8),
        EncoderType::PngEncoder(compression, filter)
            => image::codecs::png::PngEncoder::new_with_quality(bufwriter, compression, filter)
                .write_image(&decoded.data, decoded.width as u32, decoded.height as u32, ColorType::Rgb8),
        EncoderType::TiffEncoder
            => image::codecs::tiff::TiffEncoder::new(bufwriter)
                .write_image(&decoded.data, decoded.width as u32, decoded.height as u32, ColorType::Rgb8),
    };

    return match encode_result {
        Ok(()) => Ok(start_encode.elapsed()),
        Err(e) => Err(e.to_string()),
    };
}

fn switch_base(path: &path::Path, old_base: &path::Path, new_base: &path::Path) -> Result<path::PathBuf, String> {
    match path.strip_prefix(old_base) {
        Ok(stripped) => return Ok(new_base.join(stripped)),
        Err(_e) => return Err(String::from("unable to switch base")),
    }
}

fn unused_path(orig_path: &path::Path) -> Result<path::PathBuf, String> {
    let parent = match orig_path.parent() {
        Some(parent) => parent,
        None => return Err(String::from("Unable to find unused path")),
    };
    let name = match orig_path.file_stem() {
        Some(stem) => match stem.to_str() {
            Some(string) => string,
            None => return Err(String::from("Unable to find unused path")),
        },
        None => return Err(String::from("Unable to find unused path")),
    };
    let extension = match orig_path.extension() {
        Some(extension) => match extension.to_str() {
            Some(string) => string,
            None => return Err(String::from("Unable to find unused path")),
        },
        None => "",
    };

    let extended_name = | i | format!("{}_{}.{}", name, i, extension);
    let new_path = | i | parent.join(path::Path::new(&extended_name(i)));

    let mut i = 1;
    while new_path(i).exists() {
        i += 1;
    }

    return Ok(new_path(i));
}

fn file_kind(path: &path::Path) -> FileKind {
    return match path.extension() {
        Some(extension) => match extension.to_str() {
            Some(ext) => {
                if RAW_EXTENSIONS.iter().any(|e| *e == ext) {
                    FileKind::Raw
                } else if IMG_EXTENSIONS.iter().any(|e| *e == ext) {
                    FileKind::Image
                } else {
                    FileKind::Other
                }
            },
            None => FileKind::Other,
        },
        None => FileKind::Other,
    };
}

fn recode(input_path: &path::Path, output_path: &path::Path, encoder: EncoderType) -> Option<(time::Duration, time::Duration)> {
    println!("Decoding {:?}", input_path);
    let (decoded, decode_time) = match decode_raw(input_path) {
        Ok((decoded, decode_time)) => (decoded, decode_time),
        Err(e) => { println!("Unable to decode {:?}: {:?}", input_path, e); return None },
    };
    println!("Decoded {:?} in {}", input_path, fmt_duration(&decode_time));

    println!("Encoding {:?}", output_path);
    let encode_time = match encode_img(decoded, output_path, encoder) {
        Ok(encode_time) => encode_time,
        Err(e) => { println!("Unable to encode {:?}: {:?}", output_path, e); return None },
    };
    println!("Encoded {:?} in {}", output_path, fmt_duration(&encode_time));

    return Some((decode_time, encode_time));
}

fn copy(input_path: &path::Path, output_path: &path::Path) -> Option<time::Duration> {
    if input_path == output_path {
        return None;
    }

    let start_time = time::Instant::now();

    println!("Copying {:?} to {:?}", input_path, output_path);
    let bytes = match fs::copy(input_path, output_path) {
        Ok(bytes) => bytes,
        Err(e) => { println!("Unable to copy {:?}: {:?}", output_path, e); return None },
    };

    let time = start_time.elapsed();
    println!("Copied {} to {:?} in {}", fmt_bytes(bytes), output_path, fmt_duration(&time));
    return Some(time);
}

fn move_file(input_path: &path::Path, output_path: &path::Path) -> Option<time::Duration> {
    if input_path == output_path {
        return None;
    }

    let start_time = time::Instant::now();

    println!("Moving {:?} to {:?}", input_path, output_path);
    match fs::rename(input_path, output_path) {
        Ok(()) => (),
        Err(e) => { println!("Unable to move {:?}: {:?}", output_path, e); return None },
    };

    let time = start_time.elapsed();
    println!("Moved {:?} to {:?} in {}", input_path, output_path, fmt_duration(&time));
    return Some(time);
}

fn main() {
    let mut args = Args::parse();

    let start_time = time::Instant::now();
    let mut file_counter = 0;
    let mut ignored_counter = 0;
    let mut err_counter = 0;
    let mut decode_time = time::Duration::new(0, 0);
    let mut decode_counter = 0;
    let mut encode_time = time::Duration::new(0, 0);
    let mut encode_counter = 0;
    let mut copy_time = time::Duration::new(0, 0);
    let mut copy_counter = 0;
    let mut move_time = time::Duration::new(0, 0);
    let mut move_counter = 0;

    let encoder = match args.encode_type {
        EncodedType::Jpeg => EncoderType::JpegEncoder(args.jpeg_quality),
        EncodedType::Png => EncoderType::PngEncoder(image::codecs::png::CompressionType::Default,
                                                   image::codecs::png::FilterType::Adaptive),
        EncodedType::Tiff => EncoderType::TiffEncoder,
    };
    let extension = match args.encode_type {
        EncodedType::Jpeg => "jpg",
        EncodedType::Png => "png",
        EncodedType::Tiff => "tiff",
    };


    if args.filename.as_path().metadata().expect("unable to get file attributes").is_dir() {
        let files = recurse(&mut args.filename);
        let input_base = args.filename.as_path();
        let output_base = args.output.as_path();

        for file in &files {
            println!();
            let output_pathbuf = match switch_base(file.as_path(), input_base, output_base) {
                Ok(pathbuf) => pathbuf,
                Err(e) => { println!("Unable to switch base for {:?}: {:?}", file, e); continue },
            };

            let metadata = match file.metadata() {
                Ok(metadata) => metadata,
                Err(e) => { println!("Unable to get file attributes for {:?}: {:?}", file, e); continue },
            };

            if output_pathbuf.parent().is_some() && !output_pathbuf.parent().unwrap().exists() {
                let parent = output_pathbuf.parent().unwrap();
                if fs::create_dir_all(&parent).is_err() {
                    println!("Unable to create dir {:?} as parent for {:?}", parent, output_pathbuf);
                    err_counter += 1;
                    continue;
                }
            }

            if metadata.is_file() {
                let decode_pathbuf = output_pathbuf.with_extension(extension);
                let mut output_path = match file_kind(file) {
                    FileKind::Raw => match args.raws {
                        ParsableAction::Parse => decode_pathbuf.as_path(),
                        _ => output_pathbuf.as_path(),
                    }
                    _ => output_pathbuf.as_path(),
                };
                let mut alternative = output_path.to_path_buf().clone();

                if output_path.exists() {
                    match args.existing {
                        ExistingAction::Rename => {
                            alternative = match unused_path(output_path) {
                                Ok(path) => path,
                                Err(e) => {
                                    ignored_counter += 1;
                                    println!("Could not find unused path for {:?} ({}), it will be ignored", output_path, e);
                                    continue;
                                }
                            };
                            output_path = &alternative;
                        },
                        ExistingAction::Ignore => {
                            ignored_counter += 1;
                            println!("{:?} already exists and will *not* be overwritten", output_path);
                            continue;
                        }
                    }
                }

                match file_kind(file) {
                    FileKind::Raw => match args.raws {
                        ParsableAction::Ignore => {
                            println!("Ignoring {:?}", file);
                            ignored_counter += 1;
                        },
                        ParsableAction::Parse =>
                            match recode(file.as_path(), output_path, encoder) {
                                Some((dtime, etime)) => {
                                    decode_time += dtime;
                                    decode_counter += 1;
                                    encode_time += etime;
                                    encode_counter += 1;
                                },
                                None => err_counter += 1,
                            },
                        ParsableAction::Copy =>
                            match copy(file.as_path(), output_path) {
                                Some(ctime) => { copy_time += ctime; copy_counter += 1 },
                                None => err_counter += 1,
                            },
                        ParsableAction::Move =>
                            match move_file(file.as_path(), output_path) {
                                Some(mtime) => { move_time += mtime; move_counter += 1 },
                                None => err_counter += 1,
                            },
                    },
                    FileKind::Image => match args.images {
                        UnparsableAction::Ignore => {
                            println!("Ignoring {:?}", file);
                            ignored_counter += 1;
                        },
                        UnparsableAction::Copy =>
                            match copy(file.as_path(), output_path) {
                                Some(ctime) => { copy_time += ctime; copy_counter += 1 },
                                None => err_counter += 1,
                            },
                        UnparsableAction::Move =>
                            match move_file(file.as_path(), output_path) {
                                Some(mtime) => { move_time += mtime; move_counter += 1 },
                                None => err_counter += 1,
                            },
                    },
                    FileKind::Other => match args.files {
                        UnparsableAction::Ignore => {
                            println!("Ignoring {:?}", file);
                            ignored_counter += 1;
                        },
                        UnparsableAction::Copy =>
                            match copy(file.as_path(), output_path) {
                                Some(ctime) => { copy_time += ctime; copy_counter += 1 },
                                None => err_counter += 1,
                            },
                        UnparsableAction::Move =>
                            match move_file(file.as_path(), output_path) {
                                Some(mtime) => { move_time += mtime; move_counter += 1 },
                                None => err_counter += 1,
                            },
                    },
                };
                file_counter += 1;
            } else if output_pathbuf.exists() {
                println!("{:?} already exists and will therefore be ignored", output_pathbuf);
            } else if metadata.is_dir() { // recurse() currently does not pick up directories
                println!("Ignoring {:?}: directories will be created on demand", file);
            } else {
                println!("Ignoring {:?}", file);
            }

        }
    } else {
        raw_info_short(&args.filename.as_path());
        match recode(&args.filename.as_path(), &args.output, encoder) {
            Some((dtime, etime)) => {
                decode_time += dtime;
                decode_counter += 1;
                encode_time += etime;
                encode_counter += 1;
            },
            None => err_counter += 1,
        };
    }

    let total_time = start_time.elapsed();

    if file_counter > 0 {
        println!("");
        println!("DONE");
        println!("");

        let per_file = total_time / file_counter;
        println!("Processed {:?} files in {} (avg {} per file)",
                    file_counter, fmt_duration(&total_time), fmt_duration(&per_file));

        if decode_counter > 0 {
            let avg_decode_time = decode_time / decode_counter;
            println!("Decoded {:?} raw image files in {} (avg {} per file)",
                        decode_counter, fmt_duration(&decode_time), fmt_duration(&avg_decode_time));
        }
        if encode_counter > 0 {
            let avg_encode_time = encode_time / encode_counter;
            println!("Encoded {:?} image files in {} (avg {} per file)",
                        encode_counter, fmt_duration(&encode_time), fmt_duration(&avg_encode_time));
        }
        if copy_counter > 0 {
            let avg_copy_time = copy_time / copy_counter;
            println!("Copied {:?} files in {} (avg {} per file)",
                        copy_counter, fmt_duration(&copy_time), fmt_duration(&avg_copy_time));
        }
        if move_counter > 0 {
            let avg_move_time = move_time / move_counter;
            println!("Moved {:?} files in {} (avg {} per file)",
                        move_counter, fmt_duration(&move_time), fmt_duration(&avg_move_time));
        }
        println!("Ran into {:?} errors and ignored {:?} files", err_counter, ignored_counter);
    } else {
        println!("Found no files to process in {:?}", args.filename);
    }

}
