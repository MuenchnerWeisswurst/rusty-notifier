release:
	cargo build --release
install:
	cp target/release/rusty-notify /usr/local/bin/