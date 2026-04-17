use crate::error::AppError;
use image::{DynamicImage, GenericImageView, GrayImage};
use pdfium_render::prelude::*;

pub fn decode_qr_from_file(file_path: &str) -> Result<String, AppError> {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "png" | "jpg" | "jpeg" => decode_qr_from_image_file(file_path),
        "pdf" => decode_qr_from_pdf(file_path),
        _ => Err(AppError {
            code: "UNSUPPORTED_FILE".into(),
            message: "Unsupported file type".into(),
        }),
    }
}

fn decode_qr_from_image_file(file_path: &str) -> Result<String, AppError> {
    let img = image::open(file_path).map_err(|e| AppError {
        code: "IMAGE_ERROR".into(),
        message: e.to_string(),
    })?;
    decode_qr_from_dynamic_image(img)
}

fn decode_qr_from_dynamic_image(img: DynamicImage) -> Result<String, AppError> {
    // Try full image (grayscale)
    if let Ok(text) = try_decode_qr(&img) {
        return Ok(text);
    }

    // QR codes on receipts are typically near the bottom.
    // Crop bottom 40% and retry — makes the QR larger relative to the image.
    let (w, h) = img.dimensions();
    let crop_top = h * 60 / 100;
    let cropped = img.crop_imm(0, crop_top, w, h - crop_top);
    if let Ok(text) = try_decode_qr(&cropped) {
        return Ok(text);
    }

    // Low-contrast thermal receipt scans often fail on raw grayscale because
    // rqrr's error-correction can't handle the noise.  Otsu binarization
    // produces a clean black/white image that decodes reliably.
    let gray = cropped.to_luma8();
    let binarized = otsu_binarize(&gray);
    try_decode_qr_gray(&binarized)
}

fn try_decode_qr(img: &DynamicImage) -> Result<String, AppError> {
    let gray = img.to_luma8();
    try_decode_qr_gray(&gray)
}

fn try_decode_qr_gray(gray: &GrayImage) -> Result<String, AppError> {
    let mut prepared = rqrr::PreparedImage::prepare(gray.clone());
    let grids = prepared.detect_grids();

    for grid in grids {
        if let Ok((_, content)) = grid.decode() {
            return Ok(content);
        }
    }

    Err(AppError {
        code: "QR_NOT_FOUND".into(),
        message: "No QR code found in image".into(),
    })
}

/// Otsu's method: compute optimal threshold that minimizes intra-class variance,
/// then binarize to pure black/white.
fn otsu_binarize(img: &GrayImage) -> GrayImage {
    let mut histogram = [0u32; 256];
    for p in img.pixels() {
        histogram[p.0[0] as usize] += 1;
    }
    let total = (img.width() * img.height()) as f64;
    let mut sum_total: f64 = 0.0;
    for (i, &count) in histogram.iter().enumerate() {
        sum_total += i as f64 * count as f64;
    }
    let mut sum_bg: f64 = 0.0;
    let mut weight_bg: f64 = 0.0;
    let mut max_variance: f64 = 0.0;
    let mut best_threshold: u8 = 0;
    for (i, &count) in histogram.iter().enumerate() {
        weight_bg += count as f64;
        if weight_bg == 0.0 {
            continue;
        }
        let weight_fg = total - weight_bg;
        if weight_fg == 0.0 {
            break;
        }
        sum_bg += i as f64 * count as f64;
        let mean_bg = sum_bg / weight_bg;
        let mean_fg = (sum_total - sum_bg) / weight_fg;
        let variance = weight_bg * weight_fg * (mean_bg - mean_fg).powi(2);
        if variance > max_variance {
            max_variance = variance;
            best_threshold = i as u8;
        }
    }

    let mut out = img.clone();
    for p in out.pixels_mut() {
        p.0[0] = if p.0[0] > best_threshold { 255 } else { 0 };
    }
    out
}

/// Renders each page of a PDF to a raster image and attempts to decode a QR code.
///
/// Uses PDFium (Google's PDF renderer) via pdfium-render to rasterize pages.
/// This works for all PDF types: scanned documents, digitally generated PDFs,
/// and PDFs with vector-rendered QR codes. Requires the PDFium dynamic library
/// (pdfium.dll / libpdfium.so / libpdfium.dylib) to be available at runtime.
fn get_pdfium() -> Result<&'static Pdfium, AppError> {
    use std::sync::OnceLock;
    static PDFIUM: OnceLock<Result<Pdfium, String>> = OnceLock::new();
    PDFIUM
        .get_or_init(|| {
            Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
                .or_else(|_| Pdfium::bind_to_system_library())
                .map(Pdfium::new)
                .map_err(|e| format!("PDFium library not found. Place pdfium.dll next to the executable. Error: {}", e))
        })
        .as_ref()
        .map_err(|e| AppError {
            code: "PDFIUM_NOT_FOUND".into(),
            message: e.clone(),
        })
}

fn decode_qr_from_pdf(file_path: &str) -> Result<String, AppError> {
    let pdfium = get_pdfium()?;

    let document = pdfium
        .load_pdf_from_file(file_path, None)
        .map_err(|e| AppError {
            code: "PDF_ERROR".into(),
            message: format!("Failed to load PDF: {}", e),
        })?;
    // Try multiple render widths to work around pixel aliasing: at certain
    // resolutions QR module boundaries land between pixels, corrupting the
    // code enough to fail error-correction.  Starting lower is also faster.
    let candidate_widths: &[i32] = &[1500, 2000, 2500];

    for page in document.pages().iter() {
        for &width in candidate_widths {
            let render_config = PdfRenderConfig::new()
                .set_target_width(width)
                .set_maximum_height(width * 4);

            let bitmap = match page.render_with_config(&render_config) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let img = match bitmap.as_image() {
                Ok(i) => i,
                Err(_) => continue,
            };

            if let Ok(qr_text) = decode_qr_from_dynamic_image(img) {
                return Ok(qr_text);
            }
        }
    }

    Err(AppError {
        code: "QR_NOT_FOUND".into(),
        message: "No QR code found in any page of the PDF".into(),
    })
}
