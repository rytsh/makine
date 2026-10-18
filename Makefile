# makine: common commands. Run `make` (or `make help`) to see them.
#
# Works on Linux and macOS with GNU make. On Windows, use the commands in
# README.md, or run this from Git Bash / WSL.

CARGO ?= cargo
WIN_TARGET := x86_64-pc-windows-msvc
WIN_EXE := target/$(WIN_TARGET)/release/makine.exe
DOOM := examples/doom/build/doom.elf
DIST := dist

.DEFAULT_GOAL := help

.PHONY: help
help: ## Show this list
	@grep -hE '^[a-zA-Z_-]+:.*## ' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

# --- Running -----------------------------------------------------------

.PHONY: run
run: ## Run the app (release build)
	$(CARGO) run --release -p makine

.PHONY: run-gl
run-gl: ## Run the app with OpenGL instead of Vulkan/DirectX
	WGPU_BACKEND=gl $(CARGO) run --release -p makine

.PHONY: dev
dev: ## Run the app (debug build, faster to compile)
	$(CARGO) run -p makine

# --- Building ----------------------------------------------------------

.PHONY: build
build: ## Build the app for this computer -> target/release/makine
	$(CARGO) build --release -p makine

.PHONY: relay
relay: ## Build the relay server for playing over the internet -> target/release/makine-relay
	$(CARGO) build --release -p makine-net --bin makine-relay

.PHONY: windows
windows: ## Build makine.exe for Windows (needs cargo-xwin)
	@command -v cargo-xwin >/dev/null || { \
		echo "cargo-xwin bulunamadı: cargo install cargo-xwin"; exit 1; }
	$(CARGO) xwin build --release -p makine --target $(WIN_TARGET)
	@echo "-> $(WIN_EXE)"

.PHONY: doom
doom: $(DOOM) ## Build Doom for makine (needs clang, ld.lld, curl, git)

$(DOOM): examples/doom/build.sh examples/doom/doomgeneric_makine.c sdk/src/libc.c
	examples/doom/build.sh

.PHONY: c
c: ## Compile a C program: make c SRC=examples/c/merhaba.c
	@test -n "$(SRC)" || { echo "kullanım: make c SRC=program.c"; exit 1; }
	sdk/build.sh $(SRC)

.PHONY: diagrams
diagrams: ## Redraw the wiki's diagrams (docs/diagrams/make.py; needs resvg)
	python3 docs/diagrams/make.py

# --- Packaging ---------------------------------------------------------

.PHONY: dist-windows
dist-windows: windows doom ## Zip makine.exe + Doom -> dist/makine-windows-x64.zip
	$(call package,makine-windows-x64,$(WIN_EXE))

.PHONY: dist-linux
dist-linux: build doom ## Zip makine + Doom -> dist/makine-linux-x64.zip
	$(call package,makine-linux-x64,target/release/makine)

.PHONY: dist
dist: dist-linux dist-windows ## Both zips

# package(name, executable): copy the executable, Doom and docs into
# dist/name/ and zip it.
define package
	rm -rf $(DIST)/$(1) $(DIST)/$(1).zip
	mkdir -p $(DIST)/$(1)
	cp $(2) $(DOOM) README.md README.en.md LICENSE crates/app/fonts/LICENSE-DejaVu.txt $(DIST)/$(1)/
	cd $(DIST) && zip -qr $(1).zip $(1)
	@echo "-> $(DIST)/$(1).zip ($$(du -h $(DIST)/$(1).zip | cut -f1))"
endef

# --- Checking ----------------------------------------------------------

.PHONY: test
test: ## Run all tests
	$(CARGO) test

.PHONY: test-slow
test-slow: ## Also run the slow tests (CPU at NAND level), in release mode
	$(CARGO) test --release -- --include-ignored --skip screenshot

.PHONY: check
check: ## Format check, clippy and tests (what CI should pass)
	$(CARGO) fmt --check
	$(CARGO) clippy --all-targets -- -D warnings
	$(CARGO) test

.PHONY: fmt
fmt: ## Format the code
	$(CARGO) fmt

.PHONY: screenshots
screenshots: ## Render UI screenshots to dist/screenshots (needs a GPU)
	mkdir -p $(DIST)/screenshots
	MAKINE_SHOT_DIR=$(abspath $(DIST)/screenshots) WGPU_BACKEND=vulkan \
		$(CARGO) test --release -p makine screenshot -- --ignored
	@echo "-> $(DIST)/screenshots"

# --- Cleaning ----------------------------------------------------------

.PHONY: clean
clean: ## Delete build output (target/, dist/, Doom and SDK downloads)
	$(CARGO) clean
	rm -rf $(DIST) examples/doom/build sdk/rt
