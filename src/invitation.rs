//! OV3: password-protected invitation envelope. Hex is transport, AES-GCM is encryption.
use ring::{aead, pbkdf2};
use std::num::NonZeroU32;
const ROUNDS: u32 = 210_000;
pub fn encrypt(bytes: &[u8], password: &str) -> Result<String, String> {
    if password.chars().count() < 8 || password.len() > 256 {
        return Err("Пароль должен содержать не менее 8 символов (до 256 байт)".into());
    }
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut salt).map_err(|e| e.to_string())?;
    getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
    let key = key(password, &salt)?;
    let mut data = bytes.to_vec();
    key.seal_in_place_append_tag(
        aead::Nonce::assume_unique_for_key(nonce),
        aead::Aad::from(b"OmniView/OV3"),
        &mut data,
    )
    .map_err(|_| "Ошибка шифрования")?;
    let mut envelope = Vec::with_capacity(28 + data.len());
    envelope.extend(salt);
    envelope.extend(nonce);
    envelope.extend(data);
    use std::fmt::Write;
    let mut code = String::from("OV3-");
    for b in envelope {
        write!(&mut code, "{b:02X}").unwrap();
    }
    Ok(code)
}
pub fn decrypt(code: &str, password: &str) -> Result<Vec<u8>, String> {
    if password.len() > 256 {
        return Err("Слишком длинный пароль".into());
    }
    let code = code.trim().strip_prefix("OV3-").ok_or("Нужен код OV3")?;
    if code.len() > 16000 || code.len() < 88 || code.len() % 2 != 0 || !code.is_ascii() {
        return Err("Повреждён код подключения".into());
    }
    let mut data = (0..code.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&code[i..i + 2], 16)
                .map_err(|_| "Повреждён код подключения".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let key = key(password, &data[..16])?;
    let nonce: [u8; 12] = data[16..28].try_into().unwrap();
    let plain = key
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(b"OmniView/OV3"),
            &mut data[28..],
        )
        .map_err(|_| "Неверный пароль или изменён код подключения")?;
    Ok(plain.to_vec())
}
fn key(password: &str, salt: &[u8]) -> Result<aead::LessSafeKey, String> {
    let mut key = [0u8; 32];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(ROUNDS).unwrap(),
        salt,
        password.as_bytes(),
        &mut key,
    );
    aead::UnboundKey::new(&aead::AES_256_GCM, &key)
        .map(aead::LessSafeKey::new)
        .map_err(|_| "Ошибка ключа".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_invitation_rejects_wrong_password_and_tampering() {
        let code = encrypt(b"private invite", "пароль класса").unwrap();
        assert_eq!(decrypt(&code, "пароль класса").unwrap(), b"private invite");
        assert!(decrypt(&code, "wrong password").is_err());
        let mut bad = code.into_bytes();
        let last = bad.len() - 1;
        bad[last] = if bad[last] == b'A' { b'B' } else { b'A' };
        assert!(decrypt(std::str::from_utf8(&bad).unwrap(), "пароль класса").is_err());
    }
    #[test]
    fn envelope_uses_fresh_randomness_and_rejects_invalid_inputs() {
        assert_ne!(
            encrypt(b"same", "password").unwrap(),
            encrypt(b"same", "password").unwrap()
        );
        assert!(decrypt("OV3-😀", "password").is_err());
        assert!(encrypt(b"a", "short").is_err());
    }
}
