//! Imágenes dentro de la TUI: miniaturas de fondos y de la vista previa del
//! tema (con sixel, kitty o semibloques según la terminal) y los píxeles que
//! usa la paleta "desde un fondo". Se decodifican en un hilo para que la
//! interfaz no se congele con imágenes grandes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use image::DynamicImage;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

type Rgb = (u8, u8, u8);

pub enum Entry {
    Loading,
    Ready(Box<StatefulProtocol>),
    Failed,
}

struct Loaded {
    path: PathBuf,
    result: Option<(DynamicImage, Vec<Rgb>)>,
}

/// Píxeles de una imagen reducida (como mucho ~4000), para sacar su paleta.
pub fn sample_pixels(img: &DynamicImage) -> Vec<Rgb> {
    let small = img.thumbnail(64, 64).to_rgb8();
    small.pixels().map(|p| (p[0], p[1], p[2])).collect()
}

pub struct Images {
    picker: Picker,
    entries: HashMap<PathBuf, Entry>,
    pixels: HashMap<PathBuf, Vec<Rgb>>,
    tx: Sender<Loaded>,
    rx: Receiver<Loaded>,
}

impl Images {
    pub fn new(picker: Picker) -> Images {
        let (tx, rx) = channel();
        Images {
            picker,
            entries: HashMap::new(),
            pixels: HashMap::new(),
            tx,
            rx,
        }
    }

    /// Pide cargar una imagen si aún no se pidió.
    pub fn request(&mut self, path: &Path) {
        if self.entries.contains_key(path) {
            return;
        }
        self.entries.insert(path.to_path_buf(), Entry::Loading);
        let tx = self.tx.clone();
        let path = path.to_path_buf();
        std::thread::spawn(move || {
            let result = image::ImageReader::open(&path)
                .ok()
                .and_then(|r| r.with_guessed_format().ok())
                .and_then(|r| r.decode().ok())
                .map(|img| {
                    let img = img.thumbnail(1280, 1280);
                    let px = sample_pixels(&img);
                    (img, px)
                });
            let _ = tx.send(Loaded { path, result });
        });
    }

    /// Olvida una imagen (por ejemplo `preview.png` cuando se vuelve a capturar).
    pub fn forget(&mut self, path: &Path) {
        self.entries.remove(path);
        self.pixels.remove(path);
    }

    /// Recoge las imágenes que ya se decodificaron; devuelve sus rutas.
    pub fn poll(&mut self) -> Vec<PathBuf> {
        let mut done = vec![];
        while let Ok(l) = self.rx.try_recv() {
            match l.result {
                Some((img, px)) => {
                    let proto = self.picker.new_resize_protocol(img);
                    self.entries
                        .insert(l.path.clone(), Entry::Ready(Box::new(proto)));
                    self.pixels.insert(l.path.clone(), px);
                }
                None => {
                    self.entries.insert(l.path.clone(), Entry::Failed);
                }
            }
            done.push(l.path);
        }
        done
    }

    pub fn entry(&mut self, path: &Path) -> Option<&mut Entry> {
        self.entries.get_mut(path)
    }

    pub fn pixels(&self, path: &Path) -> Option<&Vec<Rgb>> {
        self.pixels.get(path)
    }
}

/// Reduce una captura a 1920×1080 como máximo y la guarda como PNG.
pub fn save_preview(src: &Path, dest: &Path) -> Result<(), String> {
    let img = image::open(src).map_err(|e| e.to_string())?;
    let img = if img.width() > 1920 || img.height() > 1080 {
        img.resize(1920, 1080, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };
    if let Some(dir) = dest.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    img.save_with_format(dest, image::ImageFormat::Png)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb as P, RgbImage};

    fn write(dir: &Path, name: &str, w: u32, h: u32, c: [u8; 3]) -> PathBuf {
        let p = dir.join(name);
        RgbImage::from_pixel(w, h, P(c)).save(&p).unwrap();
        p
    }

    #[test]
    fn loads_in_the_background_and_samples_pixels() {
        let t = tempfile::tempdir().unwrap();
        let p = write(t.path(), "a.png", 200, 100, [10, 20, 30]);
        let mut imgs = Images::new(Picker::halfblocks());
        imgs.request(&p);
        assert!(matches!(imgs.entry(&p), Some(Entry::Loading)));
        let mut done = vec![];
        for _ in 0..100 {
            done = imgs.poll();
            if !done.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(done, vec![p.clone()]);
        assert!(matches!(imgs.entry(&p), Some(Entry::Ready(_))));
        let px = imgs.pixels(&p).unwrap();
        assert!(px.iter().all(|c| *c == (10, 20, 30)));
    }

    #[test]
    fn a_broken_image_fails_without_crashing() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("rota.png");
        std::fs::write(&p, "no es una imagen").unwrap();
        let mut imgs = Images::new(Picker::halfblocks());
        imgs.request(&p);
        for _ in 0..100 {
            if !imgs.poll().is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(matches!(imgs.entry(&p), Some(Entry::Failed)));
    }

    #[test]
    fn preview_is_shrunk_to_1080p() {
        let t = tempfile::tempdir().unwrap();
        let src = write(t.path(), "big.png", 3840, 2160, [1, 2, 3]);
        let dest = t.path().join("out/preview.png");
        save_preview(&src, &dest).unwrap();
        let out = image::open(&dest).unwrap();
        assert_eq!((out.width(), out.height()), (1920, 1080));
        assert!(save_preview(&t.path().join("nope.png"), &dest).is_err());
    }
}
