release:
	cargo build --release
install:
	cp target/release/rusty-notify /usr/local/bin/
docker:
	docker build -t rusty-notify:latest .
save:
	docker save rusty-notify:latest | gzip > rusty-notify-latest.tar.gz