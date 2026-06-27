use clap::{Parser};
use chrono::Local;
use std::io::Write;
use std::fs::OpenOptions;
use std::path::{PathBuf};

mod converter;
mod glb_handler;

/* I'm completely new to Rust so if you see anything
 * wrong, it probably is.
 * I'm just focusing on the features on the moment.
 */

#[derive(clap::Parser, Debug)]
#[command(version,about)]
struct Cli
{
    glb_filepath: PathBuf,
    output_filename: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "bc7")]
    compression_format: Option<converter::CompressionFormat>
}

fn generate_output_filepath(input_path:&PathBuf) -> PathBuf
{
    let glb_filename_without_ext:&str = input_path.file_stem().unwrap().to_str().unwrap();
    let current_date = Local::now();
    let current_time = current_date.format("%Y%m%d%H%M%S");
    let output_filename = format!("{}-{}-converted.glb", glb_filename_without_ext, current_time);
    return input_path.with_file_name(output_filename);
}

fn main() {

    let cli: Cli = Cli::parse();

    let output_filepath = cli.output_filename.unwrap_or(generate_output_filepath(&cli.glb_filepath));

    match std::fs::read(cli.glb_filepath) {
        Ok(bytes) => { 
            let out_glb = glb_handler::create_new_glb_with_converted_textures(bytes, &cli.compression_format);
            let mut f = OpenOptions::new().write(true).create(true).open(output_filepath).unwrap();
            let _ = f.write(&out_glb[..]);
            let _ = f.sync_all().unwrap();
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                eprintln!("please run again with appropriate permissions.");
                return;
            }
            panic!("{}", e);
        }
    }
}




