use std::{path::Path, sync::Arc};

use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{RenderSettings, hayro_interpret::InterpreterSettings, hayro_syntax::Pdf, render};
use image::{DynamicImage, ImageReader, RgbaImage};

pub mod worker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewKind {
    Image,
    Pdf,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewSourceMeta {
    pub kind: PreviewKind,
    pub width: u32,
    pub height: u32,
    pub pdf_page: Option<usize>,
}

pub enum PreviewBuildResult {
    Ready {
        image: DynamicImage,
        meta: PreviewSourceMeta,
    },
    Unsupported {
        reason: String,
    },
}

pub fn classify_path(path: &Path) -> PreviewKind {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    match ext.as_deref() {
        Some(
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tif" | "tiff" | "heic" | "heif"
            | "svg",
        ) => PreviewKind::Image,
        Some("pdf") => PreviewKind::Pdf,
        _ => PreviewKind::Unsupported,
    }
}

pub fn build_preview(path: &Path) -> Result<PreviewBuildResult, String> {
    if !path.is_file() {
        return Ok(PreviewBuildResult::Unsupported {
            reason: "Preview is only available for files".to_string(),
        });
    }

    match classify_path(path) {
        PreviewKind::Image => load_image_preview(path),
        PreviewKind::Pdf => load_pdf_preview(path),
        PreviewKind::Unsupported => Ok(PreviewBuildResult::Unsupported {
            reason: "Preview not supported for this file type".to_string(),
        }),
    }
}

fn load_image_preview(path: &Path) -> Result<PreviewBuildResult, String> {
    let reader = ImageReader::open(path)
        .map_err(|error| format!("Failed to open image file {}: {error}", path.display()))?;
    let image = reader
        .decode()
        .map_err(|error| format!("Failed to decode image {}: {error}", path.display()))?;

    Ok(PreviewBuildResult::Ready {
        meta: PreviewSourceMeta {
            kind: PreviewKind::Image,
            width: image.width(),
            height: image.height(),
            pdf_page: None,
        },
        image,
    })
}

fn load_pdf_preview(path: &Path) -> Result<PreviewBuildResult, String> {
    let data = std::fs::read(path)
        .map_err(|error| format!("Failed to read PDF {}: {error}", path.display()))?;
    let pdf = Pdf::new(Arc::new(data))
        .map_err(|error| format!("Failed to parse PDF {}: {error:?}", path.display()))?;
    let page = pdf.pages().first().ok_or_else(|| {
        format!(
            "PDF {} contains no pages and cannot be previewed",
            path.display()
        )
    })?;

    let pixmap = render(
        page,
        &InterpreterSettings::default(),
        &RenderSettings {
            x_scale: 2.0,
            y_scale: 2.0,
            bg_color: WHITE,
            ..Default::default()
        },
    );

    let width = pixmap.width() as u32;
    let height = pixmap.height() as u32;
    let bytes = pixmap.data_as_u8_slice().to_vec();
    let image = RgbaImage::from_raw(width, height, bytes).ok_or_else(|| {
        format!(
            "Failed to convert rendered PDF page into image buffer for {}",
            path.display()
        )
    })?;

    Ok(PreviewBuildResult::Ready {
        image: DynamicImage::ImageRgba8(image),
        meta: PreviewSourceMeta {
            kind: PreviewKind::Pdf,
            width,
            height,
            pdf_page: Some(1),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_path_recognizes_image_pdf_and_unsupported() {
        assert_eq!(classify_path(Path::new("photo.png")), PreviewKind::Image);
        assert_eq!(classify_path(Path::new("doc.PDF")), PreviewKind::Pdf);
        assert_eq!(
            classify_path(Path::new("archive.zip")),
            PreviewKind::Unsupported
        );
    }
}
