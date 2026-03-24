test-lima:
	limactl shell default bash -c \
	  'export CARGO_TARGET_DIR=/tmp/ronly-target && \
	   cd /Users/ry/src/sshro && \
	   sudo -E env "PATH=$$PATH" \
	     $$HOME/.cargo/bin/cargo test --release 2>&1'
