# VCV Rack plugin Makefile for S-Bank.
#
# Requires the Rack SDK: set RACK_DIR to its location, e.g.
#   make RACK_DIR=/path/to/Rack-SDK
#
# Links the single prebuilt Rust plugin staticlib (modules/plugin), which bundles all
# S-Bank components behind one C ABI. Built automatically by the rule below:
#   (cd ../plugin && cargo build --release)  ->  ../../target/release/libs_bank_plugin.a

RACK_DIR ?= ../../../Rack-SDK

# Path to the Rust plugin staticlib (built by cargo at the workspace target dir).
CORE_LIB := ../../target/release/libs_bank_plugin.a

FLAGS += -I.
SOURCES += $(wildcard src/*.cpp)
LDFLAGS += $(CORE_LIB)

# On Linux the static Rust lib needs these system libs; macOS differs slightly.
ifeq ($(shell uname),Linux)
  LDFLAGS += -lpthread -ldl -lm
endif

DISTRIBUTABLES += res plugin.json

include $(RACK_DIR)/plugin.mk

# Ensure the Rust plugin staticlib is built before linking.
$(TARGET): $(CORE_LIB)
$(CORE_LIB):
	cd ../plugin && cargo build --release
