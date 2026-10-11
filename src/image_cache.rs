use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use futures::FutureExt;
use gpui::{
    App, AppContext, Asset, Entity, ImageAssetLoader, ImageCache, ImageCacheError, ImageCacheItem,
    RenderImage, Resource, Window, hash,
};

/// GPUI keeps a decoded image until it is dropped. This cache drops the least
/// recently used image when it holds more than `capacity`.
pub struct BoundedImageCache {
    items: HashMap<u64, ImageCacheItem>,
    order: VecDeque<u64>,
    capacity: usize,
}

impl BoundedImageCache {
    pub fn new(capacity: usize, cx: &mut App) -> Entity<Self> {
        let cache = cx.new(|_| Self {
            items: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        });
        cx.observe_release(&cache, |cache, cx| {
            for (_, mut item) in cache.items.drain() {
                if let Some(Ok(image)) = item.get() {
                    cx.drop_image(image, None);
                }
            }
        })
        .detach();
        cache
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

        let load = <ImageAssetLoader as Asset>::load(resource.clone(), cx);
        let task = cx.background_executor().spawn(load).shared();
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
            if let Some(mut item) = self.items.remove(&old)
                && let Some(Ok(image)) = item.get()
            {
                cx.drop_image(image, Some(window));
            }
        }
        None
    }
}
