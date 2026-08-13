use texture2ddecoder::{
    decode_astc, decode_bc1, decode_bc2, decode_bc3, decode_bc4, decode_bc5, decode_bc6_signed, decode_bc6_unsigned,
    decode_bc7,
};

use super::bntx::Texture;

const GOB_SIZE: usize = 512;

struct Channels {
    bytes_per_pixel: usize,
    red: u32,
    green: u32,
    blue: u32,
    alpha: u32,
}

enum Layout {
    Packed(Channels),
    Block(BlockCodec),
}

enum BlockCodec {
    Bc1,
    Bc2,
    Bc3,
    Bc4,
    Bc5,
    Bc6Signed,
    Bc6Unsigned,
    Bc7,
    Astc(usize, usize),
}

pub fn decode(texture: &Texture) -> Result<Vec<u8>, String> {
    if texture.width == 0 || texture.height == 0 {
        return Err(String::from("Texture has no pixels"));
    }

    let kind = texture.format >> 8;
    let layout = resolve_layout(texture.format)?;
    let (block_width, block_height) = block_dimensions(&layout);
    let bytes_per_block = block_stride(&layout);

    let linear = deswizzle(texture, block_width, block_height, bytes_per_block);
    let source = decode_layout(&layout, &linear, texture.width, texture.height)
        .map_err(|err| format!("{err} (format {kind:#x})"))?;

    Ok(apply_component_select(&source, texture.component_select))
}

fn resolve_layout(format: u32) -> Result<Layout, String> {
    let layout = match format >> 8 {
        0x01 => Layout::Packed(Channels::new(1, 0x0F, 0xF0, 0, 0)),
        0x02 => Layout::Packed(Channels::new(1, 0xFF, 0, 0, 0)),
        0x03 => Layout::Packed(Channels::new(2, 0x000F, 0x00F0, 0x0F00, 0xF000)),
        0x04 => Layout::Packed(Channels::new(2, 0xF000, 0x0F00, 0x00F0, 0x000F)),
        0x05 => Layout::Packed(Channels::new(2, 0x001F, 0x03E0, 0x7C00, 0x8000)),
        0x06 => Layout::Packed(Channels::new(2, 0x8000, 0x7C00, 0x03E0, 0x001F)),
        0x07 => Layout::Packed(Channels::new(2, 0x001F, 0x07E0, 0xF800, 0)),
        0x08 => Layout::Packed(Channels::new(2, 0xF800, 0x07E0, 0x001F, 0)),
        0x09 => Layout::Packed(Channels::new(2, 0x00FF, 0xFF00, 0, 0)),
        0x0B => Layout::Packed(Channels::new(4, 0x0000_00FF, 0x0000_FF00, 0x00FF_0000, 0xFF00_0000)),
        0x0C => Layout::Packed(Channels::new(4, 0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000)),
        0x0E => Layout::Packed(Channels::new(4, 0x3FF0_0000, 0x000F_FC00, 0x0000_03FF, 0xC000_0000)),
        0x3B => Layout::Packed(Channels::new(2, 0x7C00, 0x03E0, 0x001F, 0x8000)),
        0x1A => Layout::Block(BlockCodec::Bc1),
        0x1B => Layout::Block(BlockCodec::Bc2),
        0x1C => Layout::Block(BlockCodec::Bc3),
        0x1D => Layout::Block(BlockCodec::Bc4),
        0x1E => Layout::Block(BlockCodec::Bc5),
        0x1F if format & 0xFF == 0x05 => Layout::Block(BlockCodec::Bc6Signed),
        0x1F => Layout::Block(BlockCodec::Bc6Unsigned),
        0x20 => Layout::Block(BlockCodec::Bc7),
        0x2D => Layout::Block(BlockCodec::Astc(4, 4)),
        0x2E => Layout::Block(BlockCodec::Astc(5, 4)),
        0x2F => Layout::Block(BlockCodec::Astc(5, 5)),
        0x30 => Layout::Block(BlockCodec::Astc(6, 5)),
        0x31 => Layout::Block(BlockCodec::Astc(6, 6)),
        0x32 => Layout::Block(BlockCodec::Astc(8, 5)),
        0x33 => Layout::Block(BlockCodec::Astc(8, 6)),
        0x34 => Layout::Block(BlockCodec::Astc(8, 8)),
        0x35 => Layout::Block(BlockCodec::Astc(10, 5)),
        0x36 => Layout::Block(BlockCodec::Astc(10, 6)),
        0x37 => Layout::Block(BlockCodec::Astc(10, 8)),
        0x38 => Layout::Block(BlockCodec::Astc(10, 10)),
        0x39 => Layout::Block(BlockCodec::Astc(12, 10)),
        0x3A => Layout::Block(BlockCodec::Astc(12, 12)),
        unknown => return Err(format!("Unsupported texture format {unknown:#x}")),
    };

    Ok(layout)
}

