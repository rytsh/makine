//! Taking a chip out of makine: Verilog for simulators and FPGAs, and a
//! Tiny Tapeout project for a real chip.

use egui::{RichText, Ui};
use makine_hdl::{Options, Project, TinyTapeout};
use makine_lang::{tr, trf};
use makine_levels::Test;
use makine_sim::{ChipDef, Library, TestStep};

use super::{App, Target};

/// Settings of the export panel; not saved.
#[derive(Default)]
pub(super) struct HdlExport {
    pub github: String,
    pub author: String,
}

/// Test vectors for the testbench: the level's own test when it has a
/// table, else makine's simulation of the chip.
fn steps_for(target: &Target, lib: &Library, def: &ChipDef) -> Result<Vec<TestStep>, String> {
    if let Target::Level(level) = target {
        if !matches!(level.test, Test::Custom(_)) && def.inputs == level.input_pins() {
            // Big tables make big testbenches; a sample is enough.
            return Ok(level.steps().into_iter().take(2000).collect());
        }
    }
    makine_hdl::golden_steps(lib, def, &Options::default(), 200)
}

impl App {
    /// The library to export against: the player's chips, with the one
    /// being edited as it is now.
    fn export_library(&self, def: &ChipDef) -> Library {
        let mut lib = self.lib.clone();
        lib.remove(&def.name);
        lib.insert(def.clone());
        lib
    }

    pub(super) fn hdl_section(&mut self, ui: &mut Ui) {
        egui::CollapsingHeader::new(tr!(
            "🔌 Gerçek donanıma aktar",
            "🔌 Take it to real hardware"
        ))
        .id_salt("hdl-export")
        .show(ui, |ui| {
            ui.label(
                RichText::new(tr!(
                    "Çipini Verilog'a çevir: Icarus Verilog'da test et, Yosys ile sentezle, bir FPGA'ya yükle ya da Tiny Tapeout ile gerçek bir çip olarak üret. Ayrıntılar Wiki'de: \"Gerçek çip\".",
                    "Turn your chip into Verilog: test it in Icarus Verilog, synthesize it with Yosys, load it onto an FPGA or have it made as a real chip through Tiny Tapeout. Details in the Wiki: \"A real chip\"."
                ))
                .weak()
                .small(),
            );
            let is_cpu = self.design().def.inputs.len() == 4
                && makine_levels::find("Cpu")
                    .is_some_and(|l| self.design().def.inputs == l.input_pins()
                        && self.design().def.outputs == l.output_pins());
            if is_cpu
                && ui
                    .button(tr!("🖥  FPGA bilgisayarı kaydet…", "🖥  Save FPGA computer…"))
                    .on_hover_text(tr!(
                        "İşlemcin + 64 KB RAM + seri port, ULX3S kartı için. Bir program seçersin (ELF ya da .s); vazgeçersen 'Merhaba' örneği konur.",
                        "Your processor + 64 KB RAM + a serial port, for the ULX3S board. You pick a program (ELF or .s); if you cancel, the 'Hello' example is used."
                    ))
                    .clicked()
            {
                self.export_fpga();
            }
            if ui
                .button(tr!("📄  Verilog projesi kaydet…", "📄  Save Verilog project…"))
                .on_hover_text(tr!(
                    "design.v, test tezgâhı (tb.v), Yosys betiği ve Makefile",
                    "design.v, a testbench (tb.v), a Yosys script and a Makefile"
                ))
                .clicked()
            {
                self.export_hdl(false);
            }
            ui.add_space(4.0);
            ui.label(RichText::new("Tiny Tapeout").strong());
            ui.horizontal(|ui| {
                let l = ui.label(tr!("GitHub adı", "GitHub name")).id;
                ui.add(
                    egui::TextEdit::singleline(&mut self.hdl.github)
                        .hint_text("kullanici")
                        .desired_width(110.0),
                )
                .labelled_by(l);
            });
            ui.horizontal(|ui| {
                let l = ui.label(tr!("Yazar", "Author")).id;
                ui.add(egui::TextEdit::singleline(&mut self.hdl.author).desired_width(130.0))
                    .labelled_by(l);
            });
            if ui
                .button(tr!(
                    "🏭  Tiny Tapeout projesi kaydet…",
                    "🏭  Save Tiny Tapeout project…"
                ))
                .on_hover_text(tr!(
                    "info.yaml, src/ ve test/: Tiny Tapeout şablon deposuna kopyalanmaya hazır",
                    "info.yaml, src/ and test/: ready to copy into the Tiny Tapeout template repository"
                ))
                .clicked()
            {
                self.export_hdl(true);
            }
        });
    }

