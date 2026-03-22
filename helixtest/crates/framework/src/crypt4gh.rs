use anyhow::Result;
use common::config::TestConfig;
use common::crypto::{corrupt_file, decrypt_file, decrypt_partial, encrypt_file};
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};
use common::util::sha256_file;
use std::io::Write;
use tempfile::NamedTempFile;

use crate::{Features, Mode};

fn write_dummy_fastq() -> Result<NamedTempFile> {
    let mut f = NamedTempFile::new()?;
    writeln!(f, "@SEQ_ID")?;
    writeln!(f, "GATTACA")?;
    writeln!(f, "+")?;
    writeln!(f, "IIIIIII")?;
    Ok(f)
}

pub async fn run_crypt4gh_checks(
    _mode: Mode,
    _features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    let mut tests = Vec::new();
    tests.push(level5_roundtrip_checksum().await);
    tests.push(level5_partial_read().await);
    tests.push(level5_corrupted_header_fails().await);
    tests.push(level5_wrong_key_fails().await);
    tests.push(level5_corrupted_ciphertext_fails().await);
    tests.push(level5_truncated_ciphertext_fails().await);

    // Optional Ferrum HTTP: rewrap vs decrypt_plain (gated by env; does not affect default CI).
    tests.push(crate::crypt4gh_ferrum_http::ferrum_crypt4gh_drs_rewrap(cfg, client).await);
    tests
        .push(crate::crypt4gh_ferrum_http::ferrum_crypt4gh_plain_matches_rewrap(cfg, client).await);

    Ok(ServiceReport {
        service: ServiceKind::Crypt4gh,
        tests,
    })
}

async fn level5_roundtrip_checksum() -> TestCaseResult {
    let res = (|| -> Result<()> {
        let input = write_dummy_fastq()?;
        let input_path = input.path().to_path_buf();
        let input_checksum = sha256_file(&input_path)?;

        let enc = NamedTempFile::new()?;
        let enc_path = enc.path().to_path_buf();
        let dec = NamedTempFile::new()?;
        let dec_path = dec.path().to_path_buf();

        let pass = "crypt4gh-test-pass";
        let _enc_checksum = encrypt_file(&input_path, &enc_path, pass)?;
        let dec_checksum = decrypt_file(&enc_path, &dec_path, pass)?;
        if dec_checksum != input_checksum {
            anyhow::bail!(
                "Crypt4GH roundtrip checksum mismatch: expected {}, got {}",
                input_checksum,
                dec_checksum
            );
        }
        Ok(())
    })();

    TestCaseResult {
        name: "Crypt4GH roundtrip checksum".into(),
        level: ComplianceLevel::Level5,
        passed: res.is_ok(),
        error: res.err().map(|e| e.to_string()),
        category: TestCategory::Checksum,
        weight: 1.0,
    }
}

async fn level5_partial_read() -> TestCaseResult {
    let res = (|| -> Result<()> {
        let input = write_dummy_fastq()?;
        let input_path = input.path().to_path_buf();

        let enc = NamedTempFile::new()?;
        let enc_path = enc.path().to_path_buf();

        let pass = "crypt4gh-test-pass";
        let _enc_checksum = encrypt_file(&input_path, &enc_path, pass)?;
        let partial = decrypt_partial(&enc_path, pass, 16)?;
        if partial.is_empty() {
            anyhow::bail!("Crypt4GH partial decrypt returned no data");
        }
        Ok(())
    })();

    TestCaseResult {
        name: "Crypt4GH partial read".into(),
        level: ComplianceLevel::Level5,
        passed: res.is_ok(),
        error: res.err().map(|e| e.to_string()),
        category: TestCategory::Checksum,
        weight: 1.0,
    }
}

