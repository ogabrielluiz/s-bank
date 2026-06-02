# VCV Rack plugin Makefile for the vactrol LPG adapter.
#
# Requires the Rack SDK: set RACK_DIR to its location, e.g.
#   make RACK_DIR=/path/to/Rack-SDK
#
# This links the prebuilt Rust staticlib. Build the core first:
#   (cd ../crates/vactrol-core && cargo build --release)
# which produces ../target/release/libvactrol_core.a

RACK_DIR ?= ../../Rack-SDK

# Path to the Rust staticlib (built by cargo).
CORE_LIB := ../target/release/libvactrol_core.a

FLAGS += -I.
SOURCES += $(wildcard src/*.cpp)
LDFLAGS += $(CORE_LIB)

# On Linux the static Rust lib needs these system libs; macOS differs slightly.
ifeq ($(shell uname),Linux)
  LDFLAGS += -lpthread -ldl -lm
endif

DISTRIBUTABLES += res plugin.json

include $(RACK_DIR)/plugin.mk

# Ensure the Rust core is built before linking.
$(TARGET): $(CORE_LIB)
$(CORE_LIB):
	cd ../crates/vactrol-core && cargo build --release
