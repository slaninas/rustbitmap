use super::bit_depth::BitDepth;
use super::file_header::FileHeader;
use super::image::BitMap;
use super::info_header::InfoHeader;
use super::rgb_quad::RgbQuad;
use super::rgba::Rgba;

///
/// Used for working with binary data when the image is read in or converted to
/// a bit map with a bit depth of 1, 4, or 8. Each byte points to a color inside
/// of colors.
///
pub struct BitData {
    /// width of the image
    width: u32,
    /// height of the image
    #[allow(dead_code)]
    height: u32,
    /// list of colors that are used in the image
    colors: Vec<Rgba>,
    /// list of bytes that point to colors. Each byte could be point to 1 or
    /// more colors, depending on the bit depth
    bytes: Vec<u8>,
    /// bit depth of the image
    bit_depth: BitDepth,
}

impl BitData {
    ///
    /// Create bit data from stream of bytes
    ///
    pub fn stream(
        bit_stream: &[u8],
        file: &FileHeader,
        info: &InfoHeader,
        bit_depth: BitDepth,
        colors: &RgbQuad,
    ) -> BitData {
        let offset = file.get_off_bits() as usize;
        // for byte in offset..bits
        let mut bytes = Vec::new();
        for index in offset..bit_stream.len() {
            bytes.push(bit_stream[index]);
        }
        BitData {
            width: info.get_width(),
            height: info.get_height(),
            bit_depth,
            colors: colors.clone_colors(),
            bytes,
        }
    }

    ///
    /// Create bit data from a bitmap
    ///
    pub fn from_bitmap(bitmap: &BitMap, bit_depth: BitDepth) -> BitData {
        let mut unique_colors = bitmap.get_all_unique_colors().clone();
        unique_colors.push(Rgba::rgb(0, 0, 0));
        let step = bit_depth.get_step_counter();

        // figure out how much padding is on each row
        // this is needed because for each row of a bmp image needs to finish
        // with a width of bytes that is divisible by 4. Here we are figuring out
        // how much bit padding and byte padding we need.
        let bit_width = bitmap.get_width() * bit_depth.get_step_counter();
        let bit_padding = match bit_width % 8 {
            0 => 0,
            _ => 8 - (bit_width % 8),
        };
        let byte_width = (bit_width + bit_padding) / 8;
        let byte_padding = match byte_width % 4 {
            0 => 0,
            _ => 4 - (byte_width % 4),
        };
        let mut bytes =
            Vec::with_capacity(((byte_width + byte_padding) * bitmap.get_height()) as usize);

        let step = step as u8;
        let mut byte: u8 = 0;
        let mut counter: u32 = 0;
        let mut shift: u32 = 0;
        for i in 0..bitmap.get_pixels().len() {
            let pixel = bitmap.get_pixels()[i];
            let color_index = unique_colors.iter().position(|&c| c == pixel).unwrap() as u8;
            counter += step as u32;
            shift = counter % 8;
            byte = byte << step;
            // if bit_depth is a BW then we want to push the bit onto the byte
            byte += color_index;

            // push byte into data
            if shift == 0 && i != 0 && bit_width >= 8 {
                bytes.push(byte);
                byte = 0;
            }
            // add padding to row
            if counter % bitmap.get_width() == 0 && i != 0 {
                if bit_padding != 0 {
                    byte = byte << bit_padding;
                    bytes.push(byte);
                    byte = 0;
                    counter = 0;
                }

                for _ in 0..byte_padding {
                    bytes.push(0);
                }
            }
        }
        if shift != 0 {
            byte = byte << (8 - shift);
            bytes.push(byte);
        }
        if bytes.len() % 4 != 0 {
            for _ in 0..byte_padding {
                bytes.push(0);
            }
        }

        BitData {
            width: bitmap.get_width(),
            height: bitmap.get_height(),
            bit_depth,
            colors: unique_colors,
            bytes,
        }
    }

    ///
    /// Pass the bit data back as a stream of bytes
    ///
    pub fn as_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    ///
    /// Convert bits into array of colors
    ///
    pub fn as_rgba(&self) -> Vec<Rgba> {
        let mut pixels = Vec::new();
        let step = self.bit_depth.get_step_counter();
        // figure out how much padding is on each row
        // this is needed because for each row of a bmp image needs to finish
        // with a width of bytes that is divisible by 4. Here we are figuring out
        // how much bit padding and byte padding we need.
        // TODO: move to it's own class
        let bit_width = self.width * self.bit_depth.get_step_counter();
        let bit_padding = match bit_width % 8 {
            0 => 0,
            _ => 8 - (self.width % 8),
        };
        let byte_width = (bit_width + bit_padding) / 8;
        let byte_padding = match byte_width % 4 {
            0 => 0,
            _ => 4 - (byte_width % 4),
        };

        // loop through all the bytes
        let mut byte_padding_counter = 0;
        let mut start_reading_again = false;
        for byte in &self.bytes {
            if byte_padding_counter > 0 {
                byte_padding_counter = byte_padding_counter - 1;
                continue;
            }
            // loop through the bits of the byte
            for byte_indexes in (0..(8 / step)).rev() {
                // so this works on every time after the pixels length
                // reaches 72. We need a way to tell it that it is a new line
                // (at lease once)
                if pixels.len() as u32 % self.width == 0
                    && pixels.len() != 0
                    && !start_reading_again
                {
                    break;
                }
                // bits could be:
                // (1) -> 0 1 2 3 4 5 6 7
                // (4) -> 0 1
                // (8) -> 0
                let starting_bit = byte_indexes * step;
                let ending_bit = starting_bit + step;
                let mut index: usize = 0;
                for bit_index in (starting_bit..ending_bit).rev() {
                    index = index << 1;
                    let next_bit = (byte >> bit_index) & 1;
                    index = index + next_bit as usize;
                }
                pixels.push(self.colors[index]);
            }
            if start_reading_again {
                start_reading_again = false;
            }
            if (pixels.len() as u32) % self.width == 0
                && pixels.len() != 0
                && start_reading_again == false
            {
                start_reading_again = true;
                byte_padding_counter = byte_padding;
            }
        }
        pixels
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn get_bytes_size(&self) -> u32 {
        self.bytes.len() as u32
    }
}

#[cfg(debug_assertions)]
impl std::fmt::Display for BitData {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for p in (0..self.bytes.len()).rev() {
            write!(f, "{}:\t{:#b}\n", p, self.bytes[p]).unwrap();
        }
        write!(f, "")
    }
}