fn block_dimensions(layout: &Layout) -> (usize, usize) {
    match layout {
        Layout::Packed(_) => (1, 1),
        Layout::Block(BlockCodec::Astc(width, height)) => (*width, *height),
        Layout::Block(_) => (4, 4),
    }
}

fn block_stride(layout: &Layout) -> usize {
    match layout {
        Layout::Packed(channels) => channels.bytes_per_pixel,
        Layout::Block(BlockCodec::Bc1 | BlockCodec::Bc4) => 8,
        Layout::Block(_) => 16,
    }
}

fn decode_layout(layout: &Layout, linear: &[u8], width: usize, height: usize) -> Result<Vec<[u8; 4]>, String> {
    match layout {
        Layout::Packed(channels) => Ok(decode_packed(channels, linear, width, height)),
        Layout::Block(codec) => decode_block(codec, linear, width, height),
    }
}

fn decode_packed(channels: &Channels, linear: &[u8], width: usize, height: usize) -> Vec<[u8; 4]> {
    let stride = channels.bytes_per_pixel;
    let mut pixels = Vec::with_capacity(width * height);

    for index in 0..width * height {
        let offset = index * stride;
        let Some(raw) = linear.get(offset..offset + stride) else {
            pixels.push([0, 0, 0, 0]);
            continue;
        };

        let mut packed = 0u32;
        for (shift, byte) in raw.iter().enumerate() {
            packed |= u32::from(*byte) << (shift * 8);
        }

        pixels.push([
            extract(packed, channels.red, 0),
            extract(packed, channels.green, 0),
            extract(packed, channels.blue, 0),
            extract(packed, channels.alpha, 255),
        ]);
    }

    pixels
}

fn decode_block(codec: &BlockCodec, linear: &[u8], width: usize, height: usize) -> Result<Vec<[u8; 4]>, String> {
    let mut buffer = vec![0u32; width * height];

    let outcome = match codec {
        BlockCodec::Bc1 => decode_bc1(linear, width, height, &mut buffer),
        BlockCodec::Bc2 => decode_bc2(linear, width, height, &mut buffer),
        BlockCodec::Bc3 => decode_bc3(linear, width, height, &mut buffer),
        BlockCodec::Bc4 => decode_bc4(linear, width, height, &mut buffer),
        BlockCodec::Bc5 => decode_bc5(linear, width, height, &mut buffer),
        BlockCodec::Bc6Signed => decode_bc6_signed(linear, width, height, &mut buffer),
        BlockCodec::Bc6Unsigned => decode_bc6_unsigned(linear, width, height, &mut buffer),
        BlockCodec::Bc7 => decode_bc7(linear, width, height, &mut buffer),
        BlockCodec::Astc(block_width, block_height) => {
            decode_astc(linear, width, height, *block_width, *block_height, &mut buffer)
        }
    };

    outcome.map_err(|err| err.to_string())?;

    Ok(buffer
        .into_iter()
        .map(|packed| {
            let [blue, green, red, alpha] = packed.to_le_bytes();
            [red, green, blue, alpha]
        })
        .collect())
}

