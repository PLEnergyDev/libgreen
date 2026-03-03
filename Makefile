S_SO := target/release/libgreen.so
S_A := target/release/libgreen.a
S_HEADER := include/green.h
LIB_DIR := /usr/lib
INCLUDE_DIR := /usr/include

all: release

release:
	cargo build --release
	sudo install -m755 $(S_SO) $(LIB_DIR)
	sudo install -m755 $(S_A) $(LIB_DIR)
	sudo install -m644 $(S_HEADER) $(INCLUDE_DIR)

uninstall:
	sudo rm -f $(LIB_DIR)/libgreen.so
	sudo rm -f $(LIB_DIR)/libgreen.a
	sudo rm -f $(INCLUDE_DIR)/green.h
	cargo clean

.PHONY: all release uninstall
.SILENT:
