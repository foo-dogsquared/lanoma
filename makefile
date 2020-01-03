all: build

.PHONY = "build clean create-docs format test"

# This is only used for development. 
build:
	make create-docs
	make test
	cargo build --verbose --release

clean:
	cargo clean

create-docs:
	asciidoctor -b manpage -o texture-notes.1 docs/manual.adoc

format:
	cargo fmt

test:
	cargo test --verbose --all
