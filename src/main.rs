use std::{fs, path, io, time};
use threadpool::ThreadPool;
use std::sync::mpsc::channel;

use image::ColorType;
use image::ImageEncoder;
use clap::Parser;
use std::time::Instant;
use std::path::*;

extern crate imagepipe;
extern crate rawloader;

use job::*;
use statistics::*;

mod job;
mod statistics;

/// Converts raw image files produced by cameras into image files
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// File or directory to parse
    #[clap()]
    filename: std::path::PathBuf,

    /// Output file or directory (must not exist yet)
    #[clap(short, long)]
    output: std::path::PathBuf,

    /// How to handle raw image files
    #[clap(short, long, value_enum, value_parser, default_value_t = ParsableAction::Parse)]
    #[arg(value_enum)]
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
    #[clap(short('n'), long, value_enum, value_parser, default_value_t = EncodedType::Jpeg)]
    encode_type: EncodedType,

    /// Quality setting for jpeg encoding
    #[clap(long, default_value_t = 90)]
    jpeg_quality: u8,

    /// Number of threads to run in parallel
    #[clap(short, long, default_value_t = 1)]
    threads: usize,

}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum UnparsableAction {
    Copy, Move, Ignore,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum ParsableAction {
    Copy, Move, Ignore, Parse,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum ExistingAction {
    Rename, Ignore,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum EncodedType {
    Jpeg, Png, Tiff,
}

pub enum FileKind {
    Raw, Image, Other,
}

#[derive(Copy, Clone)]
pub enum EncoderType {
    JpegEncoder(u8),
    PngEncoder(image::codecs::png::CompressionType, image::codecs::png::FilterType),
    TiffEncoder,
}

const RAW_EXTENSIONS: [&str; 3] = [
    "arw", "cr2", "raw",
];

const IMG_EXTENSIONS: [&str; 4] = [
    "jpg", "jpeg", "png", "tiff",
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
    file_list
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

    string
}

fn fmt_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        return format!("{:.2} KiB", (bytes as f64) / 1024.0);
    } else {
        return format!("{:.2} MiB", (bytes as f64) / (1024.0 * 1024.0));
    }

}

fn decode_raw(path: &path::Path) -> Result<(imagepipe::SRGBImage, time::Duration), String> {
    let start_decode = Instant::now();
    let decoded = match imagepipe::simple_decode_8bit(path, 0, 0) {
        Ok(img) => img,
        Err(e) => return Err(e),
    };

    Ok((decoded, start_decode.elapsed()))
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

    match encode_result {
        Ok(()) => Ok(start_encode.elapsed()),
        Err(e) => Err(e.to_string()),
    }
}

fn output_path(input: &Path, input_base: &Path, output_base: &Path, extension: &str,
               on_raw: ParsableAction, on_existing: ExistingAction) -> Result<std::path::PathBuf, String> {
    let output_with_base = switch_base(input, input_base, output_base)?;

    let decode_pathbuf = output_with_base.with_extension(extension);
    let output_with_extension = match file_kind(input) {
        FileKind::Raw => match on_raw {
            ParsableAction::Parse => decode_pathbuf.as_path(),
            _ => output_with_base.as_path(),
        }
        _ => output_with_base.as_path(),
    };


    if output_with_extension.exists() && on_existing == ExistingAction::Rename {
        unused_path(output_with_extension)
            .map_err(|e| format!("Could not find unused path for {:?} ({}), it will be ignored", output_with_extension, e))
    } else {
        Ok(output_with_extension.to_path_buf())
    }
}

fn switch_base(path: &path::Path, old_base: &path::Path, new_base: &path::Path) -> Result<path::PathBuf, String> {
    match path.strip_prefix(old_base) {
        Ok(stripped) => Ok(new_base.join(stripped)),
        Err(_e) => Err(String::from("unable to switch base")),
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

    Ok(new_path(i))
}

fn file_kind(path: &path::Path) -> FileKind {
    return match path.extension() {
        Some(extension) => match extension.to_str() {
            Some(ext) => {
                if RAW_EXTENSIONS.iter().any(|e| e.to_lowercase() == ext.to_lowercase()) {
                    FileKind::Raw
                } else if IMG_EXTENSIONS.iter().any(|e| e.to_lowercase() == ext.to_lowercase()) {
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

    Some((decode_time, encode_time))
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
    Some(time)
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
    Some(time)
}

fn process_files(files: &Vec<PathBuf>, input_base: &Path, output_base: &Path,
                          extension: &str, encoder: EncoderType, args: &Args) -> Statistics {
    println!("Running in single job mode");

    let mut acc_stats = Statistics::default();
    let mut last_job_time = Instant::now();
    for file in files {
        let output_file = output_path(file, input_base, output_base, extension, args.raws, args.existing).unwrap();
        let job = Job::new(file, &output_file, args.raws, args.files, args.images, args.existing, encoder);
        let name = job.name();

        let stats = match job.run() {
            Ok(stats) => stats,
            Err(e) => {
                println!("Error ({}): {}", name, e);
                let mut stats = Statistics::default();
                stats.errors.inc();
                stats
            },
        };

        let now = Instant::now();
        acc_stats.total.record(now - last_job_time);
        last_job_time = now;
        acc_stats.extend(&stats);

        println!("Finished job {} ({}/{})", name, acc_stats.total.count(), files.len());
    }

    acc_stats
}

fn process_files_parallel(files: &Vec<PathBuf>, input_base: &Path, output_base: &Path,
                          extension: &str, encoder: EncoderType, args: &Args) -> Statistics {
    println!("Starting new thread pool running {} threads in parallel", args.threads);

    let mut last_job_time = time::Instant::now();
    let pool = ThreadPool::new(args.threads);
    let (tx, rx) = channel();

    for file in files {
        let output_file = output_path(file, input_base, output_base, extension, args.raws, args.existing).unwrap();
        let job = Job::new(file, &output_file, args.raws, args.files, args.images, args.existing, encoder);

        let next_tx = tx.clone();
        pool.execute(move || {
            let name = job.name();
            let stats = job.run();
            match stats {
                Ok(stats) => next_tx.send((name, stats)).unwrap(),
                Err(e) => {
                    println!("Error ({}): {}", name, e);
                    let mut stats = Statistics::default();
                    stats.errors.inc();
                    next_tx.send((name, stats)).unwrap();
                },
            }
        });
    }

    // pool.join();
    let mut acc_stats = Statistics::default();
    rx.iter().take(files.len()).fold(&mut acc_stats, |acc, (name, stats)| {
        let now = Instant::now();
        acc.total.record(now - last_job_time);
        last_job_time = now;
        println!("Finished job {} ({}/{})", name, acc.total.count(), files.len());
        acc.extend(&stats)
    });
    acc_stats
}

fn main() {
    let args = Args::parse();
    let mut statistics = Statistics::default();

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
        let files = recurse(&mut args.filename.clone());
        let input_base = args.filename.clone();
        let output_base = args.output.clone();

        if args.threads > 1 {
            statistics = process_files_parallel(&files, &input_base, &output_base, extension, encoder, &args);
        } else {
            statistics = process_files(&files, &input_base, &output_base, extension, encoder, &args);
        }

    } else {
        let starting = Instant::now();
        raw_info_short(args.filename.as_path());
        match recode(args.filename.as_path(), &args.output, encoder) {
            Some((dtime, etime)) => {
                let ending = Instant::now();
                statistics.total.record(ending - starting);
                statistics.decoded.record(dtime);
                statistics.encoded.record(etime);
            },
            None => statistics.errors.inc(),
        };
    }

    if statistics.total.count() > 0 || statistics.errors.count() > 0 {
        println!();
        println!("DONE");
        println!();

        statistics.print_nthreads(args.threads.try_into().unwrap());
    } else {
        println!("Found no files to process in {:?}", args.filename);
    }
}
