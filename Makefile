test-lima:
	limactl shell default bash -c \
	  'export CARGO_TARGET_DIR=/tmp/sshro-target && \
	   cd /Users/ry/src/sshro && \
	   cargo build --release && \
	   sudo SSHRO_BIN=/tmp/sshro-target/release/sshro \
	     bash tests/integration.sh'
