use egui::{
    ahash::HashMap,
    load::{Bytes, BytesPoll, ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint},
    mutex::Mutex,
    ColorImage, Context,
};
use std::{mem::size_of, path::Path, sync::Arc};

type Entry = Result<Arc<ColorImage>, String>;

#[derive(Default)]
pub struct PngLoader {
    cache: Mutex<HashMap<String, Entry>>,
}

impl PngLoader {
    pub const ID: &'static str = egui::generate_loader_id!(PngLoader);
}

fn is_supported_uri(uri: &str) -> bool {
    let Some(ext) = Path::new(uri).extension().and_then(|ext| ext.to_str()) else {
        return false;
    };

    ext == "png"
}

fn is_unsupported_mime(mime: &str) -> bool {
    !mime.contains("png")
}

fn load_image_bytes(header: &minipng::ImageHeader, bytes: &Bytes) -> Result<ColorImage, minipng::Error> {
    let mut buffer = vec![0; header.required_bytes_rgba8bpc()];
    let mut image = minipng::decode_png(bytes, &mut buffer)?;
    image.convert_to_rgba8bpc()?;

    let size = [image.width() as _, image.height() as _];
    let pixels = image.pixels();
    Ok(ColorImage::from_rgba_unmultiplied(size, pixels))
}

impl ImageLoader for PngLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, ctx: &egui::Context, uri: &str, _: SizeHint) -> ImageLoadResult {
        if !is_supported_uri(uri) {
            return Err(LoadError::NotSupported);
        }

        let mut cache = self.cache.lock();
        if let Some(entry) = cache.get(uri).cloned() {
            match entry {
                Ok(image) => Ok(ImagePoll::Ready { image }),
                Err(err) => Err(LoadError::Loading(err)),
            }
        } else {
            match ctx.try_load_bytes(uri) {
                Ok(BytesPoll::Ready { bytes, mime, .. }) => {
                    if mime.as_deref().is_some_and(is_unsupported_mime) {
                        return Err(LoadError::NotSupported);
                    }

                    let Ok(header) = minipng::decode_png_header(&bytes) else {
                        return Err(LoadError::NotSupported);
                    };

                    let result = load_image_bytes(&header, &bytes).map(Arc::new).map_err(|e| e.to_string());
                    cache.insert(uri.into(), result.clone());
                    match result {
                        Ok(image) => Ok(ImagePoll::Ready { image }),
                        Err(err) => Err(LoadError::Loading(err)),
                    }
                }
                Ok(BytesPoll::Pending { size }) => Ok(ImagePoll::Pending { size }),
                Err(err) => Err(err),
            }
        }
    }

    fn forget(&self, uri: &str) {
        let _ = self.cache.lock().remove(uri);
    }

    fn forget_all(&self) {
        self.cache.lock().clear();
    }

    fn byte_size(&self) -> usize {
        self.cache
            .lock()
            .values()
            .map(|result| match result {
                Ok(image) => image.pixels.len() * size_of::<egui::Color32>(),
                Err(err) => err.len(),
            })
            .sum()
    }
}

/// Installs the minipng image loader.
pub fn install(context: &Context) {
    context.add_image_loader(Arc::new(PngLoader::default()))
}
