# Licensed under the Apache License, Version 2.0 or the MIT License.
# SPDX-License-Identifier: Apache-2.0 OR MIT
# Copyright Tock Contributors 2025.

# Makefile for building the tock kernel for the Seeed Studio LoRa E5 Mini board

# Optional Cargo features from make arguments.
#
# Enable with either form:
#   make flash -- --dev
#   make flash -- --process-console --debug-macro --halt-on-panic
#   make flash dev=1
#   make flash process-console=1 debug-macro=1 halt-on-panic=1
#
# `--dev` is a convenience superset: process console + debug! + halt-on-panic
# fault policy. The `--` form needs the `--` separator so make does not treat
# the flags as its own options. Empty phony targets at the end absorb those
# goals without becoming Make's default target.
CARGO_FEATURES :=
ifneq ($(filter --dev dev,$(MAKECMDGOALS))$(dev),)
  CARGO_FEATURES += dev
endif
ifneq ($(filter --process-console process-console,$(MAKECMDGOALS))$(process-console),)
  CARGO_FEATURES += process-console
endif
ifneq ($(filter --debug-macro debug-macro,$(MAKECMDGOALS))$(debug-macro),)
  CARGO_FEATURES += debug-macro
endif
ifneq ($(filter --halt-on-panic halt-on-panic,$(MAKECMDGOALS))$(halt-on-panic),)
  CARGO_FEATURES += halt-on-panic
endif

CARGO_FEATURES := $(strip $(CARGO_FEATURES))

include ../Makefile.common

# Inject --features into cargo build/rustc/check without redefining shared
# Makefile.common recipes (which would warn about overriding commands).
# Set after the include so `cargo tree` / config probes still use plain cargo.
ifneq ($(CARGO_FEATURES),)
  empty :=
  space := $(empty) $(empty)
  comma := ,
  FEATURES_CSV := $(subst $(space),$(comma),$(CARGO_FEATURES))
  # `dev` / `debug-macro` must not pull in the board default `no-debug-macro`.
  ifneq ($(filter debug-macro dev,$(CARGO_FEATURES)),)
    FEATURES_FLAG := --no-default-features --features=$(FEATURES_CSV)
  else
    FEATURES_FLAG := --features=$(FEATURES_CSV)
  endif
  CARGO = sh -c 'c="$$1"; shift; case "$$c" in build|rustc|check|test|clippy) exec cargo "$$c" $(FEATURES_FLAG) "$$@";; *) exec cargo "$$c" "$$@";; esac' _
endif

OPENOCD=openocd

# Default target for installing the kernel.
.PHONY: install
install: flash

.PHONY: flash-debug
flash-debug: $(TOCK_ROOT_DIRECTORY)target/$(TARGET)/debug/$(PLATFORM).bin
	$(OPENOCD) -f interface/stlink.cfg -c "transport select hla_swd" -f target/stm32wlx.cfg -c "init; reset halt; program $< verify 0x08000000; reset; shutdown" 

.PHONY: flash
flash: $(TOCK_ROOT_DIRECTORY)target/$(TARGET)/release/$(PLATFORM).bin
	$(OPENOCD) -f interface/stlink.cfg -c "transport select hla_swd" -f target/stm32wlx.cfg \
	-c "init; reset halt; program $< verify 0x08000000; reset; shutdown" 

.PHONY: erase-all
erase-all:
	$(OPENOCD) -f interface/stlink.cfg -c "transport select hla_swd" -f target/stm32wlx.cfg -c "init; reset halt; flash erase_sector 0 0 last; reset; shutdown"

# Build-check every optional feature combination (see test-feature-configs.sh).
.PHONY: test-features
test-features:
	./test-feature-configs.sh

# Absorb optional feature goals (must not be the first target — that would
# make bare `make` a no-op). Depend on `all` so `make -- --dev` still builds.
.PHONY: --dev dev --process-console process-console --debug-macro debug-macro \
	--halt-on-panic halt-on-panic
--dev dev --process-console process-console --debug-macro debug-macro \
	--halt-on-panic halt-on-panic: all
	@:
