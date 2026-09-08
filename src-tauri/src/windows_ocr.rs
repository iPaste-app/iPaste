use std::{
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use rapidocr_core::{
    config::PipelineConfig,
    model::{model_set_by_name, ModelAssetKind, ModelSetSpec},
    types::{OcrLine, Quad},
    RapidOcr,
};

pub const ENGINE_ID: &str = "paddleocr-onnx";
pub const ENGINE_VERSION: &str = "PP-OCRv6";
pub const LANGUAGE: &str = "zh-Hans+en";
pub const MODEL_BASE_URL: &str = "https://www.modelscope.cn/models/RapidAI/RapidOCR/resolve/";
pub const MODEL_SOURCE_URL: &str = "https://www.modelscope.cn/models/RapidAI/RapidOCR";
const FAST_TOTAL_BYTES: u64 = 6_932_119;
const BEST_TOTAL_BYTES: u64 = 31_824_456;

const MIN_TEXT_SCORE: f32 = 0.65;
static RUNTIME_INITIALIZATION: OnceLock<Result<(), String>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DownloadAsset {
    pub role: &'static str,
    pub name: &'static str,
    pub path: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecognizedLine {
    pub text: String,
    pub score: f32,
    pub points: [[f32; 2]; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecognitionOutput {
    pub model_set: &'static str,
    pub lines: Vec<RecognizedLine>,
}

pub struct OcrService {
    runtime_path: PathBuf,
    cached: Mutex<Option<CachedEngine>>,
}

struct CachedEngine {
    mode: String,
    model_dir: PathBuf,
    model_set: &'static str,
    engine: RapidOcr,
    retry_recognizer: RapidOcr,
}

impl OcrService {
    pub fn new(runtime_path: PathBuf) -> Self {
        Self {
            runtime_path,
            cached: Mutex::new(None),
        }
    }

    pub fn is_loaded(&self, mode: &str, model_dir: &Path) -> Result<bool, String> {
        let cached = self.cached.lock().map_err(|error| error.to_string())?;
        Ok(cached.as_ref().is_some_and(|current| {
            current.mode == mode && current.model_dir.as_path() == model_dir
        }))
    }

    pub fn recognize(
        &self,
        mode: &str,
        model_dir: &Path,
        image_path: &Path,
    ) -> Result<RecognitionOutput, String> {
        if !image_path.is_file() {
            return Err("图片文件不存在".to_string());
        }
        initialize_runtime(&self.runtime_path)?;

        let mut cached = self.cached.lock().map_err(|error| error.to_string())?;
        let needs_reload = cached
            .as_ref()
            .is_none_or(|current| current.mode != mode || current.model_dir.as_path() != model_dir);

        if needs_reload {
            *cached = Some(load_engine(mode, model_dir)?);
        }

        let current = cached
            .as_mut()
            .ok_or_else(|| "无法初始化图片 OCR 引擎".to_string())?;
        let image = image::open(image_path)
            .map_err(|error| format!("无法读取图片：{error}"))?
            .to_rgb8();
        let output = current
            .engine
            .run_image(&image)
            .map_err(|error| format!("PaddleOCR 识别失败：{error:#}"))?;
        let lines =
            clean_suspected_icon_prefixes(&image, output.lines, &mut current.retry_recognizer);

        Ok(RecognitionOutput {
            model_set: current.model_set,
            lines: lines
                .into_iter()
                .map(|line| RecognizedLine {
                    text: line.text,
                    score: line.score,
                    points: line.bbox.points,
                })
                .collect(),
        })
    }

    pub fn invalidate(&self) -> Result<(), String> {
        *self.cached.lock().map_err(|error| error.to_string())? = None;
        Ok(())
    }
}

fn initialize_runtime(runtime_path: &Path) -> Result<(), String> {
    RUNTIME_INITIALIZATION
        .get_or_init(|| {
            if !runtime_path.is_file() {
                return Err(format!(
                    "未找到 ONNX Runtime：{}。请重新安装 iPaste。",
                    runtime_path.display()
                ));
            }
            ort::init_from(runtime_path)
                .map_err(|error| format!("无法加载 ONNX Runtime：{error}"))?
                .commit();
            Ok(())
        })
        .clone()
}

pub fn assets_for_mode(mode: &str) -> Result<Vec<DownloadAsset>, String> {
    let assets = model_set_for_mode(mode)?
        .assets()
        .into_iter()
        .map(|asset| {
            let path = asset
                .url
                .strip_prefix(MODEL_BASE_URL)
                .ok_or_else(|| format!("OCR 模型地址不受支持：{}", asset.url))?;
            let sha256 = asset
                .sha256
                .ok_or_else(|| format!("OCR 模型缺少校验值：{}", asset.filename))?;
            Ok(DownloadAsset {
                role: asset_role(asset.kind),
                name: asset.filename,
                path,
                size: asset_size(asset.filename)?,
                sha256,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let expected_total = match mode {
        "best" => BEST_TOTAL_BYTES,
        _ => FAST_TOTAL_BYTES,
    };
    debug_assert_eq!(
        assets.iter().map(|asset| asset.size).sum::<u64>(),
        expected_total
    );
    Ok(assets)
}

fn load_engine(mode: &str, model_dir: &Path) -> Result<CachedEngine, String> {
    let model_set = model_set_for_mode(mode)?;
    let mut config = model_set.config(model_dir);
    config.text_score = MIN_TEXT_SCORE;
    config.inference.intra_threads = std::thread::available_parallelism()
        .map(|threads| threads.get().min(4))
        .unwrap_or(1);
    config.inference.inter_threads = 1;
    config.inference.parallel_execution = false;

    let retry_config = config
        .clone()
        .with_pipeline(PipelineConfig::recognition_only());
    let engine = RapidOcr::from_config(config)
        .map_err(|error| format!("无法加载 PaddleOCR 模型：{error:#}"))?;
    let retry_recognizer = RapidOcr::from_config(retry_config)
        .map_err(|error| format!("无法加载 PaddleOCR 文字识别模型：{error:#}"))?;
    Ok(CachedEngine {
        mode: mode.to_string(),
        model_dir: model_dir.to_path_buf(),
        model_set: model_set.name,
        engine,
        retry_recognizer,
    })
}

fn clean_suspected_icon_prefixes(
    image: &image::RgbImage,
    lines: Vec<OcrLine>,
    retry_recognizer: &mut RapidOcr,
) -> Vec<OcrLine> {
    lines
        .into_iter()
        .map(|line| clean_suspected_icon_prefix(image, line, retry_recognizer))
        .collect()
}

fn clean_suspected_icon_prefix(
    image: &image::RgbImage,
    mut line: OcrLine,
    retry_recognizer: &mut RapidOcr,
) -> OcrLine {
    if line.score >= 0.97 || line.text.chars().count() < 3 {
        return line;
    }

    let Some((left, top, right, bottom)) = clipped_axis_bounds(&line.bbox, image) else {
        return line;
    };
    let crop = image::imageops::crop_imm(image, left, top, right - left, bottom - top).to_image();
    let Some(split_x) = leading_component_gap(&crop) else {
        return line;
    };
    if split_x >= crop.width().saturating_sub(2) {
        return line;
    }

    let suffix =
        image::imageops::crop_imm(&crop, split_x, 0, crop.width() - split_x, crop.height())
            .to_image();
    let Ok(candidate_output) = retry_recognizer.run_image(&suffix) else {
        return line;
    };
    let Some(candidate) = candidate_output.lines.into_iter().next() else {
        return line;
    };
    let original_text = line.text.trim();
    let candidate_text = candidate.text.trim();
    if candidate_text.is_empty()
        || !candidate.score.is_finite()
        || candidate.score < 0.95
        || candidate.score < line.score + 0.025
        || !original_text.ends_with(candidate_text)
    {
        return line;
    }

    let removed_prefix = &original_text[..original_text.len() - candidate_text.len()];
    if removed_prefix.chars().count() != 1 || removed_prefix.chars().any(char::is_whitespace) {
        return line;
    }

    line.text = candidate_text.to_string();
    line.score = candidate.score;
    line.bbox = Quad::from_xyxy(
        (left + split_x) as f32,
        top as f32,
        right.saturating_sub(1) as f32,
        bottom.saturating_sub(1) as f32,
    );
    line
}

fn clipped_axis_bounds(bbox: &Quad, image: &image::RgbImage) -> Option<(u32, u32, u32, u32)> {
    let min_x = bbox
        .points
        .iter()
        .map(|point| point[0])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as u32;
    let min_y = bbox
        .points
        .iter()
        .map(|point| point[1])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as u32;
    let max_x = bbox
        .points
        .iter()
        .map(|point| point[0])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(image.width() as f32) as u32;
    let max_y = bbox
        .points
        .iter()
        .map(|point| point[1])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(image.height() as f32) as u32;
    (max_x > min_x && max_y > min_y).then_some((min_x, min_y, max_x, max_y))
}

fn leading_component_gap(crop: &image::RgbImage) -> Option<u32> {
    let width = crop.width() as usize;
    let height = crop.height() as usize;
    if height < 8 || width < height.saturating_mul(2) {
        return None;
    }

    let mut energy = vec![0_u64; width];
    for y in 0..height {
        for x in 0..width {
            let pixel = crop.get_pixel(x as u32, y as u32).0;
            let luminance = pixel_luminance(pixel);
            if x > 0 {
                let previous = crop.get_pixel((x - 1) as u32, y as u32).0;
                energy[x] += luminance.abs_diff(pixel_luminance(previous)) as u64;
            }
            if y > 0 {
                let previous = crop.get_pixel(x as u32, (y - 1) as u32).0;
                energy[x] += luminance.abs_diff(pixel_luminance(previous)) as u64;
            }
        }
    }

    let max_energy = *energy.iter().max()?;
    if max_energy == 0 {
        return None;
    }
    let threshold = (max_energy / 12).max(1);
    let first_active = energy.iter().position(|value| *value > threshold)?;
    let last_active = energy.iter().rposition(|value| *value > threshold)?;
    let minimum_gap = (height * 35 / 100).max(6);
    let minimum_left = (height * 30 / 100).max(3);
    let maximum_left = height * 140 / 100;
    let minimum_right = height * 120 / 100;

    let mut best_gap = None;
    let mut index = first_active + 1;
    while index < last_active {
        if energy[index] > threshold {
            index += 1;
            continue;
        }
        let start = index;
        while index < last_active && energy[index] <= threshold {
            index += 1;
        }
        let end = index;
        let gap_width = end - start;
        let left_width = start.saturating_sub(first_active);
        let right_width = last_active.saturating_sub(end);
        if gap_width >= minimum_gap
            && (minimum_left..=maximum_left).contains(&left_width)
            && right_width >= minimum_right
            && best_gap.is_none_or(|(_, best_width)| gap_width > best_width)
        {
            best_gap = Some((end.saturating_sub(2), gap_width));
        }
    }

    best_gap.map(|(split, _)| split as u32)
}

fn pixel_luminance(rgb: [u8; 3]) -> u8 {
    ((u16::from(rgb[0]) * 77 + u16::from(rgb[1]) * 150 + u16::from(rgb[2]) * 29) >> 8) as u8
}

fn model_set_for_mode(mode: &str) -> Result<&'static ModelSetSpec, String> {
    let name = match mode {
        "fast" => "ppocrv6-tiny",
        "best" => "ppocrv6-small",
        _ => return Err("请选择有效的图片 OCR 模式".to_string()),
    };
    model_set_by_name(name).ok_or_else(|| format!("未找到 PaddleOCR 模型配置：{name}"))
}

fn asset_role(kind: ModelAssetKind) -> &'static str {
    match kind {
        ModelAssetKind::Detection => "detection",
        ModelAssetKind::Classification => "classification",
        ModelAssetKind::Recognition => "recognition",
        ModelAssetKind::Dictionary => "dictionary",
    }
}

fn asset_size(filename: &str) -> Result<u64, String> {
    match filename {
        "PP-OCRv6_det_tiny.onnx" => Ok(1_829_618),
        "PP-OCRv6_rec_tiny.onnx" => Ok(4_489_813),
        "ppocrv6_tiny_dict.txt" => Ok(27_156),
        "PP-OCRv6_det_small.onnx" => Ok(9_929_594),
        "PP-OCRv6_rec_small.onnx" => Ok(21_234_383),
        "ppocrv6_dict.txt" => Ok(74_947),
        "ch_ppocr_mobile_v2.0_cls_mobile.onnx" => Ok(585_532),
        _ => Err(format!("未知的 PaddleOCR 模型文件：{filename}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_mode_uses_tiny_models_with_expected_total() {
        let assets = assets_for_mode("fast").unwrap();
        assert!(assets
            .iter()
            .any(|asset| asset.name == "PP-OCRv6_det_tiny.onnx"));
        assert!(assets
            .iter()
            .any(|asset| asset.name == "PP-OCRv6_rec_tiny.onnx"));
        assert_eq!(
            assets.iter().map(|asset| asset.size).sum::<u64>(),
            FAST_TOTAL_BYTES
        );
    }

    #[test]
    fn best_mode_uses_small_models_with_expected_total() {
        let assets = assets_for_mode("best").unwrap();
        assert!(assets
            .iter()
            .any(|asset| asset.name == "PP-OCRv6_det_small.onnx"));
        assert!(assets
            .iter()
            .any(|asset| asset.name == "PP-OCRv6_rec_small.onnx"));
        assert_eq!(
            assets.iter().map(|asset| asset.size).sum::<u64>(),
            BEST_TOTAL_BYTES
        );
    }

    #[test]
    fn assets_keep_modelscope_paths_relative_to_the_pinned_base() {
        for mode in ["fast", "best"] {
            for asset in assets_for_mode(mode).unwrap() {
                assert!(!asset.path.starts_with("http"));
                assert!(!asset.path.contains(".."));
                assert_eq!(asset.sha256.len(), 64);
            }
        }
    }

    #[test]
    fn finds_a_large_gap_after_a_compact_leading_component() {
        let mut image = image::RgbImage::from_pixel(120, 40, image::Rgb([255, 255, 255]));
        for x in (6..27).step_by(4) {
            for y in 6..34 {
                image.put_pixel(x, y, image::Rgb([30, 30, 30]));
            }
        }
        for x in (52..116).step_by(5) {
            for y in 5..35 {
                image.put_pixel(x, y, image::Rgb([20, 20, 20]));
            }
        }

        let split = leading_component_gap(&image).unwrap();
        assert!((40..=54).contains(&split));
    }

    #[test]
    #[ignore = "requires external PP-OCRv6 models and an image fixture"]
    fn recognizes_external_fixture() {
        let model_dir = std::env::var_os("IPASTE_OCR_TEST_MODEL_DIR")
            .map(PathBuf::from)
            .expect("IPASTE_OCR_TEST_MODEL_DIR is required");
        let image_path = std::env::var_os("IPASTE_OCR_TEST_IMAGE")
            .map(PathBuf::from)
            .expect("IPASTE_OCR_TEST_IMAGE is required");
        let mode = std::env::var("IPASTE_OCR_TEST_MODE").unwrap_or_else(|_| "fast".to_string());

        let runtime_path = std::env::var_os("IPASTE_OCR_TEST_RUNTIME")
            .map(PathBuf::from)
            .expect("IPASTE_OCR_TEST_RUNTIME is required");
        let output = OcrService::new(runtime_path)
            .recognize(&mode, &model_dir, &image_path)
            .unwrap();
        assert!(!output.lines.is_empty());
        for line in output.lines {
            println!("{:.4}\t{}", line.score, line.text);
        }
    }
}
