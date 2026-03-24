test-lima:
	limactl shell default bash -c \
	  'export CARGO_TARGET_DIR=/tmp/ronly-target && \
	   cd /Users/ry/src/sshro && \
	   cargo build --release && \
	   sudo RONLY_BIN=/tmp/ronly-target/release/ronly \
	     bash tests/integration.sh'
