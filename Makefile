APP_NAME := Whisper
BIN_NAME := whisper
MODEL := ggml-small.en.bin
MODEL_URL := https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$(MODEL)
MODEL_DIR := $(HOME)/.config/whisper/models
APP_BUNDLE := /Applications/$(APP_NAME).app
SIGN_ID := $(shell security find-identity -v -p codesigning 2>/dev/null | grep "Apple Development" | head -1 | sed 's/.*"\(.*\)"/\1/')

ICONSET := assets/AppIcon.iconset
ICNS := assets/AppIcon.icns
LOGO := assets/raw_logo.jpg

.PHONY: build bundle run clean download-model setup icon

build:
	cargo build --release

bundle: build
	@if [ -z "$(SIGN_ID)" ]; then \
		echo "Error: No 'Apple Development' codesigning identity found." >&2; \
		echo "" >&2; \
		echo "To fix this:" >&2; \
		echo "  1. Open Xcode → Settings → Accounts" >&2; \
		echo "  2. Add your Apple ID (any free Apple ID works)" >&2; \
		echo "  3. This creates a free 'Apple Development' signing certificate" >&2; \
		echo "" >&2; \
		echo "Run 'security find-identity -v -p codesigning' to verify." >&2; \
		exit 1; \
	fi
	rm -rf $(APP_BUNDLE)
	mkdir -p $(APP_BUNDLE)/Contents/MacOS $(APP_BUNDLE)/Contents/Resources
	cp target/release/$(BIN_NAME) $(APP_BUNDLE)/Contents/MacOS/
	cp Info.plist $(APP_BUNDLE)/Contents/
	cp assets/AppIcon.icns $(APP_BUNDLE)/Contents/Resources/
	codesign --force --sign "$(SIGN_ID)" --deep $(APP_BUNDLE)
	@echo "Signed with: $(SIGN_ID)"
	@echo "Installed to $(APP_BUNDLE)"

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

icon:
	rm -rf $(ICONSET)
	mkdir -p $(ICONSET)
	sips -z 16 16     $(LOGO) --out $(ICONSET)/icon_16x16.png      -s format png
	sips -z 32 32     $(LOGO) --out $(ICONSET)/icon_16x16@2x.png   -s format png
	sips -z 32 32     $(LOGO) --out $(ICONSET)/icon_32x32.png      -s format png
	sips -z 64 64     $(LOGO) --out $(ICONSET)/icon_32x32@2x.png   -s format png
	sips -z 128 128   $(LOGO) --out $(ICONSET)/icon_128x128.png    -s format png
	sips -z 256 256   $(LOGO) --out $(ICONSET)/icon_128x128@2x.png -s format png
	sips -z 256 256   $(LOGO) --out $(ICONSET)/icon_256x256.png    -s format png
	sips -z 512 512   $(LOGO) --out $(ICONSET)/icon_256x256@2x.png -s format png
	sips -z 512 512   $(LOGO) --out $(ICONSET)/icon_512x512.png    -s format png
	sips -z 1024 1024 $(LOGO) --out $(ICONSET)/icon_512x512@2x.png -s format png
	iconutil -c icns $(ICONSET) -o $(ICNS)
	rm -rf $(ICONSET)
	@echo "Generated $(ICNS) from $(LOGO)"
