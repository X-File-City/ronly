# rosshd

Read-only SSH server for production systems. AI agents (or humans) get a
shell that looks normal — `top`, `cat /var/log/syslog`, `kubectl get pods`
all work — but destructive operations are blocked at the kernel level.

No LLM-based command review. No allowlists. The kernel says no.

## How it works

When a client connects, rosshd forks a sandboxed session with four
isolation layers:

1. **Read-only filesystem** — Host filesystem bind-mounted read-only via
   mount namespace. Writes fail with `EROFS`. A small tmpfs at `/tmp`
   provides scratch space.

2. **PID namespace** — Host `/proc` mounted read-only. `ps` and `top`
   show real processes; `kill` fails because target PIDs don't exist in
   the agent's namespace.

3. **seccomp-bpf** — Blocks `kill`, `unlink`, `rename`, `truncate`,
   `mount`, `reboot`, `ptrace` at the syscall level. Defense in depth.

4. **Tool shims** — Wrapper scripts on PATH that enforce read-only
   semantics for tools with their own read/write APIs (Docker socket,
   kubectl, etc).

## Install

```
cargo build --release
# copy target/release/rosshd to your server
```

Requires Linux. Single binary, zero runtime dependencies.

## Usage

```
rosshd \
  --port 2222 \
  --host-key /etc/rosshd/host_key \
  --authorized-keys /etc/rosshd/authorized_keys \
  --tmpfs-size-mb 64
```

Host key is auto-generated if missing. Auth is SSH public keys only
(same format as `~/.ssh/authorized_keys`).

Then connect:

```
ssh -p 2222 user@host
```

## What's allowed vs blocked

**Works normally:** `cat`, `ls`, `ps`, `top`, `htop`, `grep`, `find`,
`kubectl get`, `kubectl describe`, `kubectl logs`, `docker ps`,
`docker logs`, `docker inspect`, `perf stat`

**Blocked with clear error:**
```
$ rm /etc/hosts
rosshd: write operation blocked (read-only filesystem)

$ docker exec -it abc123 bash
rosshd: docker exec is blocked (read-only session)

$ kubectl delete pod my-pod
rosshd: kubectl delete is blocked (read-only session)
```

## Built-in shims

- **docker** — allows `ps`, `logs`, `inspect`, `stats`, `top`, `images`,
  `info`, `version`, `events`, `diff`
- **kubectl** — allows `get`, `describe`, `logs`, `top`, `explain`,
  `version`, `cluster-info`, `api-resources`, `api-versions`,
  `config view`, `auth can-i`

Everything else is blocked by default.

## License

MIT
