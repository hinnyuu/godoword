#!/usr/bin/env bash
# 容器冒烟测试：确定性 CLI 验收，资源隔离 + 自动清理（含临时用户与镜像）。
# 用法：bash scripts/acceptance/smoke.sh [镜像引用]
# 规则见 docs/project/gates.md 与 conventions.md。只清自有资源（前缀隔离），绝不碰用户容器。
set -uo pipefail

IMAGE="${1:-ghcr.io/hinnyuu/godoword:pr-1}"
TAG="gw-smoke-$$-${RANDOM}"
SMOKE_USER="${TAG}-u"
BIND_ROOT="/tmp/${TAG}-root"
PASS=0
FAIL=0

# 选一个空闲的非 1000 uid
SMOKE_UID=1002
while id "$SMOKE_UID" >/dev/null 2>&1; do SMOKE_UID=$((SMOKE_UID + 1)); done
SMOKE_HOME="/home/${SMOKE_USER}"
BIND_USER="${SMOKE_HOME}/data"

# 显式 cd 到临时用户 HOME（避免继承调用者私有 cwd 致 chdir 失败），并导出 HOME/XDG 到
# 临时用户自有路径（避免继承调用者如 CI runner 的 XDG_CONFIG_HOME 误读他人私有配置）。
# XDG_RUNTIME_DIR 须用 tmpfs 的 /run/user/UID（pasta 网络 namespace 要求，磁盘目录会 permission denied）。
as_user() {
  local cmd
  cmd=$(printf '%q ' "$@")
  sudo -u "$SMOKE_USER" bash -c "cd '$SMOKE_HOME' && export HOME='$SMOKE_HOME' XDG_CONFIG_HOME='$SMOKE_HOME/.config' XDG_DATA_HOME='$SMOKE_HOME/.local/share' XDG_RUNTIME_DIR='/run/user/$SMOKE_UID'; $cmd"
}

pass() { echo "  PASS: $1"; PASS=$((PASS + 1)); }
fail() { echo "  FAIL: $1"; FAIL=$((FAIL + 1)); }
assert_eq() { if [ "$2" = "$3" ]; then pass "$1"; else fail "$1（期望 '$2'，实得 '$3'）"; fi; }
assert_contains() { if echo "$2" | grep -q "$3"; then pass "$1"; else fail "$1（输出未含 '$3'）"; fi; }

cleanup() {
  echo "== 清理自有资源（前缀 ${TAG}）=="
  as_user bash -c "podman ps -aq --filter name=${TAG} | xargs -r podman rm -f >/dev/null 2>&1; \
    podman volume ls -q --filter name=${TAG} | xargs -r podman volume rm >/dev/null 2>&1; \
    podman rmi ${IMAGE} >/dev/null 2>&1" 2>/dev/null || true
  sudo bash -c "podman ps -aq --filter name=${TAG} | xargs -r podman rm -f >/dev/null 2>&1; \
    podman rmi ${IMAGE} >/dev/null 2>&1" 2>/dev/null || true
  sudo rm -rf "$BIND_ROOT" 2>/dev/null || true
  sudo rm -rf "/run/user/$SMOKE_UID" 2>/dev/null || true
  loginctl disable-linger "$SMOKE_USER" >/dev/null 2>&1 || true
  pkill -u "$SMOKE_USER" >/dev/null 2>&1 || true
  sleep 1
  sudo userdel -r "$SMOKE_USER" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "== 准备：临时用户 ${SMOKE_USER}(uid ${SMOKE_UID}) + 拉取镜像 =="
sudo useradd -m -u "$SMOKE_UID" "$SMOKE_USER"
loginctl enable-linger "$SMOKE_USER" >/dev/null 2>&1 || true
as_user mkdir -p "$SMOKE_HOME/.config" "$SMOKE_HOME/.local/share"
sudo mkdir -p "/run/user/$SMOKE_UID"
sudo chown "$SMOKE_UID:$SMOKE_UID" "/run/user/$SMOKE_UID"
sudo chmod 700 "/run/user/$SMOKE_UID"
as_user podman pull "$IMAGE" >/dev/null 2>&1
sudo podman pull "$IMAGE" >/dev/null 2>&1

echo "== T1 rootless + named volume（双栈/healthz/meta/静态/优雅退出）=="
as_user podman run -d --name "${TAG}-named" -p 18080:8080 -v "${TAG}-named:/data:Z" "$IMAGE" >/dev/null
assert_eq "healthz(127.0.0.1)" '{"status":"ok"}' "$(curl -s --max-time 8 --retry 5 --retry-delay 1 --retry-connrefused http://127.0.0.1:18080/healthz)"
assert_eq "healthz(localhost 双栈)" '{"status":"ok"}' "$(curl -s --max-time 8 http://localhost:18080/healthz)"
assert_contains "meta milestone=M1a" "$(curl -s --max-time 8 http://127.0.0.1:18080/api/v1/meta)" '"milestone":"M1a"'
assert_contains "静态占位页" "$(curl -s --max-time 8 http://127.0.0.1:18080/)" '<title>godoword</title>'
assert_contains "会话创建" "$(curl -s --max-time 8 -X POST http://127.0.0.1:18080/api/v1/sessions)" '"id"'
assert_contains "会话列表" "$(curl -s --max-time 8 http://127.0.0.1:18080/api/v1/sessions)" '"sessions"'
assert_contains "启动日志双栈 [::]" "$(as_user podman logs "${TAG}-named" 2>&1)" '[::]'
as_user podman stop "${TAG}-named" >/dev/null 2>&1
assert_contains "优雅退出日志" "$(as_user podman logs "${TAG}-named" 2>&1)" '优雅关闭'

echo "== T2 rootless + bind mount（属主断言）=="
as_user mkdir -p "$BIND_USER"
as_user podman run -d --name "${TAG}-bind" -p 18081:8080 -v "${BIND_USER}:/data:Z" "$IMAGE" >/dev/null
assert_eq "bind healthz" '{"status":"ok"}' "$(curl -s --max-time 8 --retry 5 --retry-delay 1 --retry-connrefused http://127.0.0.1:18081/healthz)"
assert_eq "bind 目录属主=临时用户 uid" "$SMOKE_UID" "$(stat -c %u "$BIND_USER")"

echo "== T3 rootful + bind mount =="
sudo mkdir -p "$BIND_ROOT"
sudo podman run -d --name "${TAG}-rootful" -p 18082:8080 -v "${BIND_ROOT}:/data:Z" "$IMAGE" >/dev/null
assert_eq "rootful healthz" '{"status":"ok"}' "$(curl -s --max-time 8 --retry 5 --retry-delay 1 --retry-connrefused http://127.0.0.1:18082/healthz)"

echo "== T4 删容器重建（named volume 存活）=="
as_user podman rm -f "${TAG}-named" >/dev/null 2>&1
as_user podman run -d --name "${TAG}-named" -p 18080:8080 -v "${TAG}-named:/data:Z" "$IMAGE" >/dev/null
assert_eq "重建后 healthz" '{"status":"ok"}' "$(curl -s --max-time 8 --retry 5 --retry-delay 1 --retry-connrefused http://127.0.0.1:18080/healthz)"

echo
echo "== 冒烟结果：PASS=${PASS} FAIL=${FAIL} =="
[ "$FAIL" -eq 0 ] || exit 1