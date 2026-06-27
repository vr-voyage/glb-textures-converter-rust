use bytes::Buf;
use core::mem::size_of;
use serde::{Deserialize};
use serde_json::json;
use serde_json::Map;
use serde_json::Value;
use std::io::Cursor;
use std::io::Write;

use super::converter::CompressionFormat;

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

pub fn create_new_glb_with_converted_textures(glb: Vec<u8>, compression_format:&Option<CompressionFormat>) -> Vec<u8>
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
    if !gltf_json.is_object()
    {
        panic!(
            "Expected the JSON data to represent an object {}",
            &json_content);
    }
    let gltf_json_object: &mut Map<String,Value> = gltf_json.as_object_mut().unwrap();
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
    let new_buffer = convert_images_and_rebuild_buffer(&gltf.images, gltf_buffer_views, gltf_json_object, &glb_data, compression_format);

    /* Pretty ugly, I should try to see if I can
     * - allocate memory BEFORE new_buffer
     * - store "glb header/json chunk header/json chunk data/binary chunk header" just before it
     */
    return create_glb(&gltf_json.to_string(), &new_buffer);

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
        if image.mime_type == "image/raw"
        {
            continue;
        }
        let buffer_view: &mut BufferView = &mut buffer_views[image.buffer_view_index as usize];
        println!("Image[{}] : Marking buffer {}", i, image.buffer_view_index);

        buffer_view.used_by_image = i as i32;
    }
}

fn get_or_create_json_array<'a>(json_object: &'a mut Map<String,Value>, value_name: &str) -> &'a mut Vec<Value>
{
    return json_object.entry(value_name).or_insert(json!([])).as_array_mut().unwrap();
}

fn get_or_create_json_object<'a>(json_object: &'a mut Map<String,Value>, value_name: &str) -> &'a mut Map<String, Value>
{
    return json_object.entry(value_name).or_insert(json!({})).as_object_mut().unwrap();
}

fn convert_images_and_rebuild_buffer(
    images: &Vec<Image>,
    buffer_views: &mut Vec<BufferView>,
    gltf_json: &mut Map<String,Value>,
    main_buffer: &[u8],
    preferred_compression_format_opt: &Option<CompressionFormat>
) -> Vec<u8>
{
    let preferred_compression_format = preferred_compression_format_opt.unwrap_or(CompressionFormat::Bc7);
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
            let (width, height, converted_image, compression_format) = super::converter::convert_image_content_in(buffer_content, preferred_compression_format);
            let converted_image_content = &converted_image[..];
            //write_content_to(converted_image_content, format!("out{}.dds", b).as_str());
            output_buffer.write(converted_image_content).unwrap();
         
            gltf_buffer_view["byteLength"] = converted_image.len().into();
            let gltf_image = &mut gltf_json["images"][image_index as usize];
            gltf_image["mimeType"] =  if compression_format == CompressionFormat::Bc7 { "image/dds".into() } else { "image/raw".into() };
            let extensions = get_or_create_json_object(gltf_image.as_object_mut().unwrap(), "extensions");
            extensions.insert("EXT_voyage_exporter".to_string(), json!(
                {
                    "width": width,
                    "height": height,
                    "format": compression_format.to_string()
                }
            ));
            
        }
    }

    {
        let extensions_used = get_or_create_json_array(gltf_json, "extensionsUsed");
        extensions_used.push("EXT_voyage_exporter".into());
    }

    {
        let extensions_required = get_or_create_json_array(gltf_json, "extensionsRequired");
        extensions_required.push("EXT_voyage_exporter".into());
    }

    return output_buffer;
}
