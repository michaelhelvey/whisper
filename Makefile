APP_NAME := Whisper
BIN_NAME := whisper
MODEL := ggml-small.en.bin
MODEL_URL := https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$(MODEL)
MODEL_DIR := $(HOME)/.config/whisper/models
APP_BUNDLE := target/$(APP_NAME).app

.PHONY: build bundle run clean download-model setup

build:
	cargo build --release

bundle: build
	mkdir -p $(APP_BUNDLE)/Contents/MacOS
	cp target/release/$(BIN_NAME) $(APP_BUNDLE)/Contents/MacOS/
	cp Info.plist $(APP_BUNDLE)/Contents/

download-model:
	mkdir -p $(MODEL_DIR)
	@if [ ! -f "$(MODEL_DIR)/$(MODEL)" ]; then \
		echo "Downloading $(MODEL)..."; \
		curl -L -o "$(MODEL_DIR)/$(MODEL)" "$(MODEL_URL)"; \
	else \
		echo "Model already exists at $(MODEL_DIR)/$(MODEL)"; \
	fi

setup: download-model
	@echo "Add $(APP_BUNDLE) to System Settings → Privacy & Security → Accessibility"

run: bundle
	open $(APP_BUNDLE)

clean:
	cargo clean
	rm -rf $(APP_BUNDLE)
