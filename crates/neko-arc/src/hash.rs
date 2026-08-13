const IMAGE_PREFIX: &str = "Data/Image/";
const IMAGE_SUFFIX: &str = ".bntx";
const TABLE_PREFIX: &str = "Data/CsvFiles/";
const TABLE_SUFFIX: &str = ".csb";

const HIGH_POLYNOMIAL: u32 = 0x1021_5681;
const LOW_POLYNOMIAL: u32 = 0x5681_1021;

pub fn asset_path(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".png") {
        format!("{IMAGE_PREFIX}{name}{IMAGE_SUFFIX}")
    } else {
        format!("{TABLE_PREFIX}{name}{TABLE_SUFFIX}")
    }
}

pub fn hash(name: &str) -> u64 {
    let path = asset_path(name);
    let bytes = path.as_bytes();

    if bytes.is_empty() {
        return 0;
    }

    let high = u64::from(reflected_crc(bytes, HIGH_POLYNOMIAL));
    let low = u64::from(reflected_crc(bytes, LOW_POLYNOMIAL));

    (high << 32) | low
}

pub fn hash_hex(name: &str) -> String {
    format!("{:x}", hash(name))
}

fn reflected_crc(data: &[u8], polynomial: u32) -> u32 {
    let mut register: u32 = 0xFFFF_FFFF;

    for &byte in data {
        register ^= u32::from(byte);
        for _ in 0..8 {
            let carry = register & 1 != 0;
            register >>= 1;
            if carry {
                register ^= polynomial;
            }
        }
    }

    !register
}

#[cfg(test)]
mod tests {
    use super::{asset_path, hash_hex};

    #[test]
    fn routes_images_and_tables() {
        assert_eq!(asset_path("uni001_f.png"), "Data/Image/uni001_f.png.bntx");
        assert_eq!(asset_path("unitbuy.csv"), "Data/CsvFiles/unitbuy.csv.csb");
        assert_eq!(asset_path("stage.tsv"), "Data/CsvFiles/stage.tsv.csb");
    }

    #[test]
    fn matches_known_digests() {
        assert_eq!(hash_hex("uni001_f.png"), "e9d569dea7957cf6");
        assert_eq!(hash_hex("unitbuy.csv"), "f74d8704de4d9acf");
        assert_eq!(hash_hex("stage.tsv"), "e80d656e96ee38fb");
    }
}
