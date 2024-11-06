use bytes::Buf;
use core::mem::size_of;
use ddsfile::{AlphaMode, Caps2, D3D10ResourceDimension, Dds, DxgiFormat, NewDxgiParams};
use image::io::Reader;
use intel_tex_2::bc7;
use std::io::Cursor;
use std::io::Write;
use serde_json::Value;
use serde_json::json;
use serde::{Deserialize};

const GLB_HEADER_MAGIC:u32 = 0x46546C67;
const GLB_JSON_CHUNK_MAGIC:u32 = 0x4E4F534A;
const GLB_DATA_CHUNK_MAGIC:u32 = 0x004E4942;
const NOT_USED_BY_IMAGE_BUFFER:i32 = -1;
const GLB_VERSION:u32 = 2;

#[derive(Deserialize)]
struct Image
{
    #[serde(alias = "bufferView")]
    buffer_view_index: u32,
    #[serde(alias = "mimeType")]
    mime_type: String
}

#[derive(serde::Deserialize)]
struct BufferView
{
    buffer: u32,
    #[serde(alias = "byteLength")]
    length: u32,
    #[serde(default)]
    #[serde(alias = "byteOffset")]
    offset: u32,
    #[serde(skip)]
    used_by_image: i32
}
#[derive(serde::Deserialize)]
struct Buffer
{
    #[serde(alias = "byteLength")]
    length: u32
}

#[derive(serde::Deserialize)]
struct GLTF
{
    images: Vec<Image>,
    #[serde(alias = "bufferViews")]
    buffer_views: Vec<BufferView>,
    buffers: Vec<Buffer>
}

pub fn create_new_glb_with_converted_textures(glb: Vec<u8>) -> Vec<u8>
{

    let mut glb_data = glb.as_slice();

    let magic: u32 = glb_data.get_u32_le();
    if magic != 0x46546C67
    {
        println!("Magic was {:x}", magic);
        println!("{:x}{:x}{:x}{:x}", glb[0], glb[1], glb[2], glb[3]);
        panic!("Invalid magic header");
    }

    let _version: u32 = glb_data.get_u32_le();
    let length: u32 = glb_data.get_u32_le();

    if glb.len() < length as usize
    {
        panic!("Not enough data in the GLB file !");
    }

    /* FIXME : Don't expect the first block to be the JSON one */
    let json_block_length: u32 = glb_data.get_u32_le();
    let json_block_type: u32   = glb_data.get_u32_le();
    if json_block_type != GLB_JSON_CHUNK_MAGIC
    {
        panic!(
            "Expecting a JSON Chunk with the following type {:x}, got {:x} instead", 
            json_block_type, GLB_JSON_CHUNK_MAGIC);
    }
    let json_content = std::str::from_utf8(&glb_data[..json_block_length as usize]).unwrap();

    let mut gltf_json: Value = serde_json::from_str(&json_content).unwrap();
    let mut gltf: GLTF = serde_json::from_str(&json_content).unwrap();
    let gltf_buffer_views: &mut Vec<BufferView> = &mut gltf.buffer_views;

    for b in 0..gltf.buffers.len()
    {
        println!("Buffer[{}] : Size - {}", b, gltf.buffers[b].length);
    }
    for bv in 0..gltf_buffer_views.len()
    {
        let buffer_view: &BufferView = &gltf_buffer_views[bv];
        println!("BufferView[{}] : Buffer - {} Offset - {} Size - {}", bv, buffer_view.buffer, buffer_view.offset, buffer_view.length);
    }
    for i in 0..gltf.images.len()
    {
        let image: &Image = &gltf.images[i];
        println!("Image[{}] : Buffer View - {} MimeType : {}", i, image.buffer_view_index, image.mime_type);
    }

    glb_data.advance(json_block_length as usize);
    let _binary_block_length: u32 = glb_data.get_u32_le();
    let binary_block_type: u32 = glb_data.get_u32_le();
    if binary_block_type != GLB_DATA_CHUNK_MAGIC
    {
        panic!("Wrong magic for the Binary Chunk. Expected {:x} got {:x}", GLB_DATA_CHUNK_MAGIC, binary_block_type);
    }
    let new_buffer = convert_images_and_rebuild_buffer(&gltf.images, gltf_buffer_views, &mut gltf_json, &glb_data);

    /* Pretty ugly, I should try to see if I can
     * - allocate memory BEFORE new_buffer
     * - store "glb header/json chunk header/json chunk data/binary chunk header" just before it
     */
    return create_glb(&gltf_json.to_string(), &new_buffer);

    

    /*
    let rgb_img = image::open(Path::new(&args[1])).unwrap();

    let (width, height) = rgb_img.dimensions();
    println!("Width is {}", width);
    println!("Height is {}", height);
    println!("ColorType is {:?}", rgb_img.color());

    let mut rgba_img = ImageBuffer::new(width, height);
    let mut rg_img = ImageBuffer::new(width, height);
    let mut r_img = ImageBuffer::new(width, height);

    println!("Converting RGB -> RGBA/RG/R"); // could be optimized
    for x in 0u32..width {
        for y in 0u32..height {
            let pixel = rgb_img.get_pixel(x, y);
            let pixel_rgba = pixel.to_rgba();
            let pixel_rg = LumaA::from([pixel_rgba[0], pixel_rgba[1]]);
            let pixel_r = Luma::from([pixel_rgba[0]]);
            rgba_img.put_pixel(x, y, pixel_rgba);
            rg_img.put_pixel(x, y, pixel_rg);
            r_img.put_pixel(x, y, pixel_r);
        }
    }

    let block_count = intel_tex_2::divide_up_by_multiple(width * height, 16);
    println!("Block count: {}", block_count);
    let dds_defaults = NewDxgiParams {
        height,
        width,
        depth: Some(1),
        format: DxgiFormat::BC7_UNorm,
        mipmap_levels: Some(1),
        array_layers: Some(1),
        caps2: Some(Caps2::empty()),
        is_cubemap: false,
        resource_dimension: D3D10ResourceDimension::Texture2D,
        alpha_mode: AlphaMode::Opaque,
    };
    // BC7
    {
        let mut dds = Dds::new_dxgi(NewDxgiParams {
            format: DxgiFormat::BC7_UNorm,
            ..dds_defaults
        })
        .unwrap();
        let surface = intel_tex_2::RgbaSurface {
            width,
            height,
            stride: width * 4,
            data: &rgba_img,
        };

        println!("Compressing to BC7...");
        bc7::compress_blocks_into(
            &bc7::opaque_ultra_fast_settings(),
            &surface,
            dds.get_mut_data(0 /* layer */).unwrap(),
        );
        println!("  Done!");
        println!("Saving lambertian_bc7.dds file");
        let mut dds_file = File::create(&args[2]).unwrap();
        dds.write(&mut dds_file).expect("Failed to write dds file");
    }*/
}

