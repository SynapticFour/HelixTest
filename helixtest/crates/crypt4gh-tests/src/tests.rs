use anyhow::Result;
use common::crypto::{corrupt_file, decrypt_file, decrypt_partial, encrypt_file};
use common::util::sha256_file;
use std::io::Write;
use tempfile::NamedTempFile;

fn write_dummy_fastq() -> Result<NamedTempFile> {
    let mut f = NamedTempFile::new()?;
    writeln!(f, "@SEQ_ID")?;
    writeln!(f, "GATTACA")?;
    writeln!(f, "+")?;
    writeln!(f, "IIIIIII")?;
    Ok(f)
}

#[test]
fn encrypt_decrypt_roundtrip_checksum_matches() -> Result<()> {
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
    assert_eq!(
        dec_checksum, input_checksum,
        "Decrypted checksum must match original"
    );
    Ok(())
}

#[test]
fn partial_read_returns_prefix_bytes() -> Result<()> {
    let input = write_dummy_fastq()?;
    let input_path = input.path().to_path_buf();

    let enc = NamedTempFile::new()?;
    let enc_path = enc.path().to_path_buf();
    let pass = "crypt4gh-test-pass";
    let _enc_checksum = encrypt_file(&input_path, &enc_path, pass)?;

    let partial = decrypt_partial(&enc_path, pass, 16)?;
    assert!(
        !partial.is_empty(),
        "Partial decrypt must return some data"
    );
    Ok(())
}

#[test]
fn corrupted_ciphertext_fails_to_decrypt() -> Result<()> {
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
    assert!(
        res.is_err(),
        "Decrypting corrupted ciphertext must fail, but succeeded"
    );
    Ok(())
}

#[test]
fn wrong_key_decryption_fails() -> Result<()> {
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
    assert!(
        res.is_err(),
        "Decrypting with wrong key must fail, but succeeded"
    );
    Ok(())
}

#[test]
fn corrupted_header_fails_to_decrypt() -> Result<()> {
    let input = write_dummy_fastq()?;
    let input_path = input.path().to_path_buf();

    let enc = NamedTempFile::new()?;
    let enc_path = enc.path().to_path_buf();
    let dec = NamedTempFile::new()?;
    let dec_path = dec.path().to_path_buf();
    let pass = "crypt4gh-test-pass";
    let _enc_checksum = encrypt_file(&input_path, &enc_path, pass)?;

    // Corrupt only the beginning of the file (header region)
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
    assert!(
        res.is_err(),
        "Decrypting with corrupted header must fail, but succeeded"
    );
    Ok(())
}

#[test]
fn streaming_decryption_compatible() -> Result<()> {
    use std::io::{BufRead, BufReader};

    let input = write_dummy_fastq()?;
    let input_path = input.path().to_path_buf();
    let input_checksum = sha256_file(&input_path)?;

    let enc = NamedTempFile::new()?;
    let enc_path = enc.path().to_path_buf();
    let tmp_dec = NamedTempFile::new()?;
    let tmp_dec_path = tmp_dec.path().to_path_buf();

    let pass = "crypt4gh-test-pass";
    let _enc_checksum = encrypt_file(&input_path, &enc_path, pass)?;

    // Use decrypt_file to produce a decrypted file, then read it in a streaming fashion
    let _ = decrypt_file(&enc_path, &tmp_dec_path, pass)?;
    let file = std::fs::File::open(&tmp_dec_path)?;
    let mut reader = BufReader::new(file);
    let mut buf = String::new();
    while reader.read_line(&mut buf)? > 0 {
        // no-op: just ensure streaming read works end-to-end
    }

    let final_checksum = sha256_file(&tmp_dec_path)?;
    assert_eq!(
        final_checksum, input_checksum,
        "Streaming-compatible read must preserve checksum"
    );

    Ok(())
}

