# tuix build system — containerized via podman.
#
# All compilation and testing occurs inside a container.
# No Rust toolchain required on the host.
# Data transfer uses a podman named volume (no bind mounts).
#
# SEC-008: Volume is removed and recreated per build.
# SEC-009: Base image pinned, Cargo.lock committed, rust-toolchain.toml pins Rust version.

IMAGE_NAME := tuix-builder
EXPORT_IMAGE := tuix-export
VOLUME_NAME := tuix-vol
BINARY_NAME := tuix

.PHONY: build test clean run

## build: Compile the binary inside a container and export it via volume.
build:
	@echo "==> Building container image (multi-stage)..."
	podman build -t $(IMAGE_NAME) --target builder -f Containerfile .
	podman build -t $(EXPORT_IMAGE) -f Containerfile .
	@echo "==> Preparing volume (SEC-008: clean per build)..."
	-podman volume rm $(VOLUME_NAME) 2>/dev/null
	podman volume create $(VOLUME_NAME)
	@echo "==> Extracting binary via named volume..."
	podman run --rm -v $(VOLUME_NAME):/out:Z $(EXPORT_IMAGE)
	podman run --rm -v $(VOLUME_NAME):/out:Z quay.io/centos/centos:stream10-minimal \
		cat /out/$(BINARY_NAME) > ./$(BINARY_NAME)
	chmod +x ./$(BINARY_NAME)
	@echo "==> Build complete: ./$(BINARY_NAME)"

## test: Run all tests inside the builder container.
test:
	@echo "==> Building and testing..."
	podman build -t $(IMAGE_NAME) --target builder -f Containerfile .
	@echo "==> Tests passed (run as part of builder stage)."

## clean: Remove binary, container images, and named volume.
clean:
	@echo "==> Cleaning..."
	-rm -f ./$(BINARY_NAME)
	-podman rmi $(IMAGE_NAME) $(EXPORT_IMAGE) 2>/dev/null
	-podman volume rm $(VOLUME_NAME) 2>/dev/null
	@echo "==> Clean complete."

## run: Execute the built binary on the host.
## Requires: make build (binary must exist)
## Note: tuix runs natively — it needs PTY access and terminal raw mode.
run:
	@if [ ! -f ./$(BINARY_NAME) ]; then \
		echo "Error: ./$(BINARY_NAME) not found. Run 'make build' first."; \
		exit 1; \
	fi
	./$(BINARY_NAME) $(ARGS)
