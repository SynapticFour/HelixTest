use crate::util::sha256_file;
use age::secrecy::SecretString;
use age::{Decryptor, Encryptor};
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

pub fn encrypt_file<P: AsRef<Path>>(input: P, output: P, passphrase: &str) -> Result<String> {
    let input_path = input.as_ref();
    let output_path = output.as_ref();
    let pass = SecretString::new(passphrase.to_owned());

    let encryptor = Encryptor::with_user_passphrase(pass);
    let mut output_file = File::create(output_path)?;
    let mut writer = encryptor.wrap_output(&mut output_file)?;

    let mut input_file = File::open(input_path)?;
    std::io::copy(&mut input_file, &mut writer)?;
    writer.finish()?;

    sha256_file(output_path)
}

pub fn decrypt_file<P: AsRef<Path>>(input: P, output: P, passphrase: &str) -> Result<String> {
    let input_path = input.as_ref();
    let output_path = output.as_ref();
    let file = File::open(input_path)?;
    let decryptor = match Decryptor::new(file)? {
        Decryptor::Passphrase(d) => d,
        _ => anyhow::bail!("Unsupported AGE decryptor type"),
    };
    let pass = SecretString::new(passphrase.to_owned());
    let mut reader = decryptor.decrypt(&pass, None::<u8>)?;
    let mut out = File::create(output_path)?;
    std::io::copy(&mut reader, &mut out)?;
    sha256_file(output_path)
}

pub fn decrypt_partial<P: AsRef<Path>>(
    input: P,
    passphrase: &str,
    num_bytes: usize,
) -> Result<Vec<u8>> {
    let input_path = input.as_ref();
    let file = File::open(input_path)?;
    let decryptor = match Decryptor::new(file)? {
        Decryptor::Passphrase(d) => d,
        _ => anyhow::bail!("Unsupported AGE decryptor type"),
    };
    let pass = SecretString::new(passphrase.to_owned());
    let mut reader = decryptor.decrypt(&pass, None::<u8>)?;
    let mut buf = vec![0u8; num_bytes];
    let n = reader.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

pub fn corrupt_file<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    let mut data = std::fs::read(path)?;
    if !data.is_empty() {
        data[0] ^= 0xFF;
    }
    let mut f = File::create(path).context("Failed to overwrite file for corruption")?;
    f.write_all(&data)?;
    Ok(())
}

