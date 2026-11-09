#
# Makefile - Top-level build, clean and flash for the Rust demo application
#
# Author:  Sandesh Ghimire <sandesh@soccentric.com>
# Target:  NUCLEO-F767ZI (STM32F767ZI)
#
# Everything runs in the stm32-dev-rust container (RCON-001), the same way as
# scripts/build.sh. Flashing and debugging use the same OpenOCD set-up as the
# C projects.
#
#   make                build the firmware (same as make build)
#   make build          cargo build --release, then .elf/.bin/.hex/.img in build/
#   make clean          remove build/ and target/
#   make rebuild        clean, then build
#   make lint           cargo clippy -D warnings (RGEN-007)
#   make fmt            cargo fmt
#   make flash          build, then upload build/embassy-app.img through BOOT
#   make erase-all      erase the whole flash (asks for confirmation)
#   make reset          reset the board
#   make debug          build, flash, then GDB halted at reset
#   make gdbserver      OpenOCD GDB server on :3333
#   make serial         serial console on the ST-LINK VCP (115200 8N1)
#   make probe          detect the ST-LINK and the target chip
#   make size           show the firmware size
#   make image          build the stm32-dev-rust Docker image
#   make help           show this help
#

ELF      := build/embassy-app.elf
IMAGE    := build/embassy-app.img
DEV      := scripts/docker-dev.sh
CARGO    := $(DEV) run env CARGO_HOME=/work/.cargo-home cargo
BOOTCTL  ?= ../bootloader/tools/bootctl.py
PORT     ?=
PYTHON   ?= python3

.DEFAULT_GOAL := build

.PHONY: all build clean rebuild lint fmt flash erase-all reset debug \
        gdbserver serial probe size image help

all: build

build:
	scripts/build.sh

clean:
	rm -rf build target

rebuild: clean
	@$(MAKE) --no-print-directory build

lint:
	$(CARGO) clippy --release -- -D warnings

fmt:
	$(CARGO) fmt

# A-6: never program the raw ELF into a slot. BOOT only starts an image
# that has its 256-byte header, metadata and records, which bootctl writes.
flash: build
	@test -f $(IMAGE) || { echo "error: $(IMAGE) not found, run make build"; exit 1; }
	@test -f $(BOOTCTL) || { echo "error: $(BOOTCTL) not found; set BOOTCTL=/path/to/bootctl.py"; exit 1; }
	$(PYTHON) $(BOOTCTL) $(if $(PORT),--port $(PORT),) update $(IMAGE) --activate

erase-all:
	@echo "WARNING: this erases the whole flash: BOOT, both metadata sectors"
	@echo "         and both application slots."
	@printf "Type 'yes' to continue: " && read answer && [ "$$answer" = "yes" ]
	$(DEV) erase

reset:
	$(DEV) reset

debug: build
	$(DEV) debug $(ELF) --load

gdbserver:
	$(DEV) gdbserver

serial:
	$(DEV) serial

probe:
	$(DEV) probe

size:
	@test -f $(ELF) || { echo "error: $(ELF) not found, run make build"; exit 1; }
	$(DEV) run arm-none-eabi-size /work/$(ELF)

image:
	$(DEV) build

help:
	@sed -n '/^#   make/p' $(firstword $(MAKEFILE_LIST)) | sed 's/^# \{0,1\}//'
