# VCV Rack plugin Makefile for S-Bank.
#
# Requires the Rack SDK: set RACK_DIR to its location, e.g.
#   make RACK_DIR=/path/to/Rack-SDK

RACK_DIR ?= ../../../Rack-SDK

FLAGS += -I.
SOURCES += $(wildcard src/*.cpp)

DISTRIBUTABLES += res plugin.json

include $(RACK_DIR)/plugin.mk
