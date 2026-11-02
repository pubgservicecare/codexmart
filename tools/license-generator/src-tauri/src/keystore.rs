//! The owner's Ed25519 signing key, stored encrypted on the owner's computer.
//!
//! Two layers protect the 32-byte private key:
//!
//! 1. **Owner passphrase.** Argon2id (64 MiB, 3 passes) turns the passphrase
//!    into a 256-bit key that encrypts the private key with
//!    XChaCha20-Poly1305. The key id, public key and creation time are
//!    authenticated as associated data. This layer alone is the portable
//!    **backup file** (`.cxkey`).
//! 2. **Windows DPAPI** (current Windows user). The local key file is the
//!    backup data sealed with DPAPI, so copying `signing-key.dat` to another
//!    computer or Windows account is useless without the backup file.
//!
//! A *pending* key (created from the command line before the owner chose a
//! passphrase) is protected by DPAPI only; the generator refuses to issue
//! licenses until a passphrase is set and a backup saved.
//!
//! The private key never leaves this module unencrypted except as an
//! in-memory `SigningKey` while the generator is unlocked.

use std::path::{Path, PathBuf};

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use codexmart_license::keys::{is_production_key, key_fingerprint, key_id_for};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub const MIN_PASSPHRASE: usize = 10;
const LOCAL_FILE: &str = "signing-key.dat";
const BACKUP_FORMAT: &str = "codexmart-signing-key-backup";
const LOCAL_FORMAT: &str = "codexmart-signing-key";
const KDF_MEMORY_KIB: u32 = 64 * 1024;
const KDF_ITERATIONS: u32 = 3;

pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Kdf {
    algorithm: String,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
    salt: String,
}

/// Passphrase-encrypted private key: the backup file format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Protected {
    format: String,
    version: u32,
    key_id: u8,
    public_key: String,
    created_at: String,
    kdf: Kdf,
    nonce: String,
    ciphertext: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Sealed {
    /// DPAPI only, waiting for the owner's passphrase.
    Pending {
        seed: String,
    },
    Protected {
        backup: Protected,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalFile {
    format: String,
    version: u32,
    key_id: u8,
    public_key: String,
    created_at: String,
    passphrase_set: bool,
    last_backup_at: Option<String>,
    /// DPAPI-sealed JSON of [`Sealed`], hex encoded.
    sealed: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub key_id: u8,
    pub public_key: String,
    pub fingerprint: String,
    pub created_at: String,
    pub passphrase_set: bool,
    pub last_backup_at: Option<String>,
    /// The public key is built into this source's CODE X MART builds.
    pub trusted_by_app: bool,
}

pub enum KeyState {
    None,
    Pending(KeyInfo),
    Ready(KeyInfo),
}

pub struct KeyStore {
    dir: PathBuf,
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).expect("the operating system random number generator failed");
    b
}

fn check_passphrase(passphrase: &str) -> Result<()> {
    if passphrase.chars().count() < MIN_PASSPHRASE {
        return Err(format!("Use a passphrase of at least {MIN_PASSPHRASE} characters."));
    }
    Ok(())
}

fn derive(passphrase: &str, salt: &[u8], kdf: &Kdf) -> Result<Zeroizing<[u8; 32]>> {
    let params = Params::new(kdf.memory_kib, kdf.iterations, kdf.parallelism, Some(32))
        .map_err(|e| format!("Key settings: {e}"))?;
    let mut out = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, out.as_mut())
        .map_err(|e| format!("Could not derive the key: {e}"))?;
    Ok(out)
}

fn aad(key_id: u8, public_key: &str, created_at: &str) -> Vec<u8> {
    format!("{BACKUP_FORMAT}|1|{key_id}|{public_key}|{created_at}").into_bytes()
}

