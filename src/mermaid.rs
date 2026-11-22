use base64::{Engine as _, engine::general_purpose::STANDARD};
use merman::{
    MermaidEngine, OperationControl, RenderOutput, RenderRequest, Renderer,
    SourcePresentationConfig, SvgRequest,
};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, PoisonError},
    time::Duration,
};

// Previews re-render the whole document on each edit; reuse unchanged diagrams.
// The cache is cleared when either the entry or the byte budget would be exceeded.
const CACHE_CAPACITY: usize = 128;
const CACHE_MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Default)]
struct Cache {
    entries: HashMap<String, String>,
    bytes: usize,
}

static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(Default::default);

static RENDERER: LazyLock<Renderer> = LazyLock::new(|| {
    let config = SourcePresentationConfig::from_value(serde_json::json!({
        "securityLevel": "strict",
        "htmlLabels": false,
        "flowchart": { "htmlLabels": false },
        "secure": ["secure", "securityLevel", "startOnLoad", "maxTextSize",
            "suppressErrorRendering", "maxEdges", "fontFamily", "altFontFamily",
            "themeCSS", "htmlLabels"]
    }));
    Renderer::new().with_presentation_engine(MermaidEngine::new().with_site_config(config))
});

pub fn render_image(source: &str) -> anyhow::Result<String> {
    if let Some(image) = lock_cache().entries.get(source) {
        return Ok(image.clone());
    }
    // Failures are not cached: deadline errors may succeed on a later attempt.
    let image = render_uncached(source)?;
    let size = source.len() + image.len();
    if size <= CACHE_MAX_BYTES {
        let mut cache = lock_cache();
        if cache.entries.len() >= CACHE_CAPACITY || cache.bytes + size > CACHE_MAX_BYTES {
            *cache = Cache::default();
        }
        if let Some(previous) = cache.entries.insert(source.to_owned(), image.clone()) {
            cache.bytes -= source.len() + previous.len();
        }
        cache.bytes += size;
    }
    Ok(image)
}

fn lock_cache() -> std::sync::MutexGuard<'static, Cache> {
    CACHE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn render_uncached(source: &str) -> anyhow::Result<String> {
    let request = SvgRequest {
        // Also convert labels from diagram families that use foreignObject independently
        // of htmlLabels, so they remain visible when SVG is loaded as an image.
        pipeline: Some(merman::svg::SvgPipeline::resvg_safe()),
        ..Default::default()
    };
    let output = RENDERER.render(RenderRequest::svg(
        source,
        OperationControl::new().with_deadline(Duration::from_secs(5)),
        request,
    ))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        anyhow::bail!("no Mermaid diagram detected");
    };
    // An SVG with width="100%" has no useful intrinsic image width. Preserve its
    // layout dimensions instead of letting the browser scale tiny diagrams up.
    let document = roxmltree::Document::parse(svg.svg())?;
    let view_box = document
        .root_element()
        .attribute("viewBox")
        .ok_or_else(|| anyhow::anyhow!("rendered SVG has no viewBox"))?;
    let dimensions: Vec<f64> = view_box
        .split(|ch: char| ch.is_whitespace() || ch == ',')
        .filter(|value| !value.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    anyhow::ensure!(
        dimensions.len() == 4
            && dimensions.iter().all(|value| value.is_finite())
            && dimensions[2] > 0.0
            && dimensions[3] > 0.0,
        "rendered SVG has an invalid viewBox"
    );
    let width = dimensions[2].ceil();
    let height = dimensions[3].ceil();
    Ok(format!(
        r#"<div class="mermaid"><img alt="Mermaid diagram" width="{width}" height="{height}" src="data:image/svg+xml;base64,{}"></div>"#,
        STANDARD.encode(svg.svg())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caches_successful_renders_by_source() {
        let source = format!("flowchart TD\nA-->B{}", uuid::Uuid::new_v4().simple());
        let first = render_image(&source).unwrap();
        assert_eq!(lock_cache().entries.get(&source), Some(&first));
        assert_eq!(render_image(&source).unwrap(), first);
    }

    #[test]
    fn does_not_cache_failures() {
        let source = "not a mermaid diagram";
        assert!(render_image(source).is_err());
        assert!(!lock_cache().entries.contains_key(source));
    }
}
