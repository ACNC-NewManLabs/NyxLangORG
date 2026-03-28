use super::formats::ImageFormat;

#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

pub fn decode(bytes: Vec<u8>) -> DecodedImage {
    let format = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        ImageFormat::Png
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        ImageFormat::Jpeg
    } else {
        ImageFormat::Unknown
    };
    DecodedImage {
        format,
        width: 0,
        height: 0,
        bytes,
    }
}
