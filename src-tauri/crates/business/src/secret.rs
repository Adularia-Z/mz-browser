//! 敏感字段落盘前用 Windows DPAPI 加密，绑定当前 Windows 用户。
//!
//! 保护的是数据文件被拷走、被其他用户读取的情况；同一用户下运行的程序仍然能解密。
//! 换电脑或换 Windows 用户后旧的密文解不开，读出来是空值，需要重新填写。
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use sqlx::Error;

use crate::database::{Db, Pool};

const PREFIX: &str = "dpapi:v1:";

#[cfg(windows)]
mod dpapi {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    };
    use windows::core::PCWSTR;

    /// 额外熵：同一用户的其他程序用默认方式调 DPAPI 解不开这里的数据。
    const ENTROPY: &[u8] = b"MZBrowser.secret.v1";

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
    }

    /// 拷出系统分配的结果并释放
    unsafe fn take(output: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        unsafe {
            let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            let _ = LocalFree(Some(HLOCAL(output.pbData as *mut _)));
            bytes
        }
    }

    pub fn encrypt(plain: &[u8]) -> Result<Vec<u8>, String> {
        let input = blob(plain);
        let entropy = blob(ENTROPY);
        let mut output = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                Some(&entropy),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
            .map_err(|error| error.to_string())?;
            Ok(take(output))
        }
    }

    pub fn decrypt(cipher: &[u8]) -> Result<Vec<u8>, String> {
        let input = blob(cipher);
        let entropy = blob(ENTROPY);
        let mut output = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptUnprotectData(
                &input,
                None,
                Some(&entropy),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
            .map_err(|error| error.to_string())?;
            Ok(take(output))
        }
    }
}

/// DPAPI 只支持 Windows。其他平台的写入失败关闭；旧版明文仍可读取。
#[cfg(not(windows))]
mod dpapi {
    pub fn encrypt(_plain: &[u8]) -> Result<Vec<u8>, String> {
        Err("DPAPI is only available on Windows".into())
    }

    pub fn decrypt(_cipher: &[u8]) -> Result<Vec<u8>, String> {
        Err("DPAPI is only available on Windows".into())
    }
}

pub fn is_protected(stored: &str) -> bool {
    stored.starts_with(PREFIX)
}

fn protect_with(
    plain: &str,
    encrypt: impl FnOnce(&[u8]) -> Result<Vec<u8>, String>,
) -> Result<String, String> {
    if plain.is_empty() || is_protected(plain) {
        return Ok(plain.to_string());
    }
    let cipher = encrypt(plain.as_bytes())
        .map_err(|error| format!("DPAPI encryption failed: {error}"))?;
    Ok(format!("{PREFIX}{}", BASE64.encode(cipher)))
}

/// 加密要写入磁盘的值。空值和已加密的值原样返回；加密失败时返回错误，不返回明文。
pub fn protect(plain: &str) -> Result<String, String> {
    protect_with(plain, dpapi::encrypt)
}

/// 还原读出来的值。旧版本留下的明文原样返回；解不开（换了电脑或用户）时返回 None。
pub fn reveal(stored: &str) -> Option<String> {
    let Some(encoded) = stored.strip_prefix(PREFIX) else {
        return Some(stored.to_string());
    };
    let result = BASE64
        .decode(encoded)
        .map_err(|error| error.to_string())
        .and_then(|cipher| dpapi::decrypt(&cipher))
        .and_then(|plain| String::from_utf8(plain).map_err(|error| error.to_string()));
    match result {
        Ok(plain) => Some(plain),
        Err(error) => {
            tracing::warn!("could not decrypt a stored secret: {error}");
            None
        }
    }
}

/// 可为空的加密列。在 DTO 字段上用 `#[sqlx(try_from = "StoredSecret")]`，读出来就是明文。
#[derive(sqlx::Type)]
#[sqlx(transparent)]
pub struct StoredSecret(Option<String>);

impl From<StoredSecret> for Option<String> {
    fn from(value: StoredSecret) -> Self {
        value.0.and_then(|stored| reveal(&stored))
    }
}

/// 非空的加密列。
#[derive(sqlx::Type)]
#[sqlx(transparent)]
pub struct StoredText(String);

impl From<StoredText> for String {
    fn from(value: StoredText) -> Self {
        reveal(&value.0).unwrap_or_default()
    }
}

/// (表, 列)。表名和列名都是这里写死的常量，不来自外部输入。
const ENCRYPTED_COLUMNS: [(&str, &str); 3] =
    [("proxies", "password"), ("platform_accounts", "password"), ("environment_cookies", "value")];

/// 把旧版本留下的明文加密。可以重复执行，已加密的行不会再处理。
pub async fn encrypt_existing(pool: &Pool<Db>) -> Result<u64, Error> {
    let mut updated = 0;
    let mut tx = pool.begin().await?;
    for (table, column) in ENCRYPTED_COLUMNS {
        let select = format!(
            "SELECT id, {column} FROM {table} WHERE {column} IS NOT NULL AND {column} != '' AND {column} NOT LIKE '{PREFIX}%'"
        );
        let rows: Vec<(i64, String)> = sqlx::query_as(&select).fetch_all(&mut *tx).await?;
        let update = format!("UPDATE {table} SET {column} = $1 WHERE id = $2");
        for (id, plain) in rows {
            let protected = protect(&plain).map_err(Error::Protocol)?;
            if protected == plain {
                continue;
            }
            sqlx::query(&update).bind(protected).bind(id).execute(&mut *tx).await?;
            updated += 1;
        }
    }
    tx.commit().await?;
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encryption_failure_returns_an_error_instead_of_the_plaintext() {
        let plaintext = "secret that must not be stored";
        let result = protect_with(plaintext, |_| Err("forced DPAPI failure".into()));

        assert!(matches!(
            result,
            Err(error)
                if error.contains("forced DPAPI failure") && !error.contains(plaintext)
        ));
    }

    #[test]
    fn empty_and_already_protected_values_are_unchanged() {
        let empty = protect_with("", |_| Err("encrypt should not be called".into()));
        assert!(matches!(empty, Ok(value) if value.is_empty()));

        let stored = format!("{PREFIX}AAAA");
        let unchanged = protect_with(&stored, |_| Err("encrypt should not be called".into()));
        assert!(matches!(unchanged, Ok(value) if value == stored));
    }

    #[test]
    fn legacy_plaintext_reads_as_is() {
        assert_eq!(reveal("old-password").as_deref(), Some("old-password"));
    }

    #[test]
    fn undecryptable_values_read_as_empty_rather_than_failing() {
        assert_eq!(reveal(&format!("{PREFIX}AAAA")), None);
        assert_eq!(reveal(&format!("{PREFIX}not base64!")), None);
        let column: Option<String> = StoredSecret(Some(format!("{PREFIX}AAAA"))).into();
        assert_eq!(column, None);
        let text: String = StoredText(format!("{PREFIX}AAAA")).into();
        assert_eq!(text, "");
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_writes_fail_closed() {
        let plaintext = "secret that must not be stored";
        assert!(matches!(protect(plaintext), Err(error) if !error.contains(plaintext)));
    }

    #[cfg(windows)]
    #[test]
    fn round_trip_including_chinese() {
        let stored = protect("密码 p@ss").expect("DPAPI encryption should succeed");
        assert!(is_protected(&stored));
        assert!(!stored.contains("p@ss"));
        assert_eq!(reveal(&stored).as_deref(), Some("密码 p@ss"));
    }
}
