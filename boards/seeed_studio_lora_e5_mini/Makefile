# Licensed under the Apache License, Version 2.0 or the MIT License.
# SPDX-License-Identifier: Apache-2.0 OR MIT
# Copyright Tock Contributors 2025.

# Makefile for building the tock kernel for the Seeed Studio LoRa E5 Mini board

include ../Makefile.common

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

.PHONY: flash-kernel-app-blink
flash-kernel-app-blink: $(TOCK_ROOT_DIRECTORY)target/$(TARGET)/release/$(PLATFORM).bin erase-all
	# Flash the kernel.
	$(OPENOCD) -f interface/stlink.cfg -c "transport select hla_swd" -f target/stm32wlx.cfg \
        -c "init; reset halt; program $< verify 0x08000000; reset; shutdown"
	# Flash the blink app we temporarily are including in seeed studio board directory. 
	$(OPENOCD) -f interface/stlink.cfg -c "transport select hla_swd" -f target/stm32wlx.cfg -c \
		"init; reset halt; program $(TOCK_ROOT_DIRECTORY)boards/seeed_studio_lora-E5-HF/blink.tbf verify 0x08020000; reset; shutdown"

.PHONY: erase-all
erase-all:
	$(OPENOCD) -f interface/stlink.cfg -c "transport select hla_swd" -f target/stm32wlx.cfg -c "init; reset halt; flash erase_sector 0 0 last; reset; shutdown"