fn create_glb(json_content: &str, binary_content: &Vec<u8>) -> Vec<u8>
{
    let json_chunk_length: u32 = json_content.len() as u32;
    let binary_chunk_length: u32 = binary_content.len() as u32;
    let chunk_header_length: u32 = (size_of::<u32>() * 2) as u32;
    let glb_header_length: u32 = (size_of::<u32>() * 3) as u32;

    let total_length:u32 =
        glb_header_length
        + chunk_header_length
        + json_chunk_length
        + chunk_header_length
        + binary_chunk_length;

    let mut out_buffer: Vec<u8> = Vec::new();
    let mut writer = Cursor::new(&mut out_buffer);

    let _ = writer.write(&GLB_HEADER_MAGIC.to_le_bytes());
    let _ = writer.write(&GLB_VERSION.to_le_bytes());
    let _ = writer.write(&total_length.to_le_bytes());

    let _ = writer.write(&json_chunk_length.to_le_bytes());
    let _ = writer.write(&GLB_JSON_CHUNK_MAGIC.to_le_bytes());
    let _ = writer.write(json_content.as_bytes());

    let _ = writer.write(&binary_chunk_length.to_le_bytes());
    let _ = writer.write(&GLB_DATA_CHUNK_MAGIC.to_le_bytes());
    let _ = writer.write(&binary_content[..]);

    return out_buffer.into();
}

fn mark_bufferview_used_by_images(images: &Vec<Image>, buffer_views: &mut Vec<BufferView>)
{
    /* Initialize all the the fields first */
    for b in 0..buffer_views.len()
    {
        let buffer_view: &mut BufferView = &mut buffer_views[b];
        buffer_view.used_by_image = NOT_USED_BY_IMAGE_BUFFER;
    }

    for i in 0..images.len()
    {
        let image: &Image = &images[i];
        let buffer_view: &mut BufferView = &mut buffer_views[image.buffer_view_index as usize];
        println!("Image[{}] : Marking buffer {}", i, image.buffer_view_index);

        buffer_view.used_by_image = i as i32;
    }
}

fn convert_images_and_rebuild_buffer(
    images: &Vec<Image>,
    buffer_views: &mut Vec<BufferView>,
    gltf_json: &mut Value,
    main_buffer: &[u8]) -> Vec<u8>
{
    println!("Reviewing {} buffer views", buffer_views.len());
    mark_bufferview_used_by_images(images, buffer_views);

    let mut output_buffer: Vec<u8> = Vec::new();

    for b in 0..buffer_views.len()
    {
        let buffer_view: &BufferView = &buffer_views[b];
        let image_index: i32 = buffer_view.used_by_image;

        let slice_start: usize = buffer_view.offset as usize;
        let slice_end: usize = (buffer_view.offset + buffer_view.length) as usize;

        let buffer_content: &[u8] = &main_buffer[slice_start..slice_end];

        let current_offset = output_buffer.len();

        let gltf_buffer_view = &mut gltf_json["bufferViews"][b];
        gltf_buffer_view["byteOffset"] = current_offset.into();

        if image_index == NOT_USED_BY_IMAGE_BUFFER
        {
            println!("Writing back, as-is, content of buffer view {}", b);
            output_buffer.write(buffer_content).unwrap();
        }
        else
        {
            println!("Attempting conversion of {}", b);
            let (width, height, converted_image) = convert_image_content_in(buffer_content);
            let converted_image_content = &converted_image[..];
            //write_content_to(converted_image_content, format!("out{}.dds", b).as_str());
            output_buffer.write(converted_image_content).unwrap();
         
            gltf_buffer_view["byteLength"] = converted_image.len().into();
            let gltf_image = &mut gltf_json["images"][image_index as usize];
            gltf_image["mimeType"] = "image/dds".into();
            gltf_image["extensions"] = json!({
                "EXT_voyage_exporter": {
                    "width": width,
                    "height": height,
                    "format": "BC7"
                }
            });
            
        }
    }
    /*gltf_json["asset"]["generator"] = "Voyage GLB Texture converter".into();
    gltf_json["asset"]["version"] = "20240403".into();*/
    gltf_json["extensionsUsed"] = json!(["EXT_voyage_exporter"]);

    gltf_json["extensionsRequired"] = json!(["EXT_voyage_exporter"]);

    return output_buffer;
}

/**
 * Convert the image provided to DDS, compressed using the BC7 algorithm
 * @param buffer The buffer containing the image data to convert
 * @returns (width, height, buffer_with_dds_data)
 * @note The returned buffer has no header
 */
fn convert_image_content_in(buffer: &[u8]) -> (u32, u32, Vec<u8>)
{
    let img = Reader::new(Cursor::new(buffer)).with_guessed_format().unwrap().decode().unwrap();
    let rgba8_image = img.to_rgba8();
    let width = rgba8_image.width();
    let height = rgba8_image.height();

    let rgba8_content = &rgba8_image.into_raw()[..];
    /*for b in 0..rgba8_content.len()
    {
        if (b & 7) == 0 { print!("\n"); }
        print!("0x{:x} ", rgba8_content[b]);
    }
    print!("\n");*/

    let block_count = intel_tex_2::divide_up_by_multiple(width * height, 16);
    println!("Block count: {}", block_count);
    println!("width {} - height {}", width, height);
    let dds_defaults = NewDxgiParams {
        height,
        width,
        depth: Some(1),
        format: DxgiFormat::BC7_UNorm,
        mipmap_levels: Some(1),
        array_layers: Some(1),
        caps2: Some(Caps2::empty()),
        is_cubemap: false,
        resource_dimension: D3D10ResourceDimension::Texture2D,
        alpha_mode: AlphaMode::Straight,
    };
    // BC7
    {
        let mut dds = Dds::new_dxgi(NewDxgiParams {
            format: DxgiFormat::BC7_UNorm,
            ..dds_defaults
        })
        .unwrap();
        let surface = intel_tex_2::RgbaSurface {
            width,
            height,
            stride: width * 4,
            data: rgba8_content,
        };

        println!("Compressing to BC7...");
        bc7::compress_blocks_into(
            &bc7::alpha_ultra_fast_settings(),
            &surface,
            dds.get_mut_data(0 /* layer */).unwrap(),
        );
        println!("  Done!");

        //dds.write(&mut OpenOptions::new().write(true).create(true).open("a.dds").unwrap());
        let dds_data = dds.get_data(0).unwrap();
        return (width, height, dds_data.to_vec());
    }
}


/*fn write_content_to(data: &[u8], filename: &str)
{
    let mut f = OpenOptions::new().create(true).write(true).open(filename).unwrap();
    let _ = f.write(data);
    let _ = f.sync_all();
}*/