#!/usr/bin/env bash
# 容器冒烟测试（M1c）：确定性 CLI 验收，资源隔离 + 自动清理（含临时用户与镜像）。
# 用法：bash scripts/acceptance/smoke.sh [镜像引用]
# 规则见 docs/project/gates.md 与 conventions.md。只清自有资源（前缀隔离），绝不碰用户容器。
# M1c 覆盖：healthz/readyz/meta/config/静态、双栈、优雅退出、用户级 REST 未认证 401 + auth 流
# （注册首用户成管理员/cookie/me/会话/providers）、CSRF 守卫、/data 初始化断言（db/master.key 0600/backups）、
# 完整 uid/卷矩阵（rootless named/bind 非 1000 uid、rootful bind/named 降权属主）、删容器重建数据存活。
# 容器以 GODOWORD_COOKIE_SECURE=false 运行（CI 走 HTTP，cookie 须可回传）。
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

# 显式 cd 到临时用户 HOME + 导出 HOME/XDG 到自有路径；XDG_RUNTIME_DIR 用 tmpfs /run/user/UID（pasta 要求）。
as_user() {
  local cmd
  cmd=$(printf '%q ' "$@")
  sudo -u "$SMOKE_USER" bash -c "cd '$SMOKE_HOME' && export HOME='$SMOKE_HOME' XDG_CONFIG_HOME='$SMOKE_HOME/.config' XDG_DATA_HOME='$SMOKE_HOME/.local/share' XDG_RUNTIME_DIR='/run/user/$SMOKE_UID'; $cmd"
}

pass() { echo "  PASS: $1"; PASS=$((PASS + 1)); }
fail() { echo "  FAIL: $1"; FAIL=$((FAIL + 1)); }
assert_eq() { if [ "$2" = "$3" ]; then pass "$1"; else fail "$1（期望 '$2'，实得 '$3'）"; fi; }
assert_contains() { if echo "$2" | grep -q "$3"; then pass "$1"; else fail "$1（输出未含 '$3'）"; fi; }
http_code() { curl -s -o /dev/null -w '%{http_code}' --max-time 8 "$@"; }
# 等待容器就绪（M1b 启动含迁移+备份，需充分等待；轮询 readyz 至多 30s）。
wait_ready() {
  local url="$1" i
  for i in $(seq 1 30); do
    if curl -s --max-time 2 "$url" >/dev/null 2>&1; then return 0; fi
    sleep 1
  done
  return 1
}

