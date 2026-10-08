//! Inline images from Kitty graphics / OSC 1337. One slot per image id;
//! dropping a tab removes its slots so the GPU texture can go.

use std::collections::BTreeMap;
use std::io::Cursor;

use image::ImageReader;

#[derive(Debug, Clone)]
pub struct InlineImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct InlineStore {
    images: BTreeMap<u64, InlineImage>,
}

impl InlineStore {
    pub fn put_png(&mut self, id: u64, bytes: &[u8]) -> Result<&InlineImage, String> {
        let img = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| e.to_string())?
            .decode()
            .map_err(|e| e.to_string())?
            .to_rgba8();
        let (width, height) = img.dimensions();
        let rgba = img.into_raw();
        self.images.insert(id, InlineImage { width, height, rgba });
        Ok(self.images.get(&id).unwrap())
    }

    pub fn get(&self, id: u64) -> Option<&InlineImage> {
        self.images.get(&id)
    }

    /// Close-tab: drop every image owned by that pane/tab.
    pub fn drop_ids(&mut self, ids: &[u64]) {
        for id in ids {
            self.images.remove(id);
        }
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageEncoder, ImageFormat};

    fn tiny_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
        let mut buf = Vec::new();
        image::codecs::png::PngEncoder::new(&mut buf)
            .write_image(img.as_raw(), 2, 2, image::ExtendedColorType::Rgba8)
            .unwrap();
        let _ = ImageFormat::Png;
        buf
    }

    #[test]
    fn small_png_lands_on_the_grid_and_tab_close_frees_it() {
        let bytes = tiny_png();
        let mut store = InlineStore::default();
        let img = store.put_png(1, &bytes).expect("png");
        assert_eq!((img.width, img.height), (2, 2));
        assert_eq!(img.rgba[0], 255, "red pixel on the grid");
        assert_eq!(store.len(), 1);
        store.drop_ids(&[1]);
        assert!(store.get(1).is_none(), "closing the tab must free the texture");
        assert_eq!(store.len(), 0);
    }
}
