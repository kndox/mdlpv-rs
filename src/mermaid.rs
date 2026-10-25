use base64::{Engine as _, engine::general_purpose::STANDARD};
use merman::{
    Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest,
};
use std::{sync::LazyLock, time::Duration};

static RENDERER: LazyLock<Renderer> = LazyLock::new(|| {
    let config = MermaidConfig::from_value(serde_json::json!({
        "securityLevel": "strict",
        "htmlLabels": false,
        "flowchart": { "htmlLabels": false },
        "secure": ["secure", "securityLevel", "startOnLoad", "maxTextSize",
            "suppressErrorRendering", "maxEdges", "fontFamily", "altFontFamily",
            "themeCSS", "htmlLabels"]
    }));
    Renderer::new().with_engine(Engine::new().with_site_config(config))
});

pub fn render_image(source: &str) -> anyhow::Result<String> {
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
