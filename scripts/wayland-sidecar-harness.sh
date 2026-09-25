#!/usr/bin/env bash
# Run a command against the PlatynUI Wayland compositor from a sibling PID
# namespace — the sidecar deployment, without a container runtime.
#
# The compositor (and, with --with-app, a private AT-SPI bus and the egui test
# app) runs in one user+PID namespace. The command runs in a second, sibling
# namespace that reaches the Wayland socket, the control socket and the
# accessibility bus by absolute path through a shared directory, so the
# compositor's PID reads as 0 from the command's side.
#
# Usage:
#   scripts/wayland-sidecar-harness.sh [--with-app] [--env KEY=VALUE]... -- COMMAND [ARGS...]
#   scripts/wayland-sidecar-harness.sh --check-peercred
#
#   --with-app        also start a private AT-SPI bus and the egui test app
#                     (title "Sidecar App") in the compositor's namespace
#   --env KEY=VALUE   set KEY in the command's environment, overriding the
#                     harness default (repeatable; e.g. point
#                     PLATYNUI_CONTROL_SOCKET at a path nothing serves)
#   --check-peercred  print what SO_PEERCRED on the Wayland socket reports from
#                     inside the compositor's namespace and from the sibling one
#
# The command sees WAYLAND_DISPLAY, PLATYNUI_CONTROL_SOCKET and XDG_RUNTIME_DIR
# pointing at the shared directory (plus AT_SPI_BUS_ADDRESS with --with-app),
# and PLATYNUI_HARNESS_DIR naming a scratch directory it may write results to.
# The harness exits with the command's exit status.
#
# Prerequisites, each of which fails the run with a message naming it: `unshare`
# with unprivileged user namespaces (on distributions that restrict them, e.g.
# stock Ubuntu 24.04: `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0`),
# the built compositor, and with --with-app the built egui test app, `dbus-run-session`
# and the AT-SPI binaries. Build them with
# `cargo build -p platynui-wayland-compositor -p platynui-test-app-egui`.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSITOR="$REPO/target/debug/platynui-wayland-compositor"
TEST_APP="$REPO/target/debug/platynui-test-app-egui"
UNSHARE=(unshare --user --map-current-user --keep-caps --pid --fork --mount-proc --kill-child)
SOCKET_NAME=wl-0

WITH_APP=0
CHECK_PEERCRED=0
EXTRA_ENV=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --with-app) WITH_APP=1; shift ;;
    --env) EXTRA_ENV+=("$2"); shift 2 ;;
    --check-peercred) CHECK_PEERCRED=1; shift ;;
    --) shift; break ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done
