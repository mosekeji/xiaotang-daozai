#![cfg(windows)]

use daozai::calendar::{self, Config, Provider};
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn failed_configuration_preserves_old_credentials() {
    let _guard = ENV_LOCK.lock().unwrap();
    let id = format!("{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
    // 内网版凭据与配置同在一个数据目录：先建目录，写入旧凭据后再把目录占死，
    // 验证「配置保存失败时旧凭据不被破坏」。
    let path = std::env::temp_dir().join(format!("daozai-credential-test-{id}"));
    std::fs::create_dir_all(&path).unwrap();
    std::env::set_var("DAOZAI_DATA_DIR", &path);
    let old = Config { provider: Provider::Feishu, username: format!("test-old-{id}"), href: None, server: None };
    calendar::secrets::save(&old, "旧密码").unwrap();
    assert_eq!(calendar::secrets::load(&old).unwrap(), "旧密码");
    // 把 calendar.json 变成目录，save_config 必然失败。
    std::fs::create_dir_all(path.join("calendar.json")).unwrap();
    assert!(calendar::save_connection(&old, "更新后的密码", Some(&old)).is_err());
    assert_eq!(calendar::secrets::load(&old).unwrap(), "旧密码");
    calendar::secrets::delete(&old);
    assert!(calendar::secrets::load(&old).is_err());
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calendar_platform_connections_are_independent() {
    let _guard = ENV_LOCK.lock().unwrap();
    let id = format!("platforms-{}", std::process::id());
    let path = std::env::temp_dir().join(format!("daozai-{id}"));
    std::fs::create_dir_all(&path).unwrap();
    std::env::set_var("DAOZAI_DATA_DIR", &path);
    let feishu = Config { provider: Provider::Feishu, username: format!("feishu-{id}"), href: None, server: None };
    let ding = Config { provider: Provider::DingTalk, username: format!("ding-{id}"), href: None, server: None };
    calendar::save_connection(&feishu, "飞书测试密码", None).unwrap();
    // 模拟升级前只保存一个平台的配置。
    std::fs::write(path.join("calendar.json"), calendar::encode_config(&feishu)).unwrap();
    calendar::save_connection(&ding, "钉钉测试密码", Some(&feishu)).unwrap();
    assert_eq!(calendar::secrets::load(&feishu).unwrap(), "飞书测试密码");
    assert_eq!(calendar::load_configs().len(), 2);
    assert_eq!(calendar::load_config(), Some(ding.clone()));
    calendar::save_config(&feishu).unwrap();
    assert_eq!(calendar::load_config(), Some(feishu.clone()));
    calendar::select_provider(Provider::WeCom).unwrap();
    assert_eq!(calendar::load_config(), None);
    assert_eq!(calendar::load_configs().len(), 2);
    calendar::select_provider(Provider::Feishu).unwrap();
    calendar::forget_config().unwrap();
    assert_eq!(calendar::load_config(), None);
    assert_eq!(calendar::load_configs(), vec![ding.clone()]);
    calendar::secrets::delete(&feishu);
    calendar::secrets::delete(&ding);
    std::fs::remove_file(path.join("calendar.json")).unwrap();
    // 内网版凭据存在同目录的 calendar-secrets.json，需一并清理。
    let _ = std::fs::remove_file(path.join("calendar-secrets.json"));
    std::fs::remove_dir_all(path).unwrap();
}