fn protect(seed: &[u8; 32], passphrase: &str, created_at: &str) -> Result<Protected> {
    check_passphrase(passphrase)?;
    let public = SigningKey::from_bytes(seed).verifying_key().to_bytes();
    let public_hex = hex::encode(public);
    let key_id = key_id_for(&public);
    let salt: [u8; 16] = random();
    let kdf = Kdf {
        algorithm: "argon2id".into(),
        memory_kib: KDF_MEMORY_KIB,
        iterations: KDF_ITERATIONS,
        parallelism: 1,
        salt: hex::encode(salt),
    };
    let k = derive(passphrase, &salt, &kdf)?;
    let nonce: [u8; 24] = random();
    let cipher = XChaCha20Poly1305::new(k.as_ref().into());
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: seed, aad: &aad(key_id, &public_hex, created_at) })
        .map_err(|_| "Encryption failed.".to_string())?;
    Ok(Protected {
        format: BACKUP_FORMAT.into(),
        version: 1,
        key_id,
        public_key: public_hex,
        created_at: created_at.into(),
        kdf,
        nonce: hex::encode(nonce),
        ciphertext: hex::encode(ciphertext),
    })
}

fn unprotect(p: &Protected, passphrase: &str) -> Result<SigningKey> {
    if p.format != BACKUP_FORMAT || p.version != 1 || p.kdf.algorithm != "argon2id" {
        return Err("This is not a CODE X MART signing key backup.".into());
    }
    let salt = hex::decode(&p.kdf.salt).map_err(|_| "The key file is damaged.")?;
    let nonce = hex::decode(&p.nonce).map_err(|_| "The key file is damaged.")?;
    let ciphertext = hex::decode(&p.ciphertext).map_err(|_| "The key file is damaged.")?;
    if nonce.len() != 24 {
        return Err("The key file is damaged.".into());
    }
    let k = derive(passphrase, &salt, &p.kdf)?;
    let cipher = XChaCha20Poly1305::new(k.as_ref().into());
    let seed = Zeroizing::new(
        cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload { msg: &ciphertext, aad: &aad(p.key_id, &p.public_key, &p.created_at) },
            )
            .map_err(|_| "Wrong passphrase (or the key file was changed).".to_string())?,
    );
    let seed: [u8; 32] = seed.as_slice().try_into().map_err(|_| "The key file is damaged.")?;
    let key = SigningKey::from_bytes(&seed);
    if hex::encode(key.verifying_key().to_bytes()) != p.public_key {
        return Err("The key file is damaged: the private and public keys do not match.".into());
    }
    Ok(key)
}

impl KeyStore {
    pub fn new(dir: PathBuf) -> Self {
        KeyStore { dir }
    }

    fn path(&self) -> PathBuf {
        self.dir.join(LOCAL_FILE)
    }

    fn info(f: &LocalFile) -> KeyInfo {
        let public: [u8; 32] = hex::decode(&f.public_key).ok().and_then(|b| b.try_into().ok()).unwrap_or([0; 32]);
        KeyInfo {
            key_id: f.key_id,
            public_key: f.public_key.clone(),
            fingerprint: key_fingerprint(&public),
            created_at: f.created_at.clone(),
            passphrase_set: f.passphrase_set,
            last_backup_at: f.last_backup_at.clone(),
            trusted_by_app: is_production_key(&public),
        }
    }

    fn read(&self) -> Result<Option<LocalFile>> {
        match std::fs::read_to_string(self.path()) {
            Ok(text) => {
                let f: LocalFile =
                    serde_json::from_str(&text).map_err(|e| format!("The signing key file is damaged: {e}"))?;
                if f.format != LOCAL_FORMAT || f.version != 1 {
                    return Err("The signing key file has an unknown format.".into());
                }
                Ok(Some(f))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("Could not read the signing key: {e}")),
        }
    }