if [[ "$CHECK_PEERCRED" -eq 0 && $# -eq 0 ]]; then
  echo "usage: $0 [--with-app] [--env KEY=VALUE]... -- COMMAND [ARGS...] | --check-peercred" >&2
  exit 2
fi

missing() { echo "prerequisite missing: $*" >&2; exit 2; }
command -v unshare >/dev/null || missing "\`unshare\` is not installed (util-linux)"
"${UNSHARE[@]}" true 2>/dev/null || missing "unprivileged user namespaces are unavailable; allow them with \`sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0\`"
[[ -x "$COMPOSITOR" ]] || missing "the compositor is not built ($COMPOSITOR); run \`cargo build -p platynui-wayland-compositor\`"
if [[ "$WITH_APP" -eq 1 ]]; then
  [[ -x "$TEST_APP" ]] || missing "the egui test app is not built ($TEST_APP); run \`cargo build -p platynui-test-app-egui\`"
  command -v dbus-run-session >/dev/null || missing "\`dbus-run-session\` is not installed"
fi

# A short path: the Wayland and bus sockets live in it.
DIR="$(mktemp -d "${XDG_RUNTIME_DIR:-/tmp}/wl-sidecar-XXXX")"
mkdir -m 700 "$DIR/rt" "$DIR/scratch"
COMPOSITOR_NS=""
cleanup() {
  local status=$?
  # SIGKILL reaches `unshare`, whose --kill-child ends the namespace's init and
  # with it everything in that namespace.
  if [[ -n "$COMPOSITOR_NS" ]]; then
    kill -9 "$COMPOSITOR_NS" 2>/dev/null || true
    wait "$COMPOSITOR_NS" 2>/dev/null || true
  fi
  rm -rf "$DIR"
  # Keep the harness's own exit status, not the killed namespace's.
  exit "$status"
}
trap cleanup EXIT

# The compositor's side: compositor, and with --with-app the AT-SPI bus and the
# app, all in one namespace. It runs until the harness kills it.
cat > "$DIR/compositor-side.sh" <<'EOS'
set -u
DIR=$1; REPO=$2; COMPOSITOR=$3; TEST_APP=$4; WITH_APP=$5; SOCKET_NAME=$6
export XDG_RUNTIME_DIR="$DIR/rt"
"$COMPOSITOR" --backend headless --socket-name "$SOCKET_NAME" --software-cursor --log-level warn \
  >"$DIR/compositor.log" 2>&1 &
for _ in $(seq 200); do
  [[ -S "$DIR/rt/$SOCKET_NAME" && -S "$DIR/rt/$SOCKET_NAME.control" ]] && break
  sleep 0.05
done
if [[ "$WITH_APP" -eq 1 ]]; then
  export WAYLAND_DISPLAY="$SOCKET_NAME" XDG_SESSION_TYPE=wayland
  # shellcheck source=/dev/null
  source "$REPO/scripts/setup-atspi.sh" 2>>"$DIR/atspi.log"
  echo "$AT_SPI_BUS_ADDRESS" > "$DIR/a11y-address.partial" && mv "$DIR/a11y-address.partial" "$DIR/a11y-address"
  LIBGL_ALWAYS_SOFTWARE=1 "$TEST_APP" --app-id com.platynui.sidecar --title "Sidecar App" >"$DIR/app.log" 2>&1 &
fi
touch "$DIR/ready"
wait
EOS
if [[ "$WITH_APP" -eq 1 ]]; then
  "${UNSHARE[@]}" env -i HOME="$HOME" PATH="$PATH" LANG=C.UTF-8 \
    dbus-run-session -- bash "$DIR/compositor-side.sh" "$DIR" "$REPO" "$COMPOSITOR" "$TEST_APP" 1 "$SOCKET_NAME" \
    >"$DIR/compositor-side.log" 2>&1 &
else
  "${UNSHARE[@]}" env -i HOME="$HOME" PATH="$PATH" LANG=C.UTF-8 \
    bash "$DIR/compositor-side.sh" "$DIR" "$REPO" "$COMPOSITOR" "$TEST_APP" 0 "$SOCKET_NAME" \
    >"$DIR/compositor-side.log" 2>&1 &
fi
COMPOSITOR_NS=$!

for _ in $(seq 400); do
  [[ -f "$DIR/ready" ]] && break
  if ! kill -0 "$COMPOSITOR_NS" 2>/dev/null; then break; fi
  sleep 0.05
done
if [[ ! -S "$DIR/rt/$SOCKET_NAME" || ! -S "$DIR/rt/$SOCKET_NAME.control" || ! -f "$DIR/ready" ]]; then
  echo "prerequisite missing: the compositor never came up in its namespace" >&2
  cat "$DIR"/*.log >&2 2>/dev/null || true
  exit 2
fi
if [[ "$WITH_APP" -eq 1 ]]; then
  # Give the app time to map its window and register on the bus.
  for _ in $(seq 200); do
    [[ -s "$DIR/a11y-address" ]] && break
    sleep 0.05
  done
  [[ -s "$DIR/a11y-address" ]] || { echo "prerequisite missing: the AT-SPI bus never came up" >&2; cat "$DIR"/*.log >&2; exit 2; }
  sleep 3
fi

WAYLAND_SOCKET="$DIR/rt/$SOCKET_NAME"
PEERCRED='import socket, struct, sys
s = socket.socket(socket.AF_UNIX); s.connect(sys.argv[1])
pid, uid, gid = struct.unpack("3i", s.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
print(pid)'

if [[ "$CHECK_PEERCRED" -eq 1 ]]; then
  # Inside: join the compositor's user and PID namespace (we own the user
  # namespace, so no privilege is needed). Sibling: a fresh namespace of its own.
  inside=$(nsenter --target "$(pgrep -f -n "^$COMPOSITOR" || true)" --user --pid --preserve-credentials \
    python3 -c "$PEERCRED" "$WAYLAND_SOCKET" 2>/dev/null || echo "?")
  sibling=$("${UNSHARE[@]}" python3 -c "$PEERCRED" "$WAYLAND_SOCKET")
  echo "SO_PEERCRED pid inside the compositor's namespace: $inside"
  echo "SO_PEERCRED pid from a sibling namespace: $sibling"
  exit 0
fi

SIDECAR_ENV=(
  HOME="$HOME" PATH="$PATH" LANG=C.UTF-8
  XDG_RUNTIME_DIR="$DIR/rt"
  XDG_SESSION_TYPE=wayland
  WAYLAND_DISPLAY="$WAYLAND_SOCKET"
  PLATYNUI_CONTROL_SOCKET="$WAYLAND_SOCKET.control"
  PLATYNUI_HARNESS_DIR="$DIR/scratch"
)
if [[ "$WITH_APP" -eq 1 ]]; then
  SIDECAR_ENV+=(AT_SPI_BUS_ADDRESS="$(cat "$DIR/a11y-address")")
fi
[[ -n "${RUST_LOG:-}" ]] && SIDECAR_ENV+=(RUST_LOG="$RUST_LOG")
SIDECAR_ENV+=("${EXTRA_ENV[@]}")

set +e
"${UNSHARE[@]}" env -i "${SIDECAR_ENV[@]}" "$@"
status=$?
set -e
exit $status
