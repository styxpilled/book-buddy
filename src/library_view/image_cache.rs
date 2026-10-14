use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::FutureExt;
use gpui::{
    App, AppContext, Asset, Entity, ImageAssetLoader, ImageCache, ImageCacheError, ImageCacheItem,
    RenderImage, Resource, Window, hash,
};
use image::RgbaImage;

use crate::ui::render_image;

/// Changes an image from a file before it is shown.
pub type Filter = Arc<dyn Fn(&Path, RgbaImage) -> RgbaImage + Send + Sync>;

/// GPUI keeps a decoded image until it is dropped. This cache drops the least
/// recently used image when it holds more than `capacity`.
pub struct BoundedImageCache {
    items: HashMap<u64, ImageCacheItem>,
    order: VecDeque<u64>,
    capacity: usize,
    filter: Filter,
}

impl BoundedImageCache {
    pub fn new(capacity: usize, filter: Filter, cx: &mut App) -> Entity<Self> {
        let cache = cx.new(|_| Self {
            items: HashMap::new(),
            order: VecDeque::new(),
            capacity,
            filter,
        });
        cx.observe_release(&cache, |cache, cx| {
            for (_, item) in cache.items.drain() {
                drop_item(item, None, cx);
            }
        })
        .detach();
        cache
    }

    /// The cache shrinks to `capacity` at the next load.
    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
    }

    /// Images load with `filter` from now on. The images of `paths` load again.
    pub fn set_filter(&mut self, filter: Filter, paths: &[PathBuf], cx: &mut App) {
        self.filter = filter;
        for path in paths {
            let key = hash(&Resource::Path(path.as_path().into()));
            if let Some(item) = self.items.remove(&key) {
                self.order.retain(|k| *k != key);
                drop_item(item, None, cx);
            }
        }
    }
}

impl ImageCache for BoundedImageCache {
    fn load(
        &mut self,
        resource: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        let key = hash(resource);
        if let Some(item) = self.items.get_mut(&key) {
            if let Some(index) = self.order.iter().position(|k| *k == key) {
                self.order.remove(index);
            }
            self.order.push_back(key);
            return item.get();
        }

        let executor = cx.background_executor().clone();
        let task = match resource {
            Resource::Path(path) => {
                let (filter, path) = (self.filter.clone(), path.clone());
                executor.spawn(async move { load_filtered(&path, &*filter) })
            }
            _ => executor.spawn(<ImageAssetLoader as Asset>::load(resource.clone(), cx)),
        }
        .shared();
        self.items
            .insert(key, ImageCacheItem::Loading(task.clone()));
        self.order.push_back(key);
        let view = window.current_view();
        window
            .spawn(cx, async move |cx| {
                _ = task.await;
                cx.on_next_frame(move |_, cx| cx.notify(view));
            })
            .detach();

        while self.order.len() > self.capacity {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            if let Some(item) = self.items.remove(&old) {
                drop_item(item, Some(window), cx);
            }
        }
        None
    }
}

/// Drops the image of an item from the atlas, if it is loaded.
fn drop_item(mut item: ImageCacheItem, window: Option<&mut Window>, cx: &mut App) {
    if let Some(Ok(image)) = item.get() {
        cx.drop_image(image, window);
    }
}

fn load_filtered(
    path: &Path,
    filter: &(dyn Fn(&Path, RgbaImage) -> RgbaImage + Send + Sync),
) -> Result<Arc<RenderImage>, ImageCacheError> {
    let bytes = std::fs::read(path).map_err(|e| ImageCacheError::Io(Arc::new(e)))?;
    let image =
        image::load_from_memory(&bytes).map_err(|e| ImageCacheError::Other(Arc::new(e.into())))?;
    Ok(render_image(filter(path, image.into_rgba8())))
}
