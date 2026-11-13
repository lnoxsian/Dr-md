use egui::{Image, Ui};
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

static DARK_RENDERER: OnceLock<merman::render::HeadlessRenderer> = OnceLock::new();
static LIGHT_RENDERER: OnceLock<merman::render::HeadlessRenderer> = OnceLock::new();

fn get_renderer(dark_mode: bool) -> &'static merman::render::HeadlessRenderer {
    if dark_mode {
        DARK_RENDERER.get_or_init(|| {
            let mut profile = merman::render::HostThemeProfile::one_dark();
            profile.roles.canvas = Some("transparent".to_string());
            profile.output.root_background =
                merman::render::HostThemeRootBackground::Color("transparent".to_string());
            merman::render::HeadlessRenderer::new()
                .with_diagram_id("dr-md-mermaid")
                .with_host_theme(&profile)
        })
    } else {
        LIGHT_RENDERER.get_or_init(|| {
            let mut profile = merman::render::HostThemeProfile::editor_light();
            profile.roles.canvas = Some("transparent".to_string());
            profile.output.root_background =
                merman::render::HostThemeRootBackground::Color("transparent".to_string());
            merman::render::HeadlessRenderer::new()
                .with_diagram_id("dr-md-mermaid")
                .with_host_theme(&profile)
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) enum MermaidCacheEntry {
    Rendering,
    Ready {
        uri: String,
        bytes: std::sync::Arc<[u8]>,
    },
    Failure(String),
}

/// Renders a Mermaid diagram into the UI.
/// Rendering is performed asynchronously in a background thread to prevent UI freezing.
pub fn render_mermaid(
    ui: &mut Ui,
    cache: &mut crate::CommonMarkCache,
    content: &str,
    max_width: f32,
) -> Result<(), String> {
    // Generate unique, deterministic cache key based on content and dark mode state.
    let dark_mode = ui.visuals().dark_mode;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    dark_mode.hash(&mut hasher);
    let hash = hasher.finish();

    let entry = {
        let mut map = cache.mermaid_cache.lock().unwrap();
        if let Some(entry) = map.get(&hash) {
            entry.clone()
        } else {
            map.insert(hash, MermaidCacheEntry::Rendering);

            let cache_clone = cache.mermaid_cache.clone();
            let content_str = content.to_string();
            let ctx = ui.ctx().clone();

            std::thread::spawn(move || {
                let result = render_mermaid_sync(&content_str, dark_mode, hash);
                let entry = match result {
                    Ok((uri, bytes)) => MermaidCacheEntry::Ready {
                        uri,
                        bytes: std::sync::Arc::from(bytes),
                    },
                    Err(err) => MermaidCacheEntry::Failure(err),
                };
                cache_clone.lock().unwrap().insert(hash, entry);
                ctx.request_repaint();
            });

            MermaidCacheEntry::Rendering
        }
    };

    match entry {
        MermaidCacheEntry::Rendering => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Rendering Mermaid diagram...");
            });
            Ok(())
        }
        MermaidCacheEntry::Ready { uri, bytes } => {
            let image = Image::from_bytes(uri.clone(), bytes.clone())
                .fit_to_original_size(1.0)
                .max_width(max_width)
                .sense(egui::Sense::click());

            let response = ui.add(image).on_hover_text("Preview diagram");

            if response.clicked() {
                cache.zoomed_image_request = Some((uri, bytes));
            }

            Ok(())
        }
        MermaidCacheEntry::Failure(err) => Err(err),
    }
}

fn render_mermaid_sync(
    content: &str,
    dark_mode: bool,
    hash: u64,
) -> Result<(String, Vec<u8>), String> {
    let renderer = get_renderer(dark_mode);

    let png_bytes = renderer
        .render_png_sync(content, &merman::render::raster::RasterOptions::default())
        .map_err(|e| format!("Merman engine error: {}", e))?
        .ok_or_else(|| "Failed to detect or parse Mermaid diagram".to_string())?;

    let uri = format!("bytes://mermaid_{}.png", hash);

    Ok((uri, png_bytes))
}
