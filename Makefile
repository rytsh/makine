# makine: common commands. Run `make` (or `make help`) to see them.
#
# Works on Linux and macOS with GNU make. On Windows, use the commands in
# README.md, or run this from Git Bash / WSL.
#
# Targets are named <verb>-<thing>: run-*, build-*, dist-*, test-*.
# Lines starting with `##@` are section titles in `make help`.

CARGO ?= cargo
WIN_TARGET := x86_64-pc-windows-msvc
WIN_DIR := target/$(WIN_TARGET)/release
DOOM := examples/doom/build/doom.elf
DIST := dist
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
MAC_APP := target/release/makine.app

# Apple's clang can't target RISC-V, so on macOS use Homebrew's llvm for
# build-doom and build-c (brew install llvm lld).
ifeq ($(shell uname -s),Darwin)
MAC_ARCH := $(subst x86_64,x64,$(shell uname -m))
MAC_DIST := makine-macos-$(MAC_ARCH)
LLVM_BIN := $(firstword $(wildcard /opt/homebrew/opt/llvm/bin /usr/local/opt/llvm/bin))
ifneq ($(LLVM_BIN),)
export PATH := $(LLVM_BIN):$(PATH)
endif
endif

.DEFAULT_GOAL := help

.PHONY: help
help: ## Show this list
	@awk 'BEGIN {FS = ":.*## "} \
		/^##@/ {printf "\n\033[1m%s\033[0m\n", substr($$0, 5)} \
		/^[a-zA-Z_-]+:.*## / {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}' \
		$(MAKEFILE_LIST)

##@ Running

.PHONY: run
run: ## Run the app (release build)
	$(CARGO) run --release -p makine

.PHONY: run-gl
run-gl: ## Run the app with OpenGL instead of Vulkan/DirectX
	WGPU_BACKEND=gl $(CARGO) run --release -p makine

.PHONY: run-debug
run-debug: ## Run the app (debug build, faster to compile)
	$(CARGO) run -p makine

##@ Building

.PHONY: build
build: ## Build the app for this computer -> target/release/makine
	$(CARGO) build --release -p makine

.PHONY: build-relay
build-relay: ## Build the relay server for playing over the internet -> target/release/makine-relay
	$(CARGO) build --release -p makine-net --bin makine-relay

.PHONY: build-windows
build-windows: ## Build makine.exe + makine-relay.exe for Windows (needs cargo-xwin)
	@command -v cargo-xwin >/dev/null || { \
		echo "cargo-xwin bulunamadı: cargo install cargo-xwin"; exit 1; }
	$(CARGO) xwin build --release -p makine --target $(WIN_TARGET)
	$(CARGO) xwin build --release -p makine-net --bin makine-relay --target $(WIN_TARGET)
	@echo "-> $(WIN_DIR)/makine.exe $(WIN_DIR)/makine-relay.exe"

.PHONY: build-doom
build-doom: $(DOOM) ## Build Doom for makine (needs clang, ld.lld, curl, git)

$(DOOM): examples/doom/build.sh examples/doom/doomgeneric_makine.c sdk/src/libc.c
	examples/doom/build.sh

.PHONY: build-c
build-c: ## Compile a C program: make build-c SRC=examples/c/merhaba.c
	@test -n "$(SRC)" || { echo "kullanım: make build-c SRC=program.c"; exit 1; }
	sdk/build.sh $(SRC)

.PHONY: build-diagrams
build-diagrams: ## Redraw the wiki's diagrams (docs/diagrams/make.py; needs resvg)
	python3 docs/diagrams/make.py

