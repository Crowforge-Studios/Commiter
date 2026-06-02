BINARY = commiter
TARGET = x86_64-unknown-linux-musl
RELEASE_DIR = target/$(TARGET)/release

.PHONY: all build release clean docker-build

all: release

build:
	cargo build --release
	strip target/release/$(BINARY)
	cp target/release/$(BINARY) ./$(BINARY)
	@echo "Binary created: ./$(BINARY)"
	@echo "Size: $$(ls -lh $(BINARY) | awk '{print $$5}')"

release:
	@command -v x86_64-linux-musl-gcc >/dev/null 2>&1 || { \
		echo "ERROR: x86_64-linux-musl-gcc not found."; \
		echo "Install musl-tools:"; \
		echo "  Ubuntu/Debian: sudo apt install musl-tools"; \
		echo "  Fedora:        sudo dnf install musl-gcc"; \
		echo "  Arch:          sudo pacman -S musl"; \
		echo ""; \
		echo "Or use Docker:"; \
		echo "  make docker-build"; \
		exit 1; \
	}
	cargo build --target $(TARGET) --release
	strip $(RELEASE_DIR)/$(BINARY)
	cp $(RELEASE_DIR)/$(BINARY) ./$(BINARY)
	@echo "Static binary created: ./$(BINARY)"
	@echo "Size: $$(ls -lh $(BINARY) | awk '{print $$5}')"

docker-build:
	docker build -t commiter-builder .
	docker run --rm -v "$$(pwd):/out" commiter-builder cp /build/target/x86_64-unknown-linux-musl/release/commiter /out/commiter
	docker rmi commiter-builder 2>/dev/null || true
	strip ./$(BINARY) 2>/dev/null || true
	@echo "Static binary created: ./$(BINARY)"
	@echo "Size: $$(ls -lh $(BINARY) | awk '{print $$5}')"

clean:
	cargo clean
	rm -f $(BINARY)
