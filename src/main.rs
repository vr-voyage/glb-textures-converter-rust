use chrono::Local;
use std::io::Write;
use std::env;
use std::fs::OpenOptions;
use std::path::{Path,PathBuf};

mod converter;

/* I'm completely new to Rust so if you see anything
 * wrong, it probably is.
 * I'm just focusing on the features on the moment.
 */

fn main() {
    let args: Vec<String> = env::args().collect();
    let n_args = args.len();
    if n_args < 2
    {
        println!("Usage : {} file.glb [out.glb]", args[0]);
        return;
    }

    let glb_filename = &args[1];
    let output_filepath:PathBuf;

    if n_args >= 3
    {
        output_filepath = PathBuf::from(&args[2]);
    }
    else
    {
        let input_path = Path::new(glb_filename);
        let glb_filename_without_ext:&str = input_path.file_stem().unwrap().to_str().unwrap();
        let current_date = Local::now();
        let current_time = current_date.format("%Y%m%d%H%M%S");
        let output_filename = format!("{}-{}-converted.glb", glb_filename_without_ext, current_time);
        output_filepath = input_path.with_file_name(output_filename);
    }

    match std::fs::read(glb_filename) {
        Ok(bytes) => { 
            let out_glb = converter::create_new_glb_with_converted_textures(bytes);
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