cleanup() {
  echo "== 清理自有资源（前缀 ${TAG}）=="
  as_user bash -c "podman ps -aq --filter name=${TAG} | xargs -r podman rm -f >/dev/null 2>&1; \
    podman volume ls -q --filter name=${TAG} | xargs -r podman volume rm >/dev/null 2>&1; \
    podman rmi ${IMAGE} >/dev/null 2>&1" 2>/dev/null || true
  sudo bash -c "podman ps -aq --filter name=${TAG} | xargs -r podman rm -f >/dev/null 2>&1; \
    podman volume ls -q --filter name=${TAG} | xargs -r podman volume rm >/dev/null 2>&1; \
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

echo "== T1 rootless + named volume（双栈/healthz/readyz/meta/config/静态//data/把守/优雅退出）=="
as_user podman run -d -e GODOWORD_COOKIE_SECURE=false --name "${TAG}-named" -p 18080:8080 -v "${TAG}-named:/data:Z" "$IMAGE" >/dev/null
wait_ready http://127.0.0.1:18080/readyz
assert_eq "healthz(127.0.0.1)" '{"status":"ok"}' "$(curl -s --max-time 8 http://127.0.0.1:18080/healthz)"
assert_eq "healthz(localhost 双栈)" '{"status":"ok"}' "$(curl -s --max-time 8 http://localhost:18080/healthz)"
assert_eq "readyz（存储探活）" '{"status":"ok"}' "$(curl -s --max-time 8 http://127.0.0.1:18080/readyz)"
assert_contains "meta milestone=M1d" "$(curl -s --max-time 8 http://127.0.0.1:18080/api/v1/meta)" '"milestone":"M1d"'
assert_contains "config 脱敏（sqlite/invite）" "$(curl -s --max-time 8 http://127.0.0.1:18080/api/v1/config)" '"database_backend":"sqlite"'
assert_contains "config registration_mode" "$(curl -s --max-time 8 http://127.0.0.1:18080/api/v1/config)" '"registration_mode":"invite"'
assert_contains "SPA 伺服（title）" "$(curl -s --max-time 8 http://127.0.0.1:18080/)" '<title>godoword</title>'
assert_contains "SPA 构建产物（assets 引用）" "$(curl -s --max-time 8 http://127.0.0.1:18080/)" '/assets/index-'
OPENAPI=$(curl -s --max-time 8 http://127.0.0.1:18080/api/v1/openapi.json)
assert_contains "openapi.json" "$OPENAPI" '"openapi"'
assert_contains "openapi CreateSessionReq 注册（R5）" "$OPENAPI" 'CreateSessionReq'
assert_contains "openapi cookieAuth 认证（R5）" "$OPENAPI" 'cookieAuth'
# /data 初始化断言（容器内 stat：master.key 32 字节 0600 + db + backups）
assert_eq "master.key 32 字节 0600" "32 600" "$(as_user podman exec "${TAG}-named" stat -c '%s %a' /data/keys/master.key 2>/dev/null)"
assert_eq "db 文件存在" "yes" "$(as_user podman exec "${TAG}-named" sh -c 'test -f /data/db/godoword.sqlite && echo yes' 2>/dev/null)"
assert_eq "backups 目录存在" "yes" "$(as_user podman exec "${TAG}-named" sh -c 'test -d /data/backups && echo yes' 2>/dev/null)"
# 用户级 REST 未认证 → 401（M1c 认证把守，替换 M1b 503）
assert_eq "sessions 未认证 401" "401" "$(http_code -X POST http://127.0.0.1:18080/api/v1/sessions -H 'X-Requested-With: godoword')"
assert_eq "providers 未认证 401" "401" "$(http_code http://127.0.0.1:18080/api/v1/providers)"
# CSRF 守卫：变更请求无自定义头 → 403
assert_eq "注册无 CSRF 头 403" "403" "$(http_code -X POST http://127.0.0.1:18080/api/v1/auth/register -H 'Content-Type: application/json' -d '{"username":"x","password":"y"}')"
# auth 流：注册首用户成管理员 → cookie → me → 会话 → providers
JAR="/tmp/${TAG}-jar"
REG=$(curl -s --max-time 8 -c "$JAR" -X POST http://127.0.0.1:18080/api/v1/auth/register -H 'X-Requested-With: godoword' -H 'Content-Type: application/json' -d '{"username":"smoke-admin","password":"smoke-pass-123"}')
assert_contains "注册首用户成管理员" "$REG" '"is_admin":true'
assert_contains "me（cookie 认证）" "$(curl -s --max-time 8 -b "$JAR" http://127.0.0.1:18080/api/v1/auth/me)" '"username":"smoke-admin"'
SID=$(curl -s --max-time 8 -b "$JAR" -X POST http://127.0.0.1:18080/api/v1/sessions -H 'X-Requested-With: godoword' | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
assert_eq "认证后创建会话 200" "200" "$(http_code -b "$JAR" http://127.0.0.1:18080/api/v1/sessions/$SID)"
assert_eq "认证后 providers 200" "200" "$(http_code -b "$JAR" http://127.0.0.1:18080/api/v1/providers)"
assert_contains "admin users（管理员）" "$(curl -s --max-time 8 -b "$JAR" http://127.0.0.1:18080/api/v1/admin/users)" '"username":"smoke-admin"'
# 两用户越权负例（审计 C1/C2）：邀请注册 user2 → 跨用户访问 admin 会话 404 / admin 面 403。
JAR2="/tmp/${TAG}-jar2"
CODE=$(curl -s --max-time 8 -b "$JAR" -X POST http://127.0.0.1:18080/api/v1/admin/invites -H 'X-Requested-With: godoword' -H 'Content-Type: application/json' -d '{"max_uses":1}' | sed -n 's/.*"code":"\([^"]*\)".*/\1/p')
curl -s --max-time 8 -c "$JAR2" -X POST http://127.0.0.1:18080/api/v1/auth/register -H 'X-Requested-With: godoword' -H 'Content-Type: application/json' -d "{\"username\":\"smoke-user2\",\"password\":\"smoke-pass-456\",\"invite_code\":\"$CODE\"}" >/dev/null
assert_eq "user2 跨用户读 admin 会话 404" "404" "$(http_code -b "$JAR2" http://127.0.0.1:18080/api/v1/sessions/$SID)"
assert_eq "user2 跨用户删 admin 会话 404" "404" "$(http_code -b "$JAR2" -X DELETE http://127.0.0.1:18080/api/v1/sessions/$SID -H 'X-Requested-With: godoword')"
assert_eq "user2 访问 admin 面 403" "403" "$(http_code -b "$JAR2" http://127.0.0.1:18080/api/v1/admin/users)"
assert_eq "admin 会话未被 user2 删除" "200" "$(http_code -b "$JAR" http://127.0.0.1:18080/api/v1/sessions/$SID)"
# 登录限流（审计 C10：声明覆盖限流须有 429 断言）：11 次错密码 → 第 11 次 429。
LAST=""
for i in $(seq 1 11); do LAST=$(http_code -X POST http://127.0.0.1:18080/api/v1/auth/login -H 'X-Requested-With: godoword' -H 'Content-Type: application/json' -d '{"username":"smoke-admin","password":"wrong"}'); done
assert_eq "登录限流第 11 次 429" "429" "$LAST"
rm -f "$JAR" "$JAR2"
assert_contains "启动日志双栈 [::]" "$(as_user podman logs "${TAG}-named" 2>&1)" '[::]'
assert_contains "启动日志降权/保持" "$(as_user podman logs "${TAG}-named" 2>&1)" 'uid 0'
# 趁容器运行 capturing 主密钥 mtime（T4 重建复用断言用；停止后 podman exec 不可用）。
KEY_BEFORE=$(as_user podman exec "${TAG}-named" stat -c %Y /data/keys/master.key 2>/dev/null)
as_user podman stop "${TAG}-named" >/dev/null 2>&1
assert_contains "优雅退出日志" "$(as_user podman logs "${TAG}-named" 2>&1)" '优雅关闭'

echo "== T2 rootless + bind mount（非 1000 uid，文件归宿主调用者）=="
as_user mkdir -p "$BIND_USER"
as_user podman run -d -e GODOWORD_COOKIE_SECURE=false --name "${TAG}-bind" -p 18081:8080 -v "${BIND_USER}:/data:Z" "$IMAGE" >/dev/null
wait_ready http://127.0.0.1:18081/readyz
assert_eq "bind healthz" '{"status":"ok"}' "$(curl -s --max-time 8 http://127.0.0.1:18081/healthz)"
assert_eq "bind 目录属主=临时用户 uid" "$SMOKE_UID" "$(stat -c %u "$BIND_USER")"
assert_eq "bind db 文件属主=临时用户 uid（无 subuid 残留）" "$SMOKE_UID" "$(stat -c %u "$BIND_USER/db/godoword.sqlite" 2>/dev/null)"
assert_eq "bind master.key 属主=临时用户 uid" "$SMOKE_UID" "$(stat -c %u "$BIND_USER/keys/master.key" 2>/dev/null)"

echo "== T3 rootful + bind mount（属主 root → chown 1000 + 降权）=="
sudo mkdir -p "$BIND_ROOT"
sudo podman run -d -e GODOWORD_COOKIE_SECURE=false --name "${TAG}-rootful" -p 18082:8080 -v "${BIND_ROOT}:/data:Z" "$IMAGE" >/dev/null
wait_ready http://127.0.0.1:18082/readyz
assert_eq "rootful healthz" '{"status":"ok"}' "$(curl -s --max-time 8 http://127.0.0.1:18082/healthz)"
assert_eq "rootful bind 降权后属主=1000" "1000" "$(stat -c %u "$BIND_ROOT/db/godoword.sqlite" 2>/dev/null)"
assert_contains "rootful 降权日志" "$(sudo podman logs "${TAG}-rootful" 2>&1)" '降权 → uid 1000'

echo "== T3b rootful + named volume（属主 root → chown 1000 + 降权）=="
sudo podman run -d -e GODOWORD_COOKIE_SECURE=false --name "${TAG}-rootnamed" -p 18083:8080 -v "${TAG}-rootnamed:/data:Z" "$IMAGE" >/dev/null
wait_ready http://127.0.0.1:18083/readyz
assert_eq "rootful named healthz" '{"status":"ok"}' "$(curl -s --max-time 8 http://127.0.0.1:18083/healthz)"
assert_eq "rootful named master.key 属主=1000" "1000" "$(sudo podman exec "${TAG}-rootnamed" stat -c %u /data/keys/master.key 2>/dev/null)"

echo "== T4 删容器重建（named volume 数据存活，主密钥复用）=="
as_user podman rm -f "${TAG}-named" >/dev/null 2>&1
as_user podman run -d -e GODOWORD_COOKIE_SECURE=false --name "${TAG}-named" -p 18080:8080 -v "${TAG}-named:/data:Z" "$IMAGE" >/dev/null
wait_ready http://127.0.0.1:18080/readyz
assert_eq "重建后 healthz" '{"status":"ok"}' "$(curl -s --max-time 8 http://127.0.0.1:18080/healthz)"
assert_eq "重建后 readyz（数据存活）" '{"status":"ok"}' "$(curl -s --max-time 8 http://127.0.0.1:18080/readyz)"
KEY_AFTER=$(as_user podman exec "${TAG}-named" stat -c %Y /data/keys/master.key 2>/dev/null)
assert_eq "重建后主密钥复用（mtime 不变）" "$KEY_BEFORE" "$KEY_AFTER"
assert_contains "重建后 0 迁移（幂等）" "$(as_user podman logs "${TAG}-named" 2>&1)" '本次应用迁移 0 条'

echo
echo "== 冒烟结果：PASS=${PASS} FAIL=${FAIL} =="
[ "$FAIL" -eq 0 ] || exit 1