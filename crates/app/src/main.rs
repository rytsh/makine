// No console window behind the app on Windows (release builds only, so
// debug builds still show panics and logs).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod canvas;
mod computer;
mod design;
mod profiles;
mod sandbox;
mod session;
mod store;
mod theme;
mod wiki;
mod worlds;
mod worldsim;
mod worldview;

use profiles::Profiles;
use store::Progress;

/// The profiles and the progress of the one in use (with nowhere to save
/// if there is no data directory).
fn load_progress() -> (Progress, Option<Profiles>) {
    match store::data_dir() {
        Some(dir) => {
            let profiles = Profiles::load(&dir);
            (profiles.progress(profiles.current), Some(profiles))
        }
        None => (Progress::default(), None),
    }
}

/// The logo, for the window and the taskbar.
fn icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("../../../assets/logo.png")).ok()?;
    let img = img.into_rgba8();
    Some(egui::IconData {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    })
}

/// Graphics go through wgpu, which picks Vulkan, DirectX 12, Metal or
/// OpenGL, whichever works; `WGPU_BACKEND=gl` (or `vulkan`, `dx12`) forces
/// one.
fn main() -> eframe::Result {
    let (progress, profiles) = load_progress();
    let mut viewport = egui::ViewportBuilder::default();
    if let Some(icon) = icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport: viewport
            .with_title("makine")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 560.0])
            .with_fullscreen(progress.fullscreen),
        ..Default::default()
    };
    let result = eframe::run_native(
        "makine",
        options,
        Box::new(move |_cc| Ok(Box::new(app::App::with_profiles(progress, profiles)))),
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