    fn write(&self, f: &LocalFile) -> Result<()> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let tmp = self.path().with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(f).unwrap()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, self.path()).map_err(|e| e.to_string())
    }

    fn seal(&self, sealed: &Sealed, template: &LocalFile) -> Result<LocalFile> {
        let json = Zeroizing::new(serde_json::to_vec(sealed).unwrap());
        Ok(LocalFile { sealed: hex::encode(dpapi::protect(&json)?), ..template.clone() })
    }

    fn unseal(&self, f: &LocalFile) -> Result<Sealed> {
        let blob = hex::decode(&f.sealed).map_err(|_| "The signing key file is damaged.")?;
        let json = Zeroizing::new(dpapi::unprotect(&blob)?);
        serde_json::from_slice(&json).map_err(|_| "The signing key file is damaged.".into())
    }

    pub fn state(&self) -> Result<KeyState> {
        Ok(match self.read()? {
            None => KeyState::None,
            Some(f) if !f.passphrase_set => KeyState::Pending(Self::info(&f)),
            Some(f) => KeyState::Ready(Self::info(&f)),
        })
    }

    fn ensure_none(&self) -> Result<()> {
        if self.read()?.is_some() {
            return Err("A signing key already exists on this computer.".into());
        }
        Ok(())
    }

    fn template(public_hex: &str, key_id: u8, created_at: &str, passphrase_set: bool) -> LocalFile {
        LocalFile {
            format: LOCAL_FORMAT.into(),
            version: 1,
            key_id,
            public_key: public_hex.into(),
            created_at: created_at.into(),
            passphrase_set,
            last_backup_at: None,
            sealed: String::new(),
        }
    }

    /// Creates a new key protected by `passphrase` (first-time setup).
    pub fn create(&self, passphrase: &str) -> Result<(KeyInfo, SigningKey)> {
        self.ensure_none()?;
        let seed = Zeroizing::new(random::<32>());
        let created = now();
        let backup = protect(&seed, passphrase, &created)?;
        let t = Self::template(&backup.public_key, backup.key_id, &created, true);
        let f = self.seal(&Sealed::Protected { backup }, &t)?;
        self.write(&f)?;
        Ok((Self::info(&f), SigningKey::from_bytes(&seed)))
    }

    /// Creates a key protected by Windows only, so its public key can be
    /// built into CODE X MART before the owner opens the generator. The
    /// generator then requires a passphrase and a backup before issuing.
    pub fn create_pending(&self) -> Result<KeyInfo> {
        self.ensure_none()?;
        let seed = Zeroizing::new(random::<32>());
        let public = SigningKey::from_bytes(&seed).verifying_key().to_bytes();
        let t = Self::template(&hex::encode(public), key_id_for(&public), &now(), false);
        let f = self.seal(&Sealed::Pending { seed: hex::encode(*seed) }, &t)?;
        self.write(&f)?;
        Ok(Self::info(&f))
    }

    pub fn current_info(&self) -> Result<Option<KeyInfo>> {
        Ok(self.read()?.map(|f| Self::info(&f)))
    }

    /// Pending key: protects it with the owner's first passphrase.
    pub fn set_passphrase(&self, passphrase: &str) -> Result<SigningKey> {
        let f = self.read()?.ok_or("There is no signing key yet.")?;
        let seed = match self.unseal(&f)? {
            Sealed::Pending { seed } => {
                Zeroizing::new(<[u8; 32]>::try_from(hex::decode(seed).map_err(|_| "damaged")?).map_err(|_| "damaged")?)
            }
            Sealed::Protected { .. } => return Err("A passphrase is already set.".into()),
        };
        let backup = protect(&seed, passphrase, &f.created_at)?;
        let mut t = f.clone();
        t.passphrase_set = true;
        let sealed = self.seal(&Sealed::Protected { backup }, &t)?;
        self.write(&sealed)?;
        Ok(SigningKey::from_bytes(&seed))
    }

    /// Owner access: decrypts the key with the passphrase.
    pub fn unlock(&self, passphrase: &str) -> Result<SigningKey> {
        let f = self.read()?.ok_or("There is no signing key yet.")?;
        match self.unseal(&f)? {
            Sealed::Protected { backup } => unprotect(&backup, passphrase),
            Sealed::Pending { .. } => Err("Set your owner passphrase first.".into()),
        }
    }

    /// The pending key without a passphrase. Development checks only.
    #[cfg(test)]
    pub fn pending_key(&self) -> Result<SigningKey> {
        let f = self.read()?.ok_or("There is no signing key yet.")?;
        match self.unseal(&f)? {
            Sealed::Pending { seed } => {
                Ok(SigningKey::from_bytes(&<[u8; 32]>::try_from(hex::decode(seed).unwrap()).map_err(|_| "damaged")?))
            }
            Sealed::Protected { .. } => Err("The key has a passphrase.".into()),
        }
    }

    pub fn change_passphrase(&self, current: &str, new: &str) -> Result<()> {
        let key = self.unlock(current)?;
        let f = self.read()?.ok_or("There is no signing key yet.")?;
        let backup = protect(&key.to_bytes(), new, &f.created_at)?;
        let sealed = self.seal(&Sealed::Protected { backup }, &f)?;
        self.write(&sealed)
    }

    /// Writes the passphrase-protected backup file (portable to a new PC).
    pub fn export_backup(&self, path: &Path) -> Result<KeyInfo> {
        let mut f = self.read()?.ok_or("There is no signing key yet.")?;
        let backup = match self.unseal(&f)? {
            Sealed::Protected { backup } => backup,
            Sealed::Pending { .. } => return Err("Set your owner passphrase first.".into()),
        };
        let json = serde_json::to_vec_pretty(&backup).unwrap();
        std::fs::write(path, &json).map_err(|e| format!("Could not save the backup: {e}"))?;
        // Read it back: a backup that cannot be read is worse than none.
        let check: Protected = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|_| "The backup file could not be verified after saving.".to_string())?;
        if check.public_key != f.public_key {
            return Err("The backup file could not be verified after saving.".into());
        }
        f.last_backup_at = Some(now());
        self.write(&f)?;
        Ok(Self::info(&f))
    }

    /// Restores a key from a backup file onto this computer.
    pub fn restore(&self, path: &Path, passphrase: &str) -> Result<(KeyInfo, SigningKey)> {
        self.ensure_none()?;
        let text = std::fs::read_to_string(path).map_err(|e| format!("Could not read the backup file: {e}"))?;
        let backup: Protected =
            serde_json::from_str(&text).map_err(|_| "This is not a CODE X MART signing key backup.".to_string())?;
        let key = unprotect(&backup, passphrase)?;
        let mut t = Self::template(&backup.public_key, backup.key_id, &backup.created_at, true);
        // The file just used is itself a backup.
        t.last_backup_at = Some(now());
        let f = self.seal(&Sealed::Protected { backup }, &t)?;
        self.write(&f)?;
        Ok((Self::info(&f), key))
    }
}