    fn export_fpga(&mut self) {
        let def = self.design().def.clone();
        let lib = self.export_library(&def);
        let (name, bytes) = match rfd::FileDialog::new()
            .set_title(tr!(
                "FPGA'da çalışacak program (vazgeç: Merhaba örneği)",
                "Program to run on the FPGA (cancel: the Hello example)"
            ))
            .add_filter(
                tr!("RISC-V programı", "RISC-V program"),
                &["elf", "s", "S", "asm", ""],
            )
            .pick_file()
        {
            Some(path) => match std::fs::read(&path) {
                Ok(b) => (path.display().to_string(), b),
                Err(e) => {
                    self.status = Some(format!("{}: {e}", path.display()));
                    return;
                }
            },
            None => (
                "Merhaba".to_string(),
                crate::computer::EXAMPLES[0].1.as_bytes().to_vec(),
            ),
        };
        let project = crate::computer::parse_program(&name, &bytes)
            .and_then(|(segs, entry)| {
                if entry != 0 {
                    return Err(tr!(
                        "program 0 adresinden başlamalı (sdk/build.sh böyle derler)",
                        "the program must start at address 0 (sdk/build.sh builds it so)"
                    )
                    .to_string());
                }
                let end = segs.iter().map(|(a, b)| *a as usize + b.len()).max().unwrap_or(0);
                if end > makine_hdl::fpga::RAM_BYTES {
                    return Err(trf!(
                        "program {end} bayt, FPGA'daki RAM 64 KB. C programını MK_RAM=64K sdk/build.sh ile derle.",
                        "the program is {end} bytes, the FPGA's RAM is 64 KB. Build the C program with MK_RAM=64K sdk/build.sh."
                    ));
                }
                let mut image = vec![0u8; end];
                for (a, b) in segs {
                    image[a as usize..a as usize + b.len()].copy_from_slice(&b);
                }
                makine_hdl::fpga_project(&lib, &def, &image)
            });
        let project = match project {
            Ok(p) => p,
            Err(e) => {
                self.status = Some(trf!("Aktarılamadı: {e}", "Could not export: {e}"));
                return;
            }
        };
        let Some(parent) = rfd::FileDialog::new()
            .set_title(tr!(
                "Projenin konacağı klasörü seç",
                "Choose a folder for the project"
            ))
            .pick_folder()
        else {
            return;
        };
        let dir = parent.join(format!("{}-fpga", def.name));
        self.status = Some(match project.write_to(&dir) {
            Ok(()) => trf!(
                "Kaydedildi: {}. Klasörde 'make sim' ile dene, 'make' ile FPGA dosyasını üret.",
                "Saved: {}. Try it with 'make sim' in the folder, build the bitstream with 'make'.",
                dir.display()
            ),
            Err(e) => format!("{}: {e}", dir.display()),
        });
    }

    fn export_hdl(&mut self, tapeout: bool) {
        let def = self.design().def.clone();
        let lib = self.export_library(&def);
        let project: Result<Project, String> =
            steps_for(&self.target, &lib, &def).and_then(|steps| {
                if tapeout {
                    let tt = TinyTapeout {
                        github: if self.hdl.github.trim().is_empty() {
                            "kullanici".into()
                        } else {
                            self.hdl.github.trim().into()
                        },
                        author: self.hdl.author.trim().into(),
                        ..TinyTapeout::default()
                    };
                    makine_hdl::tiny_tapeout(&lib, &def, &steps, &tt)
                } else {
                    makine_hdl::verilog_project(&lib, &def, &steps, &Options::default())
                }
            });
        let project = match project {
            Ok(p) => p,
            Err(e) => {
                self.status = Some(trf!("Aktarılamadı: {e}", "Could not export: {e}"));
                return;
            }
        };
        let Some(parent) = rfd::FileDialog::new()
            .set_title(tr!(
                "Projenin konacağı klasörü seç",
                "Choose a folder for the project"
            ))
            .pick_folder()
        else {
            return;
        };
        let folder = if tapeout {
            format!("{}-tinytapeout", def.name)
        } else {
            format!("{}-verilog", def.name)
        };
        let dir = parent.join(folder);
        self.status = Some(match project.write_to(&dir) {
            Ok(()) => {
                let mut msg = trf!("Kaydedildi: {}", "Saved: {}", dir.display());
                for n in &project.notes {
                    if !tapeout || !n.contains("TinyTapeout/") {
                        msg.push('\n');
                        msg.push_str(n);
                    }
                }
                msg
            }
            Err(e) => format!("{}: {e}", dir.display()),
        });
    }
}
