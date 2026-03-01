PREFIX ?= /usr/local

build:
	cargo build --release

install:
	install -Dm755 target/release/dumbshot $(DESTDIR)$(PREFIX)/bin/dumbshot

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/dumbshot
