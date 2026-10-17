// No console window behind the app on Windows (release builds only, so
// debug builds still show panics and logs).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod canvas;
mod computer;
mod design;
mod sandbox;
mod session;
mod store;
mod wiki;
mod worlds;
mod worldsim;
mod worldview;

use store::Progress;

fn load_progress() -> (Progress, Option<std::path::PathBuf>) {
    let path = store::default_path();
    let progress = match &path {
        Some(p) if p.exists() => Progress::load(p).unwrap_or_else(|e| {
            eprintln!("could not read {}: {e}; starting fresh", p.display());
            // Keep the unreadable file instead of overwriting it on next save.
            let _ = std::fs::rename(p, p.with_extension("json.bad"));
            Progress::default()
        }),
        _ => Progress::default(),
    };
    (progress, path)
}

/// Graphics go through wgpu, which picks Vulkan, DirectX 12, Metal or
/// OpenGL, whichever works; `WGPU_BACKEND=gl` (or `vulkan`, `dx12`) forces
/// one.
fn main() -> eframe::Result {
    let (progress, path) = load_progress();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("makine")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 560.0])
            .with_fullscreen(progress.fullscreen),
        ..Default::default()
    };
    let result = eframe::run_native(
        "makine",
        options,
        Box::new(move |cc| {
            if let Some(rs) = &cc.wgpu_render_state {
                worldview::gpu::remember_target_format(&cc.egui_ctx, rs.target_format);
            }
            Ok(Box::new(app::App::new(progress, path)))
        }),
    );
    if let Err(e) = &result {
        report_failure(e);
    }
    result
}

/// The app has no console on Windows, so a startup failure would be silent.
/// Show it in a message box instead.
fn report_failure(e: &eframe::Error) {
    let text = format!(
        "makine açılamadı: grafik sürücüsüyle pencere oluşturulamadı.\n\n{e}\n\n\
         Ekran kartı sürücülerini güncellemeyi ya da WGPU_BACKEND=gl ortam \
         değişkeniyle OpenGL'i denemeyi deneyebilirsin."
    );
    eprintln!("{text}");
    #[cfg(windows)]
    {
        rfd::MessageDialog::new()
            .set_title("makine")
            .set_description(&text)
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