fn apply_component_select(source: &[[u8; 4]], selectors: [u8; 4]) -> Vec<u8> {
    if selectors == [2, 3, 4, 5] {
        return source.iter().flatten().copied().collect();
    }

    let mut pixels = Vec::with_capacity(source.len() * 4);

    for texel in source {
        for selector in selectors {
            pixels.push(match selector {
                1 => 255,
                2..=5 => texel[usize::from(selector) - 2],
                _ => 0,
            });
        }
    }

    pixels
}

fn extract(packed: u32, mask: u32, fallback: u8) -> u8 {
    if mask == 0 {
        return fallback;
    }

    let shift = mask.trailing_zeros();
    let maximum = mask >> shift;
    let value = (packed & mask) >> shift;

    ((u64::from(value) * 255 + u64::from(maximum) / 2) / u64::from(maximum)) as u8
}

fn deswizzle(texture: &Texture, block_width: usize, block_height: usize, bytes_per_block: usize) -> Vec<u8> {
    let width = div_round_up(texture.width, block_width);
    let height = div_round_up(texture.height, block_height);

    let mut gob_height = 1usize << texture.block_height_log2;
    if pow2_round_up(height) < gob_height * 8 {
        gob_height = 1 << texture.block_height_log2.saturating_sub(1);
    }

    let (pitch, surface_size) = if texture.tile_mode == 1 {
        let pitch = if texture.round_pitch {
            round_up(width * bytes_per_block, 32)
        } else {
            width * bytes_per_block
        };
        (pitch, pitch * height)
    } else {
        let pitch = round_up(width * bytes_per_block, 64);
        (pitch, pitch * round_up(height, gob_height * 8))
    };

    let mut linear = vec![0u8; width * height * bytes_per_block];

    for y in 0..height {
        for x in 0..width {
            let source = if texture.tile_mode == 1 {
                y * pitch + x * bytes_per_block
            } else {
                block_linear_address(x, y, width, bytes_per_block, gob_height)
            };

            if source + bytes_per_block > surface_size {
                continue;
            }

            let Some(chunk) = texture.data.get(source..source + bytes_per_block) else {
                continue;
            };

            let destination = (y * width + x) * bytes_per_block;
            linear[destination..destination + bytes_per_block].copy_from_slice(chunk);
        }
    }

    linear
}

fn block_linear_address(x: usize, y: usize, width: usize, bytes_per_block: usize, gob_height: usize) -> usize {
    let width_in_gobs = div_round_up(width * bytes_per_block, 64);

    let gob_address = (y / (8 * gob_height)) * GOB_SIZE * gob_height * width_in_gobs
        + (x * bytes_per_block / 64) * GOB_SIZE * gob_height
        + (y % (8 * gob_height) / 8) * GOB_SIZE;

    let x = x * bytes_per_block;

    gob_address + ((x % 64) / 32) * 256 + ((y % 8) / 2) * 64 + ((x % 32) / 16) * 32 + (y % 2) * 16 + (x % 16)
}

fn div_round_up(value: usize, divisor: usize) -> usize {
    value.div_ceil(divisor)
}

fn round_up(value: usize, alignment: usize) -> usize {
    if value == 0 {
        return 0;
    }

    ((value - 1) | (alignment - 1)) + 1
}

fn pow2_round_up(value: usize) -> usize {
    if value == 0 {
        return 0;
    }

    let mut value = value - 1;
    value |= value >> 1;
    value |= value >> 2;
    value |= value >> 4;
    value |= value >> 8;
    value |= value >> 16;

    value + 1
}

impl Channels {
    fn new(bytes_per_pixel: usize, red: u32, green: u32, blue: u32, alpha: u32) -> Self {
        Self {
            bytes_per_pixel,
            red,
            green,
            blue,
            alpha,
        }
    }
}
