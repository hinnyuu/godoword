//! 会话虚拟路径域逻辑（M1d，服务端权威）。路径即文件夹身份（`/日语/五十音`），前端 trie 渲染。
//! NFC 归一化（日语浊音 `が` 单码位 vs `か`+组合符 NFD 视觉同形异码→裂文件夹陷阱）+ 固定上限校验。
//! 上限是协议级常量（不进 config）：校验仅作用写入时，已存数据永不回溯失效（放宽安全/收紧需 grandfather）。
//! 前缀匹配在 Rust 侧（不用 SQL LIKE，免 `%`/`_` 转义，守 JSON 整取回 Rust 纪律）。

use unicode_normalization::UnicodeNormalization;

/// 最大嵌套深度（协议级常量）。
pub const MAX_DEPTH: usize = 8;
/// 单段最大字符数（按字符非字节，CJK 友好）。
pub const MAX_SEGMENT_CHARS: usize = 64;
/// 全路径最大字符数。
pub const MAX_PATH_CHARS: usize = 1024;
/// 无模块命名空间哨兵（module chroot；区别 bindings `*` 全模块默认语义）。
pub const DEFAULT_NAMESPACE: &str = "default";

/// 路径校验错误（REST 映射 400）。
#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("无效路径：{0}")]
    Invalid(String),
}

fn invalid(msg: impl Into<String>) -> PathError {
    PathError::Invalid(msg.into())
}

/// 归一化 + 校验虚拟路径：NFC → 去首尾空白/首尾斜杠 → 逐段校验 → 重组 `/a/b`。
/// 空/纯根输入拒绝（未分类用 NULL，根文件夹无意义）。
pub fn normalize_path(input: &str) -> Result<String, PathError> {
    let nfc: String = input.nfc().collect();
    let trimmed = nfc.trim();
    let body = trimmed.trim_start_matches('/').trim_end_matches('/');
    if body.is_empty() {
        return Err(invalid("路径不可为空（未分类请用 null）"));
    }
    let segments: Vec<&str> = body.split('/').collect();
    if segments.len() > MAX_DEPTH {
        return Err(invalid(format!("嵌套深度超上限 {MAX_DEPTH}")));
    }
    for seg in &segments {
        if seg.is_empty() {
            return Err(invalid("含空段（连续斜杠）"));
        }
        if *seg == "." || *seg == ".." {
            return Err(invalid("段不可为 . 或 .."));
        }
        if seg.chars().count() > MAX_SEGMENT_CHARS {
            return Err(invalid(format!("段长度超上限 {MAX_SEGMENT_CHARS} 字符")));
        }
        if seg.chars().any(|c| c.is_control()) {
            return Err(invalid("段含控制字符"));
        }
        if seg.trim() != *seg {
            return Err(invalid("段首尾不可有空白"));
        }
    }
    let path = format!("/{}", segments.join("/"));
    if path.chars().count() > MAX_PATH_CHARS {
        return Err(invalid(format!("全路径超上限 {MAX_PATH_CHARS} 字符")));
    }
    Ok(path)
}

/// path 是否在 prefix 子树内（含自身）。前缀匹配 Rust 侧。
pub fn is_within(prefix: &str, path: &str) -> bool {
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

/// 父文件夹路径（删非空文件夹子会话上移一层用）；根级返回 None（上移到未分类 NULL）。
pub fn parent(path: &str) -> Option<String> {
    let body = path.trim_start_matches('/');
    body.rsplit_once('/').map(|(head, _)| format!("/{head}"))
}

/// 前缀改写（文件夹改名/移动）：old 子树内的 path 换前缀为 new；子树外不变。
pub fn rewrite_prefix(path: &str, old: &str, new: &str) -> String {
    if path == old {
        new.to_string()
    } else if is_within(old, path) {
        format!("{new}{}", &path[old.len()..])
    } else {
        path.to_string()
    }
}

/// 路径深度（段数）。
pub fn depth(path: &str) -> usize {
    path.trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_basic() {
        assert_eq!(normalize_path("/日语/五十音").unwrap(), "/日语/五十音");
        assert_eq!(normalize_path("日语/五十音/").unwrap(), "/日语/五十音");
        assert_eq!(normalize_path("  /a/b  ").unwrap(), "/a/b");
    }

    #[test]
    fn normalize_nfc_japanese_dakuten() {
        // か + 组合浊音符（NFD）→ が（NFC 单码位），防视觉同形裂文件夹。
        let nfd = "/か\u{3099}";
        let normalized = normalize_path(nfd).unwrap();
        assert_eq!(normalized, "/が");
        assert_eq!(normalized.chars().count(), 2); // '/' + 'が'
    }

    #[test]
    fn normalize_rejects_invalid() {
        assert!(normalize_path("").is_err());
        assert!(normalize_path("/").is_err());
        assert!(normalize_path("/a//b").is_err()); // 空段
        assert!(normalize_path("/a/../b").is_err()); // ..
        assert!(normalize_path("/a/./b").is_err()); // .
        assert!(normalize_path("/a/b /c").is_err()); // 中段尾空白
        assert!(normalize_path("/a/\u{0}b").is_err()); // 控制字符
    }

    #[test]
    fn normalize_caps() {
        let deep = format!("/{}", ["a"; MAX_DEPTH + 1].join("/"));
        assert!(normalize_path(&deep).is_err());
        let ok_depth = format!("/{}", ["a"; MAX_DEPTH].join("/"));
        assert!(normalize_path(&ok_depth).is_ok());
        let long_seg = format!("/{}", "a".repeat(MAX_SEGMENT_CHARS + 1));
        assert!(normalize_path(&long_seg).is_err());
    }

    #[test]
    fn is_within_subtree() {
        assert!(is_within("/a", "/a"));
        assert!(is_within("/a", "/a/b"));
        assert!(!is_within("/a", "/ab")); // 前缀须按段边界
        assert!(!is_within("/a", "/b"));
    }

    #[test]
    fn parent_path() {
        assert_eq!(parent("/a/b/c"), Some("/a/b".to_string()));
        assert_eq!(parent("/a/b"), Some("/a".to_string()));
        assert_eq!(parent("/a"), None);
    }

    #[test]
    fn rewrite_prefix_subtree() {
        assert_eq!(rewrite_prefix("/a/b", "/a", "/x"), "/x/b");
        assert_eq!(rewrite_prefix("/a", "/a", "/x"), "/x");
        assert_eq!(rewrite_prefix("/ab/c", "/a", "/x"), "/ab/c"); // 子树外不变
        assert_eq!(rewrite_prefix("/a/b/c", "/a/b", "/x"), "/x/c");
    }

    #[test]
    fn depth_count() {
        assert_eq!(depth("/a/b/c"), 3);
        assert_eq!(depth("/a"), 1);
    }
}
