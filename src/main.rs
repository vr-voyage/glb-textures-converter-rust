use std::io::Write;
use std::env;
use std::fs::OpenOptions;

mod converter;

/* I'm completely new to Rust so if you see anything
 * wrong, it probably is.
 * I'm just focusing on the features on the moment.
 */

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3
    {
        println!("Usage : {} file.glb out.glb", args[0]);
        return;
    }

    let glb_filename = &args[1];
    let output_filename = &args[2];

    match std::fs::read(glb_filename) {
        Ok(bytes) => { 
            let out_glb = converter::create_new_glb_with_converted_textures(bytes);
            let mut f = OpenOptions::new().write(true).create(true).open(output_filename).unwrap();
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