# makine.app holds the app and Doom (Contents/Resources). It gets an ad-hoc
# signature: enough for this Mac; on others, open it once with right-click > Open.
.PHONY: build-macos-app
build-macos-app: build build-doom ## Build makine.app for macOS -> target/release/makine.app
	@test "$$(uname -s)" = Darwin || { echo "build-macos-app yalnızca macOS'ta çalışır"; exit 1; }
	rm -rf $(MAC_APP)
	mkdir -p $(MAC_APP)/Contents/MacOS $(MAC_APP)/Contents/Resources
	sed 's/@VERSION@/$(VERSION)/g' crates/app/Info.plist > $(MAC_APP)/Contents/Info.plist
	cp target/release/makine $(MAC_APP)/Contents/MacOS/
	cp assets/makine.icns $(DOOM) $(MAC_APP)/Contents/Resources/
	codesign --force --sign - $(MAC_APP)
	@echo "-> $(MAC_APP)"

##@ Packaging

.PHONY: dist-linux
dist-linux: build build-relay build-doom ## Zip makine + relay + Doom -> dist/makine-linux-x64.zip
	$(call package,makine-linux-x64,target/release/makine target/release/makine-relay)

.PHONY: dist-windows
dist-windows: build-windows build-doom ## Zip makine.exe + relay + Doom -> dist/makine-windows-x64.zip
	$(call package,makine-windows-x64,$(WIN_DIR)/makine.exe $(WIN_DIR)/makine-relay.exe)

.PHONY: dist-macos
dist-macos: build-macos-app build-relay ## Zip makine.app + relay on a Mac -> dist/makine-macos-<arch>.zip
	rm -rf $(DIST)/$(MAC_DIST) $(DIST)/$(MAC_DIST).zip
	mkdir -p $(DIST)/$(MAC_DIST)
	cp -R $(MAC_APP) $(DIST)/$(MAC_DIST)/
	cp target/release/makine-relay README.md README.en.md LICENSE crates/app/fonts/LICENSE-DejaVu.txt $(DIST)/$(MAC_DIST)/
	cd $(DIST) && ditto -c -k --norsrc --noextattr --noacl --keepParent $(MAC_DIST) $(MAC_DIST).zip
	@echo "-> $(DIST)/$(MAC_DIST).zip ($$(du -h $(DIST)/$(MAC_DIST).zip | cut -f1))"

.PHONY: dist
dist: dist-linux dist-windows ## Linux and Windows zips

# package(name, executables): copy the executables, Doom and docs into
# dist/name/ and zip it.
define package
	rm -rf $(DIST)/$(1) $(DIST)/$(1).zip
	mkdir -p $(DIST)/$(1)
	cp $(2) $(DOOM) README.md README.en.md LICENSE crates/app/fonts/LICENSE-DejaVu.txt $(DIST)/$(1)/
	cd $(DIST) && zip -qr $(1).zip $(1)
	@echo "-> $(DIST)/$(1).zip ($$(du -h $(DIST)/$(1).zip | cut -f1))"
endef

##@ Checking

.PHONY: check
check: ## Format check, clippy and tests (what CI should pass)
	$(CARGO) fmt --check
	$(CARGO) clippy --all-targets -- -D warnings
	$(CARGO) test

.PHONY: test
test: ## Run all tests
	$(CARGO) test

.PHONY: test-slow
test-slow: ## Also run the slow tests (CPU at NAND level), in release mode
	$(CARGO) test --release -- --include-ignored --skip screenshot

.PHONY: test-screenshots
test-screenshots: ## Render UI screenshots to dist/screenshots (needs a GPU)
	mkdir -p $(DIST)/screenshots
	MAKINE_SHOT_DIR=$(abspath $(DIST)/screenshots) WGPU_BACKEND=vulkan \
		$(CARGO) test --release -p makine screenshot -- --ignored
	@echo "-> $(DIST)/screenshots"

.PHONY: fmt
fmt: ## Format the code
	$(CARGO) fmt

##@ Cleaning

.PHONY: clean
clean: ## Delete build output (target/, dist/, Doom and SDK downloads)
	$(CARGO) clean
	rm -rf $(DIST) examples/doom/build sdk/rt