#[cfg(windows)]
mod dpapi {
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    const ENTROPY: &[u8] = b"CODE X MART License Generator v1";

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
    }

    unsafe fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        LocalFree(out.pbData.cast());
        v
    }

    pub fn protect(data: &[u8]) -> super::Result<Vec<u8>> {
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: null_mut() };
        unsafe {
            if CryptProtectData(
                &blob(data),
                null(),
                &blob(ENTROPY),
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            ) == 0
            {
                return Err(format!("Windows could not protect the key ({}).", std::io::Error::last_os_error()));
            }
            Ok(take(out))
        }
    }

    pub fn unprotect(data: &[u8]) -> super::Result<Vec<u8>> {
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: null_mut() };
        unsafe {
            if CryptUnprotectData(
                &blob(data),
                null_mut(),
                &blob(ENTROPY),
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            ) == 0
            {
                return Err(
                    "Windows could not open the signing key. It was saved by a different Windows user or computer: restore it from your key backup file."
                        .into(),
                );
            }
            Ok(take(out))
        }
    }
}

#[cfg(not(windows))]
mod dpapi {
    pub fn protect(data: &[u8]) -> super::Result<Vec<u8>> {
        Ok(data.to_vec())
    }
    pub fn unprotect(data: &[u8]) -> super::Result<Vec<u8>> {
        Ok(data.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASS: &str = "correct horse battery";

    #[test]
    fn create_unlock_and_wrong_passphrase() {
        let dir = tempfile::tempdir().unwrap();
        let store = KeyStore::new(dir.path().to_path_buf());
        assert!(matches!(store.state().unwrap(), KeyState::None));
        assert!(store.create("short").is_err());
        let (info, key) = store.create(PASS).unwrap();
        assert!(info.passphrase_set);
        assert_eq!(info.public_key, hex::encode(key.verifying_key().to_bytes()));
        assert_eq!(store.unlock(PASS).unwrap().to_bytes(), key.to_bytes());
        assert!(store.unlock("wrong passphrase!").unwrap_err().contains("Wrong passphrase"));
        assert!(store.create(PASS).is_err(), "never overwrites an existing key");
        // The private key is not readable in the file.
        let file = std::fs::read_to_string(dir.path().join(LOCAL_FILE)).unwrap();
        assert!(!file.contains(&hex::encode(key.to_bytes())));
    }

    #[test]
    fn backup_and_restore_on_another_computer() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let store = KeyStore::new(a.path().to_path_buf());
        let (_, key) = store.create(PASS).unwrap();
        let backup_path = a.path().join("owner.cxkey");
        let info = store.export_backup(&backup_path).unwrap();
        assert!(info.last_backup_at.is_some());
        let backup_text = std::fs::read_to_string(&backup_path).unwrap();
        assert!(!backup_text.contains(&hex::encode(key.to_bytes())));

        let other = KeyStore::new(b.path().to_path_buf());
        assert!(other.restore(&backup_path, "not the passphrase").is_err());
        let (restored_info, restored) = other.restore(&backup_path, PASS).unwrap();
        assert_eq!(restored.to_bytes(), key.to_bytes());
        assert_eq!(restored_info.public_key, info.public_key);
        assert_eq!(other.unlock(PASS).unwrap().to_bytes(), key.to_bytes());
    }

    #[test]
    fn changed_backup_is_rejected() {
        let a = tempfile::tempdir().unwrap();
        let store = KeyStore::new(a.path().to_path_buf());
        store.create(PASS).unwrap();
        let path = a.path().join("k.cxkey");
        store.export_backup(&path).unwrap();
        let mut backup: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        backup["createdAt"] = "2020-01-01T00:00:00Z".into();
        std::fs::write(&path, backup.to_string()).unwrap();
        let b = tempfile::tempdir().unwrap();
        assert!(KeyStore::new(b.path().to_path_buf()).restore(&path, PASS).is_err());
    }

    #[test]
    fn pending_key_needs_a_passphrase() {
        let dir = tempfile::tempdir().unwrap();
        let store = KeyStore::new(dir.path().to_path_buf());
        let info = store.create_pending().unwrap();
        assert!(!info.passphrase_set);
        assert!(store.unlock(PASS).is_err());
        assert!(store.export_backup(&dir.path().join("x.cxkey")).is_err());
        let key = store.set_passphrase(PASS).unwrap();
        assert_eq!(hex::encode(key.verifying_key().to_bytes()), info.public_key);
        assert!(store.unlock(PASS).is_ok());
        assert!(store.set_passphrase(PASS).is_err());
    }

    #[test]
    fn change_passphrase() {
        let dir = tempfile::tempdir().unwrap();
        let store = KeyStore::new(dir.path().to_path_buf());
        let (_, key) = store.create(PASS).unwrap();
        store.change_passphrase(PASS, "a brand new passphrase").unwrap();
        assert!(store.unlock(PASS).is_err());
        assert_eq!(store.unlock("a brand new passphrase").unwrap().to_bytes(), key.to_bytes());
    }
}
