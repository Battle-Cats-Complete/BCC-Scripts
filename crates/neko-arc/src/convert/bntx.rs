use tracing::trace;

const FILE_MAGIC: &[u8] = b"BNTX\0\0\0\0";
const LITTLE_ENDIAN_MARK: &[u8] = b"\xFF\xFE";
const CONTAINER_OFFSET: usize = 32;
const TEXTURE_MAGIC: &[u8] = b"BRTI";
const INFO_OFFSET: usize = 16;

pub struct Texture {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub format: u32,
    pub tile_mode: u16,
    pub block_height_log2: u32,
    pub component_select: [u8; 4],
    pub round_pitch: bool,
    pub data: Vec<u8>,
}

pub fn parse(bytes: &[u8]) -> Result<Vec<Texture>, String> {
    if bytes.len() < CONTAINER_OFFSET || !bytes.starts_with(FILE_MAGIC) {
        return Err(String::from("Missing BNTX signature"));
    }

    if bytes.get(12..14) != Some(LITTLE_ENDIAN_MARK) {
        return Err(String::from("Only little endian BNTX files are supported"));
    }

    let target = bytes.get(32..36).ok_or("Truncated texture container")?;
    let round_pitch = match target {
        b"NX  " => true,
        b"Gen " => false,
        _ => return Err(String::from("Unknown texture container target")),
    };

    let texture_count = read_u32(bytes, 36)? as usize;
    let pointer_table = read_u64(bytes, 40)? as usize;

    trace!(textures = texture_count, "Parsed BNTX container");

    let mut textures = Vec::with_capacity(texture_count);

    for index in 0..texture_count {
        let info_pointer = read_u64(bytes, pointer_table + index * 8)? as usize;

        if bytes.get(info_pointer..info_pointer + 4) != Some(TEXTURE_MAGIC) {
            return Err(String::from("Missing BRTI signature"));
        }

        textures.push(read_texture(bytes, info_pointer + INFO_OFFSET, round_pitch)?);
    }

    Ok(textures)
}

fn read_texture(bytes: &[u8], base: usize, round_pitch: bool) -> Result<Texture, String> {
    let dimension = *bytes.get(base + 1).ok_or("Truncated texture info")?;
    let tile_mode = read_u16(bytes, base + 2)?;
    let format = read_u32(bytes, base + 12)?;
    let width = read_u32(bytes, base + 20)? as usize;
    let height = read_u32(bytes, base + 24)? as usize;
    let array_length = read_u32(bytes, base + 32)?;
    let texture_layout = read_u32(bytes, base + 36)?;
    let image_size = read_u32(bytes, base + 64)? as usize;
    let packed_select = read_u32(bytes, base + 72)?;
    let name_address = read_u64(bytes, base + 80)? as usize;
    let pointer_address = read_u64(bytes, base + 96)? as usize;

    if dimension != 2 {
        return Err(format!("Unsupported storage dimension {dimension}"));
    }

    if array_length > 1 {
        return Err(format!("Unsupported array length {array_length}"));
    }

    if tile_mode > 1 {
        return Err(format!("Unsupported tiling mode {tile_mode}"));
    }

    let mip_address = read_u64(bytes, pointer_address)? as usize;
    let data = bytes
        .get(mip_address..mip_address + image_size)
        .ok_or("Texture payload extends past the end of the file")?
        .to_vec();

    Ok(Texture {
        name: read_name(bytes, name_address)?,
        width,
        height,
        format,
        tile_mode,
        block_height_log2: texture_layout & 7,
        component_select: [
            (packed_select & 0xFF) as u8,
            ((packed_select >> 8) & 0xFF) as u8,
            ((packed_select >> 16) & 0xFF) as u8,
            ((packed_select >> 24) & 0xFF) as u8,
        ],
        round_pitch,
        data,
    })
}

fn read_name(bytes: &[u8], address: usize) -> Result<String, String> {
    let length = read_u16(bytes, address)? as usize;
    let raw = bytes
        .get(address + 2..address + 2 + length)
        .ok_or("Texture name extends past the end of the file")?;

    Ok(String::from_utf8_lossy(raw).into_owned())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let slice = bytes.get(offset..offset + 2).ok_or("Truncated BNTX structure")?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let slice = bytes.get(offset..offset + 4).ok_or("Truncated BNTX structure")?;
    let mut buffer = [0u8; 4];
    buffer.copy_from_slice(slice);
    Ok(u32::from_le_bytes(buffer))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let slice = bytes.get(offset..offset + 8).ok_or("Truncated BNTX structure")?;
    let mut buffer = [0u8; 8];
    buffer.copy_from_slice(slice);
    Ok(u64::from_le_bytes(buffer))
}
