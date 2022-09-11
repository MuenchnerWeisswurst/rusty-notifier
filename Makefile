release:
	cargo build --release
install:
	cp target/release/api-telegram /usr/local/bin/