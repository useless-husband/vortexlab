# All paths are relative: the repository may live in a directory whose name
# contains spaces or non-ASCII characters.
CARGO ?= cargo
JOBS ?= 4
THREADS ?= 4
BIN := ./target/release/vortexlab

.PHONY: build test lint demo quick validate corners bench report shapes fixtures clean

build:
	$(CARGO) build --release -j $(JOBS)

# Unit tests, solver tests, codec tests and the small-grid regression tests (about a minute).
test:
	$(CARGO) test --release -j $(JOBS)

lint:
	$(CARGO) fmt --check
	$(CARGO) clippy --release -j $(JOBS) --all-targets -- -D warnings

# Coarse cylinder benchmark plus an animated vortex street (about half a minute).
demo: build
	$(BIN) demo --threads $(THREADS)

# Every validation case on small grids, written to results/quick (about a minute).
quick: build
	$(BIN) validate --quick --threads $(THREADS) --out results/quick

# Every validation case at full resolution; see README for the measured runtime.
validate: build
	$(BIN) validate --threads $(THREADS) --out results

# The corner-modification experiment: main set, then the sensitivity sets (see README for runtimes).
corners: build
	$(BIN) corners --tag main --re 200 --d 60 --threads $(THREADS) --out results
	$(BIN) corners --tag re100 --re 100 --d 60 --no-animation --threads $(THREADS) --out results
	$(BIN) corners --tag coarse --re 200 --d 40 --no-animation --threads $(THREADS) --out results

# Million lattice updates per second at 1, 2 and 4 threads.
bench: build
	$(BIN) bench --n 40 --steps 3000 --threads-list 1,2,4 --out results
	$(BIN) bench --n 80 --steps 1000 --threads-list 1,2,4 --out results

# Rebuild docs/report from the CSV files in results/.
report: build
	$(BIN) report --results results --out docs/report

# Regenerate the sample silhouettes and the PNG test fixtures.
shapes: build
	$(BIN) make-shapes "我的形狀"

fixtures:
	python3 tools/make_png_fixtures.py

clean:
	$(CARGO) clean
	rm -rf results