async fn level5_corrupted_header_fails() -> TestCaseResult {
    let res = (|| -> Result<()> {
        let input = write_dummy_fastq()?;
        let input_path = input.path().to_path_buf();

        let enc = NamedTempFile::new()?;
        let enc_path = enc.path().to_path_buf();
        let dec = NamedTempFile::new()?;
        let dec_path = dec.path().to_path_buf();
        let pass = "crypt4gh-test-pass";
        let _enc_checksum = encrypt_file(&input_path, &enc_path, pass)?;

        let mut data = std::fs::read(&enc_path)?;
        if data.len() > 16 {
            for b in &mut data[..16] {
                *b ^= 0xAA;
            }
        } else if !data.is_empty() {
            data[0] ^= 0xAA;
        }
        std::fs::write(&enc_path, &data)?;

        let res = decrypt_file(&enc_path, &dec_path, pass);
        if res.is_ok() {
            anyhow::bail!("Decrypting with corrupted header unexpectedly succeeded");
        }
        Ok(())
    })();

    TestCaseResult {
        name: "Crypt4GH corrupted header fails".into(),
        level: ComplianceLevel::Level5,
        passed: res.is_ok(),
        error: res.err().map(|e| e.to_string()),
        category: TestCategory::Security,
        weight: 1.0,
    }
}

async fn level5_wrong_key_fails() -> TestCaseResult {
    let res = (|| -> Result<()> {
        let input = write_dummy_fastq()?;
        let input_path = input.path().to_path_buf();

        let enc = NamedTempFile::new()?;
        let enc_path = enc.path().to_path_buf();
        let dec = NamedTempFile::new()?;
        let dec_path = dec.path().to_path_buf();

        let pass_ok = "crypt4gh-test-pass";
        let pass_wrong = "crypt4gh-wrong-pass";

        let _enc_checksum = encrypt_file(&input_path, &enc_path, pass_ok)?;
        let res = decrypt_file(&enc_path, &dec_path, pass_wrong);
        if res.is_ok() {
            anyhow::bail!("Decrypting with wrong key unexpectedly succeeded");
        }
        Ok(())
    })();

    TestCaseResult {
        name: "Crypt4GH wrong key fails".into(),
        level: ComplianceLevel::Level5,
        passed: res.is_ok(),
        error: res.err().map(|e| e.to_string()),
        category: TestCategory::Security,
        weight: 1.0,
    }
}

async fn level5_corrupted_ciphertext_fails() -> TestCaseResult {
    let res = (|| -> Result<()> {
        let input = write_dummy_fastq()?;
        let input_path = input.path().to_path_buf();

        let enc = NamedTempFile::new()?;
        let enc_path = enc.path().to_path_buf();
        let dec = NamedTempFile::new()?;
        let dec_path = dec.path().to_path_buf();
        let pass = "crypt4gh-test-pass";
        let _enc_checksum = encrypt_file(&input_path, &enc_path, pass)?;

        corrupt_file(&enc_path)?;
        let res = decrypt_file(&enc_path, &dec_path, pass);
        if res.is_ok() {
            anyhow::bail!("Decrypting corrupted ciphertext unexpectedly succeeded");
        }
        Ok(())
    })();

    TestCaseResult {
        name: "Crypt4GH corrupted ciphertext fails".into(),
        level: ComplianceLevel::Level5,
        passed: res.is_ok(),
        error: res.err().map(|e| e.to_string()),
        category: TestCategory::Robustness,
        weight: 1.0,
    }
}

/// Truncated Crypt4GH stream (e.g. interrupted download) must not yield a successful full decrypt.
async fn level5_truncated_ciphertext_fails() -> TestCaseResult {
    let res = (|| -> Result<()> {
        let input = write_dummy_fastq()?;
        let input_path = input.path().to_path_buf();

        let enc = NamedTempFile::new()?;
        let enc_path = enc.path().to_path_buf();
        let dec = NamedTempFile::new()?;
        let dec_path = dec.path().to_path_buf();
        let pass = "crypt4gh-test-pass";
        let _enc_checksum = encrypt_file(&input_path, &enc_path, pass)?;

        let mut data = std::fs::read(&enc_path)?;
        let len = data.len();
        if len <= 64 {
            anyhow::bail!("encrypted fixture unexpectedly small for truncation test");
        }
        data.truncate(len.saturating_sub(48));
        std::fs::write(&enc_path, &data)?;

        let res = decrypt_file(&enc_path, &dec_path, pass);
        if res.is_ok() {
            anyhow::bail!("Decrypting truncated Crypt4GH stream unexpectedly succeeded");
        }
        Ok(())
    })();

    TestCaseResult {
        name: "Crypt4GH truncated ciphertext stream fails".into(),
        level: ComplianceLevel::Level5,
        passed: res.is_ok(),
        error: res.err().map(|e| e.to_string()),
        category: TestCategory::Robustness,
        weight: 1.0,
    }
}
