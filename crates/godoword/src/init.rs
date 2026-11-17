//! 部署初始化：自适应降权 + 幂等 /data 目录准备（DEV-DEBT T3 回归）。
//!
//! 自适应降权（Rust 进程内，无 shell/gosu，distroless 兼容；history 0700）：
//! 读 `/proc/self/uid_map` 判 rootless/rootful。**rootless 保持 uid 0**——容器 0 映射宿主调用者，
//! 以 0 运行写出的文件归宿主用户，无 subuid 残留（降到容器 1000 反而落 subuid）。
//! **rootful 且 euid=0**：stat /data 属主 → 降到属主 uid/gid；属主为 0（docker 自建 bind/named volume）
//! 则 chown -R /data 到 1000（PUID/PGID 可覆盖）再降权。chown 须在降权前（降权后无 CAP_CHOWN）；
//! M1a→M1b 升级时旧 root 属主文件经 chown -R 转为可读写。顺序 setgroups([])→setgid→setuid。
//! SELinux :Z 容器内无法自动修复 → /data 写 EACCES 时明确提示。

use std::os::unix::fs::MetadataExt;
use std::path::Path;

use nix::unistd::{chown, geteuid, setgid, setgroups, setuid, Gid, Uid};

/// 初始化错误。
#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("IO 错误：{0}")]
    Io(#[from] std::io::Error),
    #[error("权限操作失败：{0}")]
    Nix(#[from] nix::Error),
    #[error("{0}")]
    Msg(String),
}

/// 幂等准备 /data：自适应降权 → 建 db/keys/cache/backups 子目录。容器误删重建零丢失。
pub fn init_data_dir(data_dir: &Path) -> Result<(), InitError> {
    drop_privileges(data_dir)?;
    for sub in ["db", "keys", "cache", "backups"] {
        let p = data_dir.join(sub);
        std::fs::create_dir_all(&p).map_err(|e| permission_hint(&p, e))?;
    }
    Ok(())
}

/// /data 写权限错误时附 SELinux :Z 提示（容器内无法自动 relabel）。
fn permission_hint(path: &Path, e: std::io::Error) -> InitError {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        InitError::Msg(format!(
            "无法写 {}（权限拒绝）。SELinux 主机 bind mount 须加 :Z 标签（容器内无法自动修复）；\
             并确认卷属主与运行 uid 匹配。原始错误：{e}",
            path.display()
        ))
    } else {
        InitError::Io(e)
    }
}

fn parse_id_env(key: &str) -> Option<u32> {
    std::env::var(key).ok().and_then(|s| s.parse().ok())
}

/// 自适应降权（见模块文档）。非 root 或 rootless 不降权。
fn drop_privileges(data_dir: &Path) -> Result<(), InitError> {
    let euid = geteuid();
    if !euid.is_root() {
        tracing::info!("已非 root（uid {}），无需降权", euid.as_raw());
        return Ok(());
    }
    if is_rootless()? {
        tracing::info!("rootless 容器：保持 uid 0（映射宿主调用者，文件无 subuid 残留）");
        return Ok(());
    }

    // rootful + root：以 /data 属主决定目标 uid/gid。
    std::fs::create_dir_all(data_dir)?;
    let meta = std::fs::metadata(data_dir)?;
    let owner_uid = meta.uid();
    let owner_gid = meta.gid();
    let target_uid = parse_id_env("PUID").unwrap_or(if owner_uid != 0 { owner_uid } else { 1000 });
    let target_gid = parse_id_env("PGID").unwrap_or(if owner_uid != 0 { owner_gid } else { 1000 });
    if target_uid == 0 {
        tracing::info!("目标 uid 0（PUID=0 或属主 root 且未覆盖），保持 root");
        return Ok(());
    }

    // chown -R 须在降权前（含 M1a 旧 root 文件升级）。
    chown_recursive(data_dir, target_uid, target_gid)?;
    setgroups(&[])?;
    setgid(Gid::from_raw(target_gid))?;
    setuid(Uid::from_raw(target_uid))?;
    tracing::info!("rootful 自适应降权 → uid {target_uid} gid {target_gid}");
    Ok(())
}

/// 读 /proc/self/uid_map 判定 rootless。
fn is_rootless() -> Result<bool, InitError> {
    let content = std::fs::read_to_string("/proc/self/uid_map")?;
    Ok(parse_rootless(&content))
}

/// 解析 uid_map：容器 uid 0 映射到宿主非 0 → rootless；映射到 0 → rootful。
fn parse_rootless(content: &str) -> bool {
    for line in content.lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() >= 2 {
            if let (Ok(inside), Ok(outside)) = (p[0].parse::<u32>(), p[1].parse::<u32>()) {
                if inside == 0 {
                    return outside != 0;
                }
            }
        }
    }
    false
}

/// 递归 chown（跳过已属目标者；不跟随符号链接）。
fn chown_recursive(path: &Path, uid: u32, gid: u32) -> Result<(), InitError> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.uid() != uid || meta.gid() != gid {
        chown(path, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid)))?;
    }
    if meta.is_dir() {
        for entry in std::fs::read_dir(path)? {
            chown_recursive(&entry?.path(), uid, gid)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rootless_rootless() {
        assert!(parse_rootless("0 1000 1\n1 100000 65536"));
        assert!(parse_rootless("         0       1001 1"));
    }

    #[test]
    fn parse_rootless_rootful() {
        assert!(!parse_rootless("0 0 4294967295"));
        assert!(!parse_rootless(""));
    }
}
