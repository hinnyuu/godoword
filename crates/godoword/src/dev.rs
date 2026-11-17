//! DEV-DEBT T1：开发态旁路（固定 dev user）。
//! 仅 `dev-bypass` feature 编译，release Dockerfile 永不启用；M1c 真认证落地后**删除**本模块。
//! M1b 绑定解析已转 DB（DbBindingResolver）；dev provider/binding 经 env 幂等种入 DB（phase 6 落地）。

use kernel_api::UserId;

/// 固定 dev 用户（M1c 真认证后删除）。
pub fn dev_user() -> UserId {
    UserId(uuid::Uuid::from_u128(1))
}
