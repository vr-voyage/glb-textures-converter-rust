use clap;
use ddsfile::{AlphaMode, Caps2, D3D10ResourceDimension, Dds, DxgiFormat, NewDxgiParams};
use intel_tex_2::bc3;
use intel_tex_2::bc7;
use image::io::Reader;
use std::fmt;
use std::io::Cursor;

#[derive(Copy, Clone, Debug, clap::ValueEnum, PartialEq)]
pub enum CompressionFormat
{
    Rgba8,
    Rgba8Unorm,
    Dxt5,
    Bc7
}

impl fmt::Display for CompressionFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CompressionFormat::Rgba8 => write!(f, "RGBA8"),
            CompressionFormat::Dxt5 => write!(f, "DXT5"),
            CompressionFormat::Bc7 => write!(f, "BC7"),
            CompressionFormat::Rgba8Unorm => write!(f, "RGBA8_UNORM")
        }
    }
}

fn dxgi_to_str(format: DxgiFormat) -> String
{
    return match format
    {
        DxgiFormat::BC7_UNorm_sRGB => "BC7".to_string(),
        DxgiFormat::R8G8B8A8_UInt => "RGBA8".to_string(),
        DxgiFormat::R8G8B8A8_UNorm => "RGBA8_UNORM".to_string(),
        DxgiFormat::BC3_UNorm_sRGB => "DXT5".to_string(),
        _ => "Unknown".to_string()
    };
}

fn compression_format_to_dxgi_format(compression_format:&CompressionFormat) -> DxgiFormat
{
    return match compression_format {
        CompressionFormat::Bc7 => DxgiFormat::BC7_UNorm_sRGB,
        CompressionFormat::Rgba8 => DxgiFormat::R8G8B8A8_UInt,
        CompressionFormat::Rgba8Unorm => DxgiFormat::R8G8B8A8_UNorm,
        CompressionFormat::Dxt5 => DxgiFormat::BC3_UNorm_sRGB,
    }
}

type SurfaceHandler = fn(&intel_tex_2::RgbaSurface, &mut [u8]);

fn convert_image_as(width: u32, height: u32, rgba8_content: &[u8], format: DxgiFormat, surface_handler: SurfaceHandler) -> Vec<u8>
{
    let dds_defaults = NewDxgiParams {
        height,
        width,
        depth: Some(1),
        format: format,
        mipmap_levels: Some(1),
        array_layers: Some(1),
        caps2: Some(Caps2::empty()),
        is_cubemap: false,
        resource_dimension: D3D10ResourceDimension::Texture2D,
        alpha_mode: AlphaMode::Straight,
    };

    let mut dds = Dds::new_dxgi(NewDxgiParams {
        format: format,
        ..dds_defaults
    })
    .unwrap();
    let surface = intel_tex_2::RgbaSurface {
        width,
        height,
        stride: width * 4,
        data: rgba8_content,
    };
    surface_handler(&surface, dds.get_mut_data(0 /* layer */).unwrap());
    let dds_data = dds.get_data(0).unwrap();
    return dds_data.to_vec();
}

fn align_on(pow2_value: u32, val: u32) -> u32
{
    let mask: u32 = pow2_value - 1;
    return (val+mask) & (!mask);
}

fn surface_treatment_none(_surface: &intel_tex_2::RgbaSurface, _blocks: &mut [u8])
{

}

fn surface_treatment_dxt5(surface: &intel_tex_2::RgbaSurface, blocks: &mut [u8])
{
    bc3::compress_blocks_into(surface, blocks);
}

fn surface_treatment_bc7(surface: &intel_tex_2::RgbaSurface, blocks: &mut [u8])
{
    bc7::compress_blocks_into(
        &bc7::alpha_ultra_fast_settings(),
        &surface,
        blocks // dds.get_mut_data(0 /* layer */).unwrap(),
    );
}

fn surface_handler_for(compression_format:&CompressionFormat) -> SurfaceHandler
{
    return match compression_format {
        CompressionFormat::Bc7 => surface_treatment_bc7,
        CompressionFormat::Rgba8 => surface_treatment_none,
        CompressionFormat::Rgba8Unorm => surface_treatment_none,
        CompressionFormat::Dxt5 => surface_treatment_dxt5,
    }
}

/**
 * Convert the image provided to DDS, compressed using the BC7 algorithm
 * @param buffer The buffer containing the image data to convert
 * @returns (width, height, buffer_with_dds_data)
 * @note The returned buffer has no header
 */
pub fn convert_image_content_in(buffer: &[u8], preferred_compression_format:CompressionFormat) -> (u32, u32, Vec<u8>, CompressionFormat)
{
    println!("Format: {}", preferred_compression_format);
    let img = Reader::new(Cursor::new(buffer)).with_guessed_format().unwrap().decode().unwrap();
    
    let width = img.width();
    let height = img.height();

    if (preferred_compression_format == CompressionFormat::Rgba8) || (width * height) < (256*256)
    {
        let rgba8_image = img.to_rgba8();
        let rgba8_content = &rgba8_image.into_raw()[..];
        return (width, height, rgba8_content.to_vec(), CompressionFormat::Rgba8);
    }
    else
    {
        let compression_format = preferred_compression_format;
        let dxgi_format = compression_format_to_dxgi_format(&compression_format);
        println!("DXGI Format: {}", dxgi_to_str(dxgi_format));
        let surface_handler = surface_handler_for(&compression_format);

        let used_width = align_on(4, width);
        let used_height  = align_on(4, height);
        let resize_filter:image::imageops::FilterType = image::imageops::FilterType::Lanczos3;

        let needs_resize :bool = (used_width != width) | (used_height != height);
        let used_img: image::DynamicImage = if needs_resize { img.resize_exact(used_width, used_height, resize_filter) } else { img };

        let rgba8_image = used_img.to_rgba8();
        let rgba8_content = &rgba8_image.into_raw()[..];
        let ret_content = convert_image_as(used_width, used_height, rgba8_content, dxgi_format, surface_handler);
        return (used_width, used_height, ret_content, compression_format);
    }
}
